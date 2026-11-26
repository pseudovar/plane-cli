//! Policy enforcement for work-item writes: runs the project's policy rules before
//! `work-items create|update|delete`, intake create/update/delete, comment, link and
//! attachment deletes and TUI edits, and builds the `--dry-run` report. The rules themselves
//! are in `crate::policy`.

use crate::policy::{self, Check, CreateFacts, LoadedPolicy};
use plane_core::{CoreError, NameScope, PlaneApi, ProjectEntry, StateEntry};
use serde_json::{json, Value};
use uuid::Uuid;

/// The member a write is made as: `--as`, else the API key's own member (`users/me`).
pub(crate) struct Caller {
    pub me: Value,
    pub label: String,
}

fn caller(api: &PlaneApi) -> Result<Caller, CoreError> {
    let me = api.whoami()?;
    let label = crate::identity::current_member()
        .map(String::from)
        .or_else(|| {
            me.get("display_name")
                .and_then(Value::as_str)
                .map(String::from)
        })
        .unwrap_or_else(|| "this member".into());
    Ok(Caller { me, label })
}

/// Project identifier for `pid`, refreshing the catalog once if it is stale.
pub(crate) fn project_identifier(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    pid: &Uuid,
) -> Result<String, CoreError> {
    if let Some(p) = catalog.iter().find(|p| &p.id == pid) {
        return Ok(p.identifier.clone());
    }
    api.fetch_projects_catalog(true)?
        .into_iter()
        .find(|p| &p.id == pid)
        .map(|p| p.identifier)
        .ok_or_else(|| CoreError::Resolve(format!("project {pid} is not in the project list")))
}

fn load_policy(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    pid: &Uuid,
) -> Result<(String, Option<LoadedPolicy>), CoreError> {
    let ident = project_identifier(api, catalog, pid)?;
    let loaded = policy::load(&ident)?;
    Ok((ident, loaded))
}

fn find_state(states: &[StateEntry], raw: &str) -> Option<StateEntry> {
    let raw = raw.trim();
    match Uuid::parse_str(raw) {
        Ok(u) => states.iter().find(|s| s.id == u).cloned(),
        Err(_) => states
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(raw))
            .cloned(),
    }
}

/// A body `state` (UUID or name) as a catalog entry. A miss refreshes the catalog once; if
/// the state is still unknown, `policy_unknown_state` (a state rule cannot be judged on a
/// state plane-cli cannot name, so it fails closed).
fn lookup_state(
    api: &mut PlaneApi,
    pid: &Uuid,
    raw: &str,
    refresh: bool,
) -> Result<StateEntry, CoreError> {
    if let Some(s) = find_state(&api.fetch_states_catalog(pid, refresh)?, raw) {
        return Ok(s);
    }
    if !refresh {
        if let Some(s) = find_state(&api.fetch_states_catalog(pid, true)?, raw) {
            return Ok(s);
        }
    }
    Err(CoreError::usage_code(
        "policy_unknown_state",
        format!(
            "state {raw:?} is not a state of this project, so the policy's state rules cannot be checked; use a state from `states list --project <PROJECT>`"
        ),
    ))
}

/// Display name for a state id (never fails; falls back to the raw value).
fn state_label(api: &mut PlaneApi, pid: &Uuid, raw: &str, refresh: bool) -> String {
    api.fetch_states_catalog(pid, refresh)
        .ok()
        .and_then(|st| find_state(&st, raw))
        .map(|s| s.name)
        .unwrap_or_else(|| raw.to_string())
}

fn state_id_of(api: &mut PlaneApi, pid: &Uuid, raw: &str, refresh: bool) -> String {
    if Uuid::parse_str(raw.trim()).is_ok() {
        return raw.trim().to_lowercase();
    }
    api.fetch_states_catalog(pid, refresh)
        .ok()
        .and_then(|st| find_state(&st, raw))
        .map(|s| s.id.to_string())
        .unwrap_or_else(|| raw.trim().to_string())
}

fn label_names(
    api: &mut PlaneApi,
    pid: &Uuid,
    body: &Value,
    refresh: bool,
) -> Result<Vec<String>, CoreError> {
    let Some(arr) = body.get("labels").and_then(Value::as_array) else {
        return Ok(vec![]);
    };
    let raw: Vec<String> = arr
        .iter()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect();
    if raw.iter().all(|r| Uuid::parse_str(r).is_err()) {
        return Ok(raw);
    }
    let labels = api.fetch_labels_catalog(pid, refresh)?;
    Ok(raw
        .into_iter()
        .map(|r| match Uuid::parse_str(&r) {
            Ok(u) => labels
                .iter()
                .find(|l| l.id == u)
                .map(|l| l.name.clone())
                .unwrap_or(r),
            Err(_) => r,
        })
        .collect())
}

/// How the create path can set a module.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModuleSupport {
    /// `work-items create --module` (resolved UUID or none).
    Flag(Option<Uuid>),
    /// `intake create` cannot add a module.
    Unsupported,
}

/// What a create will do and how the policy judged it.
pub(crate) struct CreatePlan {
    pub project: String,
    pub policy: Option<LoadedPolicy>,
    pub checks: Vec<Check>,
    pub module: Option<Uuid>,
    /// Close matches that are open (they refuse the create without --allow-duplicate).
    pub duplicates: Vec<Value>,
    /// Close matches that are Done or Cancelled (reported, never refused).
    pub closed_duplicates: Vec<Value>,
}

/// Close-name candidates for `name` in the project, with each match's state group.
fn duplicate_matches(
    api: &mut PlaneApi,
    pid: &Uuid,
    project: &str,
    rule: &policy::SearchRule,
    name: &str,
    refresh: bool,
) -> Result<(Vec<Value>, Vec<Value>), CoreError> {
    let mut candidates: Vec<(String, String, String)> = Vec::new();
    for query in policy::search_queries(name) {
        let found = api.search_work_items(&query, Some(pid), Some(20))?;
        for i in found
            .get("issues")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let ident = i
                .get("project__identifier")
                .and_then(Value::as_str)
                .unwrap_or(project);
            let seq = i.get("sequence_id").and_then(Value::as_i64).unwrap_or(0);
            let composite = format!("{ident}-{seq}");
            if candidates.iter().all(|(c, _, _)| c != &composite) {
                let n = i.get("name").and_then(Value::as_str).unwrap_or("");
                let id = i.get("id").and_then(Value::as_str).unwrap_or("");
                candidates.push((composite, n.to_string(), id.to_string()));
            }
        }
    }
    let pairs: Vec<(String, String)> = candidates
        .iter()
        .map(|(c, n, _)| (c.clone(), n.clone()))
        .collect();
    let states = api.fetch_states_catalog(pid, refresh)?;
    let (mut open, mut closed) = (Vec::new(), Vec::new());
    for mut m in policy::close_matches(rule, name, &pairs) {
        let id = candidates
            .iter()
            .find(|(c, _, _)| Some(c.as_str()) == m["work_item"].as_str())
            .map(|(_, _, id)| id.clone())
            .unwrap_or_default();
        let group = Uuid::parse_str(&id)
            .ok()
            .and_then(|u| api.get_work_item(pid, &u).ok())
            .and_then(|item| {
                let sid = item
                    .get("state")
                    .and_then(|v| v.as_str().or_else(|| v.get("id").and_then(Value::as_str)))
                    .map(str::to_string)?;
                find_state(&states, &sid).and_then(|s| s.group)
            });
        m["state_group"] = json!(group);
        if matches!(group.as_deref(), Some("completed") | Some("cancelled")) {
            closed.push(m);
        } else {
            open.push(m);
        }
    }
    Ok((open, closed))
}

pub(crate) fn plan_create(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
    pid: &Uuid,
    body: &Value,
    module: ModuleSupport,
    allow_duplicate: bool,
) -> Result<CreatePlan, CoreError> {
    let (project, loaded) = load_policy(api, catalog, pid)?;
    let mut plan = CreatePlan {
        project,
        policy: None,
        checks: vec![],
        module: match module {
            ModuleSupport::Flag(m) => m,
            ModuleSupport::Unsupported => None,
        },
        duplicates: vec![],
        closed_duplicates: vec![],
    };
    let Some(loaded) = loaded else {
        return Ok(plan);
    };
    let p = &loaded.policy;
    if let Some(rule) = &p.search_before_create {
        let name = body.get("name").and_then(Value::as_str).unwrap_or("");
        if !name.trim().is_empty() {
            let (open, closed) =
                duplicate_matches(api, pid, &plan.project.clone(), rule, name, refresh)?;
            plan.checks
                .push(policy::duplicate_check(&open, allow_duplicate));
            plan.duplicates = open;
            plan.closed_duplicates = closed;
        }
    }
    if let Some(rule) = &p.description {
        let html = body.get("description_html").and_then(Value::as_str);
        plan.checks.extend(policy::check_description(rule, html));
    }
    if let Some(rule) = &p.required_fields {
        let facts = CreateFacts::from_body(body, plan.module.is_some());
        let mut check = policy::check_required(rule, &facts);
        if module == ModuleSupport::Unsupported {
            if let Some(c) = check.as_mut().filter(|c| c.message.contains("module")) {
                c.fix = Some(format!(
                    "{}; intake cannot set a module, so create it with `work-items create --module` instead",
                    c.fix.clone().unwrap_or_default()
                ));
            }
        }
        plan.checks.extend(check);
    }
    if let Some(rule) = &p.labels {
        let names = label_names(api, pid, body, refresh)?;
        plan.checks.extend(policy::check_labels(rule, &names));
    }
    if !p.state_rules.is_empty() {
        // Without `state`, Plane puts the item in the project's default state.
        let target = match body.get("state").and_then(Value::as_str) {
            Some(s) => Some(lookup_state(api, pid, s, refresh)?),
            None => api
                .fetch_states_catalog(pid, refresh)?
                .into_iter()
                .find(|s| s.is_default == Some(true)),
        };
        if let Some(target) = target {
            let who = caller(api)?;
            plan.checks.extend(policy::check_state(
                &p.state_rules,
                &target.name,
                target.group.as_deref(),
                &who.me,
                &who.label,
            ));
        }
    }
    plan.policy = Some(loaded);
    Ok(plan)
}

/// One watched field that the update changes.
#[derive(Debug, Clone)]
pub(crate) struct Change {
    pub field: &'static str,
    pub before: String,
    pub after: String,
}

pub(crate) struct UpdatePlan {
    pub project: String,
    pub policy: Option<LoadedPolicy>,
    pub checks: Vec<Check>,
    pub changes: Vec<Change>,
    pub comment_html: Option<String>,
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The comment `--reason` posts: the reason, then the watched changes.
pub(crate) fn reason_comment_html(reason: &str, changes: &[Change]) -> String {
    let mut html = format!("<p>{}</p>", html_escape(reason.trim()));
    if !changes.is_empty() {
        html.push_str("<ul>");
        for c in changes {
            html.push_str(&format!(
                "<li><strong>{}</strong>: {} → {}</li>",
                c.field,
                html_escape(&c.before),
                html_escape(&c.after)
            ));
        }
        html.push_str("</ul>");
    }
    html
}

fn member_names(api: &PlaneApi, ids: &[String]) -> Vec<String> {
    let rows = api.list_workspace_members().ok();
    let rows = rows
        .as_ref()
        .and_then(|v| {
            v.as_array()
                .or_else(|| v.get("results").and_then(Value::as_array))
        })
        .cloned()
        .unwrap_or_default();
    ids.iter()
        .map(|id| {
            rows.iter()
                .find(|r| {
                    r.get("id").and_then(Value::as_str) == Some(id.as_str())
                        || r.get("member")
                            .and_then(|m| m.get("id"))
                            .and_then(Value::as_str)
                            == Some(id.as_str())
                })
                .and_then(|r| {
                    r.get("display_name")
                        .or_else(|| r.get("member").and_then(|m| m.get("display_name")))
                        .and_then(Value::as_str)
                })
                .map(String::from)
                .unwrap_or_else(|| id.clone())
        })
        .collect()
}

fn id_list(v: Option<&Value>) -> Vec<String> {
    let mut out: Vec<String> = v
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().or_else(|| x.get("id").and_then(Value::as_str)))
                .map(str::to_lowercase)
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn list_or_none(names: &[String]) -> String {
    if names.is_empty() {
        "none".into()
    } else {
        names.join(", ")
    }
}

/// Plan an update. `always_fetch` reads the current item even without a policy or reason
/// (for `--dry-run`, so `changes` is always right).
#[allow(clippy::too_many_arguments)]
pub(crate) fn plan_update(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
    pid: &Uuid,
    iid: &Uuid,
    body: &Value,
    reason: Option<&str>,
    always_fetch: bool,
) -> Result<UpdatePlan, CoreError> {
    let (project, loaded) = load_policy(api, catalog, pid)?;
    let watches = loaded
        .as_ref()
        .is_some_and(|l| l.policy.comment_on_change.is_some() || !l.policy.state_rules.is_empty());
    let touches_watched = ["state", "priority", "assignees"]
        .iter()
        .any(|k| body.get(*k).is_some());
    let mut changes = Vec::new();
    let mut new_state: Option<String> = None;
    if touches_watched && (watches || reason.is_some() || always_fetch) {
        let current = api.get_work_item(pid, iid)?;
        if let Some(s) = body.get("state").and_then(Value::as_str) {
            let new_id = state_id_of(api, pid, s, refresh);
            let old_id = current
                .get("state")
                .and_then(|v| v.as_str().or_else(|| v.get("id").and_then(Value::as_str)))
                .unwrap_or("")
                .to_lowercase();
            if new_id.to_lowercase() != old_id {
                let before = if old_id.is_empty() {
                    "none".into()
                } else {
                    state_label(api, pid, &old_id, refresh)
                };
                new_state = Some(s.to_string());
                changes.push(Change {
                    field: "state",
                    before,
                    after: state_label(api, pid, s, refresh),
                });
            }
        }
        if let Some(p) = body.get("priority").and_then(Value::as_str) {
            let old = current
                .get("priority")
                .and_then(Value::as_str)
                .unwrap_or("none");
            if !p.eq_ignore_ascii_case(old) {
                changes.push(Change {
                    field: "priority",
                    before: old.into(),
                    after: p.into(),
                });
            }
        }
        if body.get("assignees").is_some() {
            let new = id_list(body.get("assignees"));
            let old = id_list(current.get("assignees"));
            if new != old {
                changes.push(Change {
                    field: "assignees",
                    before: list_or_none(&member_names(api, &old)),
                    after: list_or_none(&member_names(api, &new)),
                });
            }
        }
    }
    let changed: Vec<&str> = changes.iter().map(|c| c.field).collect();
    let mut checks = Vec::new();
    if let Some(l) = &loaded {
        let p = &l.policy;
        if let Some(rule) = &p.description {
            if let Some(html) = body.get("description_html").and_then(Value::as_str) {
                checks.extend(policy::check_description(rule, Some(html)));
            }
        }
        if let Some(rule) = &p.labels {
            let names = label_names(api, pid, body, refresh)?;
            checks.extend(policy::check_labels(rule, &names));
        }
        if let Some(rule) = &p.comment_on_change {
            checks.extend(policy::check_comment_on_change(rule, &changed, reason));
        }
        if let (false, Some(raw)) = (p.state_rules.is_empty(), &new_state) {
            let target = lookup_state(api, pid, raw, refresh)?;
            let who = caller(api)?;
            checks.extend(policy::check_state(
                &p.state_rules,
                &target.name,
                target.group.as_deref(),
                &who.me,
                &who.label,
            ));
        }
    }
    let comment_html = reason
        .filter(|r| !r.trim().is_empty())
        .map(|r| reason_comment_html(r, &changes));
    Ok(UpdatePlan {
        project,
        policy: loaded,
        checks,
        changes,
        comment_html,
    })
}

/// `no_deletes` for a delete of `what` in project `pid`.
pub(crate) fn plan_delete(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    pid: &Uuid,
    what: &str,
) -> Result<(String, Option<LoadedPolicy>, Vec<Check>), CoreError> {
    let (project, loaded) = load_policy(api, catalog, pid)?;
    let mut checks = Vec::new();
    if let Some(rule) = loaded.as_ref().and_then(|l| l.policy.deletes.as_ref()) {
        let who = caller(api)?;
        checks.push(policy::check_delete(rule, what, &who.me, &who.label));
    }
    Ok((project, loaded, checks))
}

/// `no_deletes` for archiving a project or module (`what`) in project `pid`: archiving
/// hides work, so a `[deletes]` rule covers it. Unarchiving only restores, so it is not gated.
pub(crate) fn gate_archive(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    pid: &Uuid,
    what: &str,
    action: &str,
) -> Result<(), CoreError> {
    let (_, loaded) = load_policy(api, catalog, pid)?;
    let mut checks = Vec::new();
    if let Some(rule) = loaded.as_ref().and_then(|l| l.policy.deletes.as_ref()) {
        let who = caller(api)?;
        checks.push(policy::check_archive(rule, what, &who.me, &who.label));
    }
    enforce(loaded.as_ref(), action, &checks)
}

/// Refuse the write with `policy_violation` when any check failed.
pub(crate) fn enforce(
    loaded: Option<&LoadedPolicy>,
    action: &str,
    checks: &[Check],
) -> Result<(), CoreError> {
    match loaded {
        Some(l) if checks.iter().any(Check::failed) => {
            Err(policy::violation_error(l, action, checks))
        }
        _ => Ok(()),
    }
}

/// Delete gate for the commands without `--dry-run`.
pub(crate) fn gate_delete(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    pid: &Uuid,
    what: &str,
    action: &str,
) -> Result<(), CoreError> {
    let (_, loaded, checks) = plan_delete(api, catalog, pid, what)?;
    enforce(loaded.as_ref(), action, &checks)
}

/// `--dry-run` report: nothing was written. `allowed` is false when a rule would refuse it.
pub(crate) fn dry_run_report(
    action: &str,
    project: &str,
    loaded: Option<&LoadedPolicy>,
    checks: &[Check],
    request: Value,
    then: Vec<Value>,
) -> Value {
    json!({
        "dry_run": true,
        "action": action,
        "project": project,
        "allowed": !checks.iter().any(Check::failed),
        "policy": loaded.map(|l| json!({
            "file": l.file().path.display().to_string(),
            "source": l.file().source,
            "files": l.files_json(),
        })),
        "checks": checks,
        "request": request,
        "then": then,
    })
}

// ----- write runners used by `work-items create|update|delete` -----

#[derive(clap::Args, Clone, Debug, Default)]
pub struct CreatePolicyArgs {
    #[arg(
        long,
        value_name = "MODULE",
        value_hint = clap::ValueHint::Other,
        help = "Add the new work item to this module (UUID or exact name)"
    )]
    pub module: Option<String>,
    #[arg(
        long,
        help = "Create even when the project policy finds an open work item with a close name"
    )]
    pub allow_duplicate: bool,
    #[arg(
        long,
        help = "Show the request and the policy checks without writing anything"
    )]
    pub dry_run: bool,
}

#[derive(clap::Args, Clone, Debug, Default)]
pub struct UpdatePolicyArgs {
    #[arg(
        long,
        value_name = "TEXT",
        help = "Why: posted as a comment after the update (plain text); the project policy can require it for state, assignee or priority changes"
    )]
    pub reason: Option<String>,
    #[arg(
        long,
        help = "Show the request, the comment and the policy checks without writing anything"
    )]
    pub dry_run: bool,
}

fn api_path(api: &PlaneApi, tail: &str) -> String {
    format!(
        "/api/v1/workspaces/{}/projects/{tail}",
        api.config.workspace_slug
    )
}

fn composite(project: &str, item: &Value) -> String {
    match item.get("sequence_id").and_then(Value::as_i64) {
        Some(n) => format!("{project}-{n}"),
        None => item
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    }
}

/// Warnings for close matches that did not refuse the create.
pub(crate) fn duplicate_warnings(plan: &CreatePlan) -> Vec<String> {
    let ids = |v: &[Value]| -> String {
        v.iter()
            .filter_map(|m| m["work_item"].as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut out = Vec::new();
    if !plan.duplicates.is_empty() {
        out.push(format!(
            "created with --allow-duplicate although {} has a close name",
            ids(&plan.duplicates)
        ));
    }
    if !plan.closed_duplicates.is_empty() {
        out.push(format!(
            "{} (Done or Cancelled) has a close name; check it is not the same work",
            ids(&plan.closed_duplicates)
        ));
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_create(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
    json_mode: bool,
    pid: Uuid,
    body: Value,
    args: &CreatePolicyArgs,
) -> Result<(), CoreError> {
    let action = "work-items create";
    let module = match args.module.as_deref() {
        // Work items can only be added to active modules.
        Some(m) => Some(super::modules::resolve_module(
            api,
            &pid,
            m,
            NameScope::Active,
        )?),
        None => None,
    };
    let plan = plan_create(
        api,
        catalog,
        refresh,
        &pid,
        &body,
        ModuleSupport::Flag(module),
        args.allow_duplicate,
    )?;
    if args.dry_run {
        let then = plan
            .module
            .map(|m| vec![json!({"step": "module_add", "module": m.to_string()})])
            .unwrap_or_default();
        let request = json!({
            "method": "POST",
            "path": api_path(api, &format!("{pid}/work-items/")),
            "body": body,
        });
        let mut report = dry_run_report(
            action,
            &plan.project,
            plan.policy.as_ref(),
            &plan.checks,
            request,
            then,
        );
        report["closed_duplicates"] = json!(plan.closed_duplicates);
        return crate::output::emit_success(json_mode, report, None, false);
    }
    enforce(plan.policy.as_ref(), action, &plan.checks)?;
    let mut v = api.create_work_item(&pid, &body)?;
    let warnings = duplicate_warnings(&plan);
    if let Some(mid) = plan.module {
        let iid = v
            .get("id")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| CoreError::UnexpectedResponse("created work item has no id".into()))?;
        if let Err(e) = api.add_module_work_items(&pid, &mid, &[iid]) {
            let created = composite(&plan.project, &v);
            return Err(e.with_details(
                &format!("{created} was created but not added to the module"),
                json!({"step": "module_add", "created": created, "id": iid.to_string(), "module": mid.to_string()}),
            ));
        }
        if let Some(obj) = v.as_object_mut() {
            obj.insert("module_id".into(), json!(mid.to_string()));
        }
    }
    crate::output::emit_success_with_warnings(json_mode, v, None, false, warnings)
}

/// Post the `--reason` comment after a successful update. A failure is not an error (the
/// update stands): `reason_comment` is null, `reason_comment_error` says why, plus a warning.
pub(crate) fn post_reason_comment(
    api: &PlaneApi,
    pid: &Uuid,
    iid: &Uuid,
    html: &str,
    target: &mut Value,
    warnings: &mut Vec<String>,
) {
    let Some(obj) = target.as_object_mut() else {
        return;
    };
    match api.create_comment(pid, iid, &json!({"comment_html": html})) {
        Ok(comment) => {
            obj.insert(
                "reason_comment".into(),
                json!({"id": comment.get("id").cloned().unwrap_or(Value::Null), "comment_html": html}),
            );
        }
        Err(e) => {
            warnings.push(format!(
                "the update was applied but the --reason comment was not posted ({}); post it with `work-items comments create`",
                e.error_code()
            ));
            obj.insert("reason_comment".into(), Value::Null);
            obj.insert(
                "reason_comment_error".into(),
                json!({"step": "reason_comment", "code": e.error_code(), "comment_html": html}),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_update(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
    json_mode: bool,
    pid: Uuid,
    iid: Uuid,
    body: Value,
    args: &UpdatePolicyArgs,
) -> Result<(), CoreError> {
    let action = "work-items update";
    let plan = plan_update(
        api,
        catalog,
        refresh,
        &pid,
        &iid,
        &body,
        args.reason.as_deref(),
        args.dry_run,
    )?;
    if args.dry_run {
        let then = plan
            .comment_html
            .as_ref()
            .map(|c| vec![json!({"step": "comment", "comment_html": c})])
            .unwrap_or_default();
        let request = json!({
            "method": "PATCH",
            "path": api_path(api, &format!("{pid}/work-items/{iid}/")),
            "body": body,
        });
        let mut report = dry_run_report(
            action,
            &plan.project,
            plan.policy.as_ref(),
            &plan.checks,
            request,
            then,
        );
        report["changes"] = json!(plan
            .changes
            .iter()
            .map(|c| json!({"field": c.field, "before": c.before, "after": c.after}))
            .collect::<Vec<_>>());
        return crate::output::emit_success(json_mode, report, None, false);
    }
    enforce(plan.policy.as_ref(), action, &plan.checks)?;
    let mut v = api.update_work_item(&pid, &iid, &body)?;
    let mut warnings = Vec::new();
    if let Some(html) = &plan.comment_html {
        post_reason_comment(api, &pid, &iid, html, &mut v, &mut warnings);
    }
    crate::output::emit_success_with_warnings(json_mode, v, None, false, warnings)
}

pub(super) fn run_delete(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    json_mode: bool,
    pid: Uuid,
    iid: Uuid,
    dry_run: bool,
) -> Result<(), CoreError> {
    let action = "work-items delete";
    let (project, loaded, checks) = plan_delete(api, catalog, &pid, "work item")?;
    if dry_run {
        let request = json!({
            "method": "DELETE",
            "path": api_path(api, &format!("{pid}/work-items/{iid}/")),
        });
        let report = dry_run_report(action, &project, loaded.as_ref(), &checks, request, vec![]);
        return crate::output::emit_success(json_mode, report, None, false);
    }
    enforce(loaded.as_ref(), action, &checks)?;
    api.delete_work_item(&pid, &iid)?;
    crate::output::emit_empty_success(json_mode)
}

// ----- TUI -----

/// Policy check for a TUI edit, run in the API worker before the PATCH. The TUI has no
/// reason prompt, so a change the policy wants a reason for is refused with a pointer to the
/// CLI. `Err` is the status-line text.
pub(crate) fn tui_check_update(
    api: &mut PlaneApi,
    pid: &Uuid,
    iid: &Uuid,
    body: &Value,
) -> Result<(), String> {
    let catalog = api
        .fetch_projects_catalog(false)
        .map_err(|e| e.to_string())?;
    let plan = plan_update(api, &catalog, false, pid, iid, body, None, false)
        .map_err(|e| e.to_string())?;
    tui_refusal(
        &plan.project,
        &plan.checks,
        "work-items update --reason \"…\"",
    )
}

pub(crate) fn tui_check_delete(api: &mut PlaneApi, pid: &Uuid) -> Result<(), String> {
    let catalog = api
        .fetch_projects_catalog(false)
        .map_err(|e| e.to_string())?;
    let (project, _, checks) =
        plan_delete(api, &catalog, pid, "work item").map_err(|e| e.to_string())?;
    tui_refusal(&project, &checks, "work-items delete")
}

/// Status-line text for a TUI write the policy refuses (`policy:` prefix, never sent to Sentry).
pub(crate) const TUI_POLICY_PREFIX: &str = "policy: ";

fn tui_refusal(project: &str, checks: &[Check], cli: &str) -> Result<(), String> {
    let failed: Vec<&Check> = checks.iter().filter(|c| c.failed()).collect();
    let Some(first) = failed.first() else {
        return Ok(());
    };
    let hint = if first.rule == policy::RULE_COMMENT {
        format!("use `plane-cli {cli}`, which posts the reason as a comment")
    } else {
        first.fix.clone().unwrap_or_default()
    };
    Err(format!(
        "{TUI_POLICY_PREFIX}{project} refused this ({}: {}); {hint}",
        first.rule, first.message
    ))
}

// ----- intake -----

/// Policy for `intake create`: the create checks on `issue` (intake cannot set a module).
pub(super) fn gate_intake_create(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
    pid: &Uuid,
    issue: &Value,
) -> Result<Vec<String>, CoreError> {
    let plan = plan_create(
        api,
        catalog,
        refresh,
        pid,
        issue,
        ModuleSupport::Unsupported,
        false,
    )?;
    enforce(plan.policy.as_ref(), "intake create", &plan.checks)?;
    Ok(duplicate_warnings(&plan))
}

/// Policy for `intake update`: the update checks on `issue` fields (description, priority
/// with `comment_on_change`). Returns the `--reason` comment to post afterwards.
pub(super) fn gate_intake_update(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
    pid: &Uuid,
    iid: &Uuid,
    issue: &Value,
    reason: Option<&str>,
) -> Result<Option<String>, CoreError> {
    let plan = plan_update(api, catalog, refresh, pid, iid, issue, reason, false)?;
    enforce(plan.policy.as_ref(), "intake update", &plan.checks)?;
    Ok(plan.comment_html)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A `[deletes]` rule that only lets `alice` delete or archive.
    pub(crate) const DENY_DELETES: &str = "version = 1\n[deletes]\nmembers = [\"alice\"]\n";

    /// A temporary XDG_CONFIG_HOME with an optional policy file for `project`; restores the
    /// env on drop. Hold `lock_env()` for its lifetime.
    pub(crate) struct PolicyEnv {
        _xdg: tempfile::TempDir,
        old: Option<std::ffi::OsString>,
    }

    impl PolicyEnv {
        pub(crate) fn new(project: &str, policy_toml: Option<&str>) -> Self {
            let xdg = tempfile::tempdir().unwrap();
            if let Some(t) = policy_toml {
                let dir = xdg.path().join("plane-cli").join("policy");
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join(format!("{project}.toml")), t).unwrap();
            }
            let old = std::env::var_os("XDG_CONFIG_HOME");
            std::env::set_var("XDG_CONFIG_HOME", xdg.path());
            PolicyEnv { _xdg: xdg, old }
        }
    }

    impl Drop for PolicyEnv {
        fn drop(&mut self) {
            match &self.old {
                Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
    }

    /// Mocks `users/me` as `bob` (not allowed by [`DENY_DELETES`]).
    pub(crate) fn mock_me(server: &mut mockito::ServerGuard) {
        server
            .mock("GET", "/api/v1/users/me/")
            .with_body(r#"{"id":"55555555-5555-4555-8555-555555555555","display_name":"bob"}"#)
            .create();
    }

    #[test]
    fn reason_comment_escapes_and_lists_changes() {
        let html = reason_comment_html(
            "  moved <now> & why ",
            &[Change {
                field: "state",
                before: "Todo".into(),
                after: "In Progress".into(),
            }],
        );
        assert_eq!(
            html,
            "<p>moved &lt;now&gt; &amp; why</p><ul><li><strong>state</strong>: Todo → In Progress</li></ul>"
        );
        assert_eq!(reason_comment_html("x", &[]), "<p>x</p>");
    }

    #[test]
    fn id_lists_compare_as_sets() {
        let a = id_list(Some(&json!(["B", "a"])));
        let b = id_list(Some(&json!([{"id": "a"}, "b"])));
        assert_eq!(a, b);
    }

    #[test]
    fn tui_refusal_points_reason_changes_at_the_cli() {
        let checks = vec![Check::fail(
            policy::RULE_COMMENT,
            "this changes state without --reason",
            "add --reason",
        )];
        let e = tui_refusal("WEB", &checks, "work-items update --reason \"…\"").unwrap_err();
        assert!(e.starts_with(TUI_POLICY_PREFIX));
        assert!(e.contains("--reason"));
        assert!(tui_refusal("WEB", &[], "x").is_ok());
    }

    // ----- orchestration against a mock Plane -----

    use mockito::Matcher;

    const PID: &str = "211751d8-8fc6-4df8-9b1d-757550e5180b";
    const IID: &str = "11111111-1111-4111-8111-111111111111";
    const BACKLOG: &str = "22222222-2222-4222-8222-222222222222";
    const TODO: &str = "33333333-3333-4333-8333-333333333333";
    const DONE: &str = "44444444-4444-4444-8444-444444444444";
    const BOB: &str = "55555555-5555-4555-8555-555555555555";

    /// A policy file under a temporary XDG_CONFIG_HOME plus a mock API; restores the env on
    /// drop. Hold `lock_env()` for its lifetime.
    struct Fixture {
        server: mockito::ServerGuard,
        mock: crate::identity::tests::MockApi,
        _xdg: tempfile::TempDir,
        old_xdg: Option<std::ffi::OsString>,
    }

    impl Fixture {
        fn new(policy_toml: &str) -> Self {
            let xdg = tempfile::tempdir().unwrap();
            let dir = xdg.path().join("plane-cli").join("policy");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("WEB.toml"), policy_toml).unwrap();
            let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
            std::env::set_var("XDG_CONFIG_HOME", xdg.path());
            let mut server = mockito::Server::new();
            server
                .mock("GET", "/api/v1/users/me/")
                .with_body(format!(r#"{{"id":"{BOB}","display_name":"bob"}}"#))
                .create();
            server
                .mock(
                    "GET",
                    Matcher::Regex(format!("^/api/v1/workspaces/ws/projects/{PID}/states/")),
                )
                .with_body(format!(
                    r#"{{"results":[
                        {{"id":"{BACKLOG}","name":"Backlog","group":"backlog","default":true}},
                        {{"id":"{TODO}","name":"Todo","group":"unstarted","default":false}},
                        {{"id":"{DONE}","name":"Done","group":"completed","default":false}}
                    ],"next_page_results":false}}"#
                ))
                .create();
            server
                .mock("GET", "/api/v1/workspaces/ws/members/")
                .with_body(format!(r#"[{{"id":"{BOB}","display_name":"bob"}}]"#))
                .create();
            server
                .mock(
                    "GET",
                    format!("/api/v1/workspaces/ws/projects/{PID}/work-items/{IID}/").as_str(),
                )
                .with_body(format!(
                    r#"{{"id":"{IID}","sequence_id":7,"state":"{BACKLOG}","priority":"low","assignees":[]}}"#
                ))
                .create();
            let mock = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
            Fixture {
                server,
                mock,
                _xdg: xdg,
                old_xdg,
            }
        }

        fn catalog() -> Vec<ProjectEntry> {
            vec![ProjectEntry {
                id: Uuid::parse_str(PID).unwrap(),
                name: "Lab".into(),
                identifier: "WEB".into(),
            }]
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            match &self.old_xdg {
                Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
        }
    }

    fn pid() -> Uuid {
        Uuid::parse_str(PID).unwrap()
    }
    fn iid() -> Uuid {
        Uuid::parse_str(IID).unwrap()
    }

    const POLICY: &str = r#"version = 1
[comment_on_change]
fields = ["state", "assignees", "priority"]
[[state_rules]]
groups = ["unstarted", "completed", "backlog"]
members = ["alice"]
[deletes]
members = ["alice"]
"#;

    #[test]
    fn body_fields_count_as_changes_and_need_reason_and_permission() {
        let _g = crate::plane_env_files::lock_env();
        let mut f = Fixture::new(POLICY);
        let body = json!({"state": TODO, "priority": "high", "assignees": [BOB]});
        let plan = plan_update(
            &mut f.mock.api,
            &Fixture::catalog(),
            false,
            &pid(),
            &iid(),
            &body,
            None,
            false,
        )
        .unwrap();
        let fields: Vec<&str> = plan.changes.iter().map(|c| c.field).collect();
        assert_eq!(fields, vec!["state", "priority", "assignees"]);
        let failed: Vec<&str> = plan
            .checks
            .iter()
            .filter(|c| c.failed())
            .map(|c| c.rule)
            .collect();
        assert_eq!(failed, vec![policy::RULE_COMMENT, policy::RULE_STATES]);
        let e = enforce(plan.policy.as_ref(), "work-items update", &plan.checks).unwrap_err();
        assert_eq!(e.error_code(), "policy_violation");
        assert_eq!(e.exit_kind().code(), 2);
        let _ = &mut f.server;
    }

    #[test]
    fn create_without_state_is_checked_against_the_default_state() {
        let _g = crate::plane_env_files::lock_env();
        let mut f = Fixture::new(POLICY);
        let body = json!({"name": "x"});
        let plan = plan_create(
            &mut f.mock.api,
            &Fixture::catalog(),
            false,
            &pid(),
            &body,
            ModuleSupport::Flag(None),
            false,
        )
        .unwrap();
        let c = plan
            .checks
            .iter()
            .find(|c| c.rule == policy::RULE_STATES)
            .unwrap();
        assert!(c.failed() && c.message.contains("Backlog"), "{c:?}");
    }

    #[test]
    fn unknown_state_fails_closed() {
        let _g = crate::plane_env_files::lock_env();
        let mut f = Fixture::new(POLICY);
        let body = json!({"state": "66666666-6666-4666-8666-666666666666"});
        let err = plan_update(
            &mut f.mock.api,
            &Fixture::catalog(),
            false,
            &pid(),
            &iid(),
            &body,
            Some("why"),
            false,
        )
        .err()
        .expect("unknown state must refuse");
        assert_eq!(err.error_code(), "policy_unknown_state");
    }

    #[test]
    fn deletes_of_comments_links_attachments_are_gated() {
        let _g = crate::plane_env_files::lock_env();
        let mut f = Fixture::new(POLICY);
        for what in ["comment", "link", "attachment", "intake item"] {
            let e = gate_delete(
                &mut f.mock.api,
                &Fixture::catalog(),
                &pid(),
                what,
                "x delete",
            )
            .unwrap_err();
            assert_eq!(e.error_code(), "policy_violation", "{what}");
        }
    }

    #[test]
    fn archives_of_projects_and_modules_are_gated_by_deletes() {
        let _g = crate::plane_env_files::lock_env();
        let mut f = Fixture::new(POLICY);
        for what in ["project", "module"] {
            let e = gate_archive(
                &mut f.mock.api,
                &Fixture::catalog(),
                &pid(),
                what,
                "x archive",
            )
            .unwrap_err();
            assert_eq!(e.error_code(), "policy_violation", "{what}");
            assert!(e.to_string().contains("may not archive"), "{e}");
        }
        drop(f);
        let mut f = Fixture::new("version = 1\n");
        gate_archive(
            &mut f.mock.api,
            &Fixture::catalog(),
            &pid(),
            "project",
            "projects archive",
        )
        .unwrap();
    }

    #[test]
    fn dry_runs_write_nothing() {
        let _g = crate::plane_env_files::lock_env();
        let mut f = Fixture::new(POLICY);
        let patch = f.server.mock("PATCH", Matcher::Any).expect(0).create();
        let delete = f.server.mock("DELETE", Matcher::Any).expect(0).create();
        let post = f.server.mock("POST", Matcher::Any).expect(0).create();
        let args = UpdatePolicyArgs {
            reason: Some("because".into()),
            dry_run: true,
        };
        run_update(
            &mut f.mock.api,
            &Fixture::catalog(),
            false,
            true,
            pid(),
            iid(),
            json!({"priority": "high"}),
            &args,
        )
        .unwrap();
        run_delete(
            &mut f.mock.api,
            &Fixture::catalog(),
            true,
            pid(),
            iid(),
            true,
        )
        .unwrap();
        patch.assert();
        delete.assert();
        post.assert();
    }

    #[test]
    fn dry_run_reads_current_item_without_a_policy() {
        let _g = crate::plane_env_files::lock_env();
        let mut f = Fixture::new("version = 1\n");
        let plan = plan_update(
            &mut f.mock.api,
            &Fixture::catalog(),
            false,
            &pid(),
            &iid(),
            &json!({"priority": "high"}),
            None,
            true,
        )
        .unwrap();
        assert_eq!(plan.changes.len(), 1);
        assert_eq!(plan.changes[0].before, "low");
    }

    #[test]
    fn a_failed_reason_comment_is_a_warning_not_an_error() {
        let _g = crate::plane_env_files::lock_env();
        let mut f = Fixture::new("version = 1\n");
        f.server
            .mock(
                "POST",
                format!("/api/v1/workspaces/ws/projects/{PID}/work-items/{IID}/comments/").as_str(),
            )
            .with_status(500)
            .with_body(r#"{"detail":"boom"}"#)
            .create();
        let mut v = json!({"id": IID});
        let mut warnings = Vec::new();
        post_reason_comment(
            &f.mock.api,
            &pid(),
            &iid(),
            "<p>x</p>",
            &mut v,
            &mut warnings,
        );
        assert_eq!(v["reason_comment"], Value::Null);
        assert_eq!(v["reason_comment_error"]["step"], "reason_comment");
        assert_eq!(v["reason_comment_error"]["code"], "server_error");
        assert_eq!(warnings.len(), 1);
    }
}
