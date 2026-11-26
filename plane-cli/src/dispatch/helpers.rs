use super::cycles::CycleItemsCmd;
use super::AttachmentsCmd;
use super::{
    ActivitiesCmd, Cli, Commands, CommentsCmd, CyclesCmd, LinksCmd, ModuleIssuesCmd, ModulesCmd,
    RelationsCmd, WorkItemTarget, WorkItemsCmd,
};
use plane_core::{
    normalize_work_item_body, parse_composite_work_item, push_warning, resolve_project_id,
    CoreError, PlaneApi, ProjectEntry,
};
use serde_json::Value;
use uuid::Uuid;

/// Resolve a project identifier, auto-refreshing the catalog once on cache miss.
pub(super) fn resolve_project_auto_refresh(
    api: &mut PlaneApi,
    project: &str,
    catalog: &[ProjectEntry],
    refresh: bool,
    warnings: &mut Vec<String>,
) -> Result<Uuid, CoreError> {
    match resolve_project_id(project, catalog) {
        Ok(uuid) => Ok(uuid),
        Err(CoreError::Resolve(msg)) if msg.contains("unknown project") && !refresh => {
            let fresh = api.fetch_projects_catalog(true)?;
            match resolve_project_id(project, &fresh) {
                Ok(uuid) => {
                    warnings.push(
                        "project catalog was stale; refreshed automatically (consider --refresh on next call)".into(),
                    );
                    Ok(uuid)
                }
                Err(_) => Err(CoreError::Resolve(msg)),
            }
        }
        Err(e) => Err(e),
    }
}

pub(super) fn issue_project_uuid(v: &Value) -> Result<Uuid, CoreError> {
    v.get("project")
        .and_then(|x| x.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| CoreError::Usage("work item JSON missing string project UUID".into()))
}

pub(super) fn issue_id_uuid(v: &Value) -> Result<Uuid, CoreError> {
    v.get("id")
        .and_then(|x| x.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| CoreError::Usage("work item JSON missing id".into()))
}

/// How a `--work-item` value addresses its work item.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum WorkItemRef<'a> {
    /// `ENG-123`: the project comes from the prefix; `--project` is optional.
    Composite {
        prefix: String,
        seq: u64,
        project: Option<&'a str>,
    },
    /// Bare UUID: `--project` is required.
    Uuid { project: &'a str, id: Uuid },
}

/// Classify a work item value without touching the API.
pub(super) fn classify_work_item<'a>(
    project_arg: Option<&'a str>,
    work_item: &str,
) -> Result<WorkItemRef<'a>, CoreError> {
    // UUID first: the composite pattern also matches a UUID whose last group is all digits.
    let Ok(id) = Uuid::parse_str(work_item.trim()) else {
        let (prefix, seq) = parse_composite_work_item(work_item).ok_or_else(|| {
            CoreError::Usage(format!(
                "invalid work-item value {work_item:?}; expected a composite id (PROJ-123) or a UUID"
            ))
        })?;
        return Ok(WorkItemRef::Composite {
            prefix,
            seq,
            project: project_arg,
        });
    };
    let project = project_arg.ok_or_else(|| {
        CoreError::usage_code(
            "project_required_for_uuid",
            format!(
                "--project is required when --work-item is a UUID ({work_item}); \
                 a PREFIX-N id such as ENG-123 works without --project"
            ),
        )
    })?;
    Ok(WorkItemRef::Uuid { project, id })
}

/// Refuse when `--project` names a different project than the composite id's prefix.
pub(super) fn ensure_same_project(
    flag_project_id: Uuid,
    item_project_id: Uuid,
    project_arg: &str,
    work_item: &str,
    prefix: &str,
    project_required: bool,
) -> Result<(), CoreError> {
    if flag_project_id == item_project_id {
        return Ok(());
    }
    let advice = if project_required {
        format!("make --project match the prefix (--project {prefix})")
    } else {
        format!("drop --project or pass --project {prefix}")
    };
    Err(CoreError::usage_code(
        "project_mismatch",
        format!(
            "--project {project_arg} does not match --work-item {work_item} (project {prefix}); {advice}"
        ),
    ))
}

/// Fetch a work item by composite id or (project, UUID), refusing a mismatched `--project`.
pub(super) fn get_work_item_checked(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
    project_arg: Option<&str>,
    work_item: &str,
    warnings: &mut Vec<String>,
) -> Result<Value, CoreError> {
    match classify_work_item(project_arg, work_item)? {
        WorkItemRef::Composite {
            prefix,
            seq,
            project,
        } => {
            let v = api.get_work_item_by_composite(&prefix, seq)?;
            if let Some(p) = project {
                let flag_pid = resolve_project_auto_refresh(api, p, catalog, refresh, warnings)?;
                ensure_same_project(
                    flag_pid,
                    issue_project_uuid(&v)?,
                    p,
                    work_item,
                    &prefix,
                    false,
                )?;
            }
            Ok(v)
        }
        WorkItemRef::Uuid { project, id } => {
            let pid = resolve_project_auto_refresh(api, project, catalog, refresh, warnings)?;
            api.get_work_item(&pid, &id)
        }
    }
}

/// Resolve (project_id, issue_uuid) for sub-resources.
///
/// Composite ids carry their project, so `project_arg` is optional for them; when given it
/// must match. UUIDs need `project_arg`. `project_required` is true where the command itself
/// requires `--project` (e.g. `modules issues remove`), which changes the mismatch advice.
pub(super) fn resolve_work_item_pair(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
    project_arg: Option<&str>,
    work_item: &str,
    project_required: bool,
) -> Result<(Uuid, Uuid), CoreError> {
    let mut warnings = Vec::new();
    let out = match classify_work_item(project_arg, work_item)? {
        WorkItemRef::Composite {
            prefix,
            seq,
            project,
        } => {
            let v = api.get_work_item_by_composite(&prefix, seq)?;
            let iid = issue_id_uuid(&v)?;
            let pid = issue_project_uuid(&v)?;
            if let Some(p) = project {
                let flag_pid =
                    resolve_project_auto_refresh(api, p, catalog, refresh, &mut warnings)?;
                ensure_same_project(flag_pid, pid, p, work_item, &prefix, project_required)?;
            }
            (pid, iid)
        }
        WorkItemRef::Uuid { project, id } => {
            let pid = resolve_project_auto_refresh(api, project, catalog, refresh, &mut warnings)?;
            (pid, id)
        }
    };
    for w in warnings {
        push_warning(w);
    }
    Ok(out)
}

/// The work item named by `--work-item` or the hidden positional, plus the warning to emit.
/// No side effects, so it can run before config/network (see [`prevalidate`]).
fn target_work_item_checked(
    target: &WorkItemTarget,
) -> Result<(String, Option<String>), CoreError> {
    match (
        target.work_item.as_deref(),
        target.work_item_positional.as_deref(),
    ) {
        (Some(flag), None) => Ok((flag.to_string(), None)),
        (Some(flag), Some(pos)) if flag == pos => Ok((
            flag.to_string(),
            Some(format!(
                "work item {pos:?} given both as --work-item and positionally; drop the positional"
            )),
        )),
        (Some(flag), Some(pos)) => Err(CoreError::Usage(format!(
            "work item given twice: --work-item {flag} and positional {pos:?}; pass only --work-item"
        ))),
        (None, Some(pos)) => Ok((
            pos.to_string(),
            Some(format!(
                "work item {pos:?} passed as positional arg; use --work-item {pos} to suppress this warning"
            )),
        )),
        (None, None) => Err(CoreError::Usage(
            "--work-item <ITEM> is required (composite id like PROJ-123, or a UUID with --project)"
                .into(),
        )),
    }
}

/// The work item named by `--work-item` or, with a warning, the hidden positional.
pub(super) fn target_work_item(target: &WorkItemTarget) -> Result<String, CoreError> {
    let (work_item, warning) = target_work_item_checked(target)?;
    if let Some(w) = warning {
        push_warning(w);
    }
    Ok(work_item)
}

/// Resolve a [`WorkItemTarget`] to (project_id, issue_uuid).
pub(super) fn resolve_target(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
    target: &WorkItemTarget,
) -> Result<(Uuid, Uuid), CoreError> {
    let work_item = target_work_item(target)?;
    resolve_work_item_pair(
        api,
        catalog,
        refresh,
        target.project.as_deref(),
        &work_item,
        false,
    )
}

/// The [`WorkItemTarget`] of a command, if it takes one.
fn command_target(cmd: &Commands) -> Option<&WorkItemTarget> {
    let Commands::WorkItems { cmd } = cmd else {
        return None;
    };
    Some(match cmd {
        WorkItemsCmd::Update { target, .. } | WorkItemsCmd::Delete { target, .. } => target,
        WorkItemsCmd::Activities { cmd } => match cmd {
            ActivitiesCmd::List { target, .. } | ActivitiesCmd::Get { target, .. } => target,
        },
        WorkItemsCmd::Comments { cmd } => match cmd {
            CommentsCmd::List { target, .. }
            | CommentsCmd::Get { target, .. }
            | CommentsCmd::Create { target, .. }
            | CommentsCmd::Update { target, .. }
            | CommentsCmd::Delete { target, .. } => target,
        },
        WorkItemsCmd::Links { cmd } => match cmd {
            LinksCmd::List { target, .. }
            | LinksCmd::Get { target, .. }
            | LinksCmd::Create { target, .. }
            | LinksCmd::Update { target, .. }
            | LinksCmd::Delete { target, .. } => target,
        },
        WorkItemsCmd::Relations { cmd } => match cmd {
            RelationsCmd::List { target } | RelationsCmd::Create { target, .. } => target,
        },
        WorkItemsCmd::Attachments { cmd } => match cmd {
            AttachmentsCmd::List { target }
            | AttachmentsCmd::Get { target, .. }
            | AttachmentsCmd::Create { target, .. }
            | AttachmentsCmd::Delete { target, .. } => target,
        },
        _ => return None,
    })
}

/// Work-item usage errors that need no config or network: missing `--work-item`, a value that
/// is neither `PREFIX-N` nor a UUID, a UUID without `--project`. Run before loading config so
/// these exit 2 even when Plane is unreachable or unconfigured.
/// `data` for archive and unarchive commands: Plane answers 204, so plane-cli reports
/// `{ "id", "archived" }`; a non-empty response body is passed through.
pub(super) fn archived_data(response: Value, id: &Uuid, archived: bool) -> Value {
    if response.is_null() {
        serde_json::json!({ "id": id.to_string(), "archived": archived })
    } else {
        response
    }
}

pub fn prevalidate(cli: &Cli) -> Result<(), CoreError> {
    if let Some(target) = command_target(&cli.command) {
        let (work_item, _) = target_work_item_checked(target)?;
        classify_work_item(target.project.as_deref(), &work_item)?;
        return Ok(());
    }
    match &cli.command {
        Commands::WorkItems {
            cmd:
                WorkItemsCmd::Get {
                    work_item,
                    project,
                    positional,
                },
        } => {
            let mut ignored = Vec::new();
            let (wi, proj) = super::resolve_get_args(
                work_item.as_deref(),
                project.as_deref(),
                positional,
                &mut ignored,
            )?;
            classify_work_item(proj, &wi)?;
        }
        Commands::Modules {
            cmd:
                ModulesCmd::Issues {
                    cmd:
                        ModuleIssuesCmd::Add {
                            project,
                            work_items,
                            ..
                        }
                        | ModuleIssuesCmd::Remove {
                            project,
                            work_items,
                            ..
                        },
                },
        } => classify_work_items(project, work_items)?,
        Commands::Cycles {
            cmd:
                CyclesCmd::Items {
                    cmd:
                        CycleItemsCmd::Add {
                            project,
                            work_items,
                            ..
                        }
                        | CycleItemsCmd::Remove {
                            project,
                            work_items,
                            ..
                        },
                },
        } => classify_work_items(project, work_items)?,
        _ => {}
    }
    Ok(())
}

/// Check every `--work-items` entry is `PREFIX-N` or a UUID (with `--project`), before any
/// config or network work.
fn classify_work_items(project: &str, items: &[String]) -> Result<(), CoreError> {
    let mut any = false;
    for item in items.iter().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        classify_work_item(Some(project), item)?;
        any = true;
    }
    if !any {
        return Err(CoreError::Usage(
            "--work-items needs at least one id (PROJ-123 or UUID, comma-separated)".into(),
        ));
    }
    Ok(())
}

pub(super) fn normalize_work_item_if_strict(
    api: &mut PlaneApi,
    project_id: Uuid,
    body: Value,
    strict_catalog: bool,
    refresh: bool,
) -> Result<Value, CoreError> {
    if !strict_catalog {
        return Ok(body);
    }
    let states = api.fetch_states_catalog(&project_id, refresh)?;
    let labels = api.fetch_labels_catalog(&project_id, refresh)?;
    normalize_work_item_body(body, &states, &labels)
}

/// One `--work-items` entry resolved to its UUID, keeping what the caller typed.
pub(super) struct ResolvedWorkItem {
    pub input: String,
    pub id: Uuid,
}

/// Resolve `--work-items` entries (composite `PROJ-123` or UUID) to UUIDs, all before any
/// write. Composite ids must belong to `project_id`. Empty entries and repeats (the same
/// text, or two spellings of the same work item) are skipped.
pub(super) fn resolve_work_items_in_project(
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
    project_arg: &str,
    project_id: Uuid,
    items: &[String],
) -> Result<Vec<ResolvedWorkItem>, CoreError> {
    let mut out: Vec<ResolvedWorkItem> = Vec::new();
    let mut seen_inputs: Vec<String> = Vec::new();
    for raw in items {
        let item = raw.trim();
        if item.is_empty() {
            continue;
        }
        let key = item.to_ascii_lowercase();
        if seen_inputs.contains(&key) {
            continue;
        }
        seen_inputs.push(key);
        // A composite id from another project is refused here with `project_mismatch`.
        let (wpid, wiid) =
            resolve_work_item_pair(api, catalog, refresh, Some(project_arg), item, true)?;
        if wpid != project_id {
            return Err(CoreError::usage_code(
                "project_mismatch",
                format!(
                    "work item {item} belongs to a different project than --project {project_arg}"
                ),
            ));
        }
        if out.iter().any(|r| r.id == wiid) {
            continue;
        }
        out.push(ResolvedWorkItem {
            input: item.to_string(),
            id: wiid,
        });
    }
    if out.is_empty() {
        return Err(CoreError::Usage(
            "--work-items needs at least one id (PROJ-123 or UUID, comma-separated)".into(),
        ));
    }
    Ok(out)
}

/// Remove each resolved work item with `remove`, stopping at the first failure. After a
/// partial success the original error is kept (same `error.code` and exit code) and
/// `error.details` gets `{ "removed": [...], "failed": "<input>" }`.
pub(super) fn remove_each(
    items: &[ResolvedWorkItem],
    mut remove: impl FnMut(&Uuid) -> Result<(), CoreError>,
) -> Result<Vec<Value>, CoreError> {
    let mut removed: Vec<Value> = Vec::new();
    for it in items {
        if let Err(e) = remove(&it.id) {
            if removed.is_empty() {
                return Err(e);
            }
            let done: Vec<&str> = removed
                .iter()
                .filter_map(|v| v.get("input").and_then(|x| x.as_str()))
                .collect();
            let note = format!("failed on {} after removing {}", it.input, done.join(", "));
            return Err(e.with_details(
                &note,
                serde_json::json!({ "removed": removed, "failed": it.input }),
            ));
        }
        removed.push(serde_json::json!({ "input": it.input, "id": it.id.to_string() }));
    }
    Ok(removed)
}

/// Parse a UUID flag value, naming the flag in the error.
pub(super) fn parse_uuid_flag(flag: &str, raw: &str) -> Result<Uuid, CoreError> {
    Uuid::parse_str(raw.trim())
        .map_err(|_| CoreError::Usage(format!("{flag} {raw:?} is not a UUID")))
}

#[cfg(test)]
mod tests {

    #[test]
    fn archived_data_reports_id_and_flag_for_empty_responses() {
        let id = uuid::Uuid::parse_str("211751d8-8fc6-4df8-9b1d-757550e5180b").unwrap();
        assert_eq!(
            super::archived_data(serde_json::Value::Null, &id, true),
            serde_json::json!({"id": "211751d8-8fc6-4df8-9b1d-757550e5180b", "archived": true})
        );
        assert_eq!(
            super::archived_data(serde_json::Value::Null, &id, false)["archived"],
            false
        );
        let body = serde_json::json!({"id": "x", "archived_at": "t"});
        assert_eq!(super::archived_data(body.clone(), &id, true), body);
    }

    use super::*;
    use plane_core::ExitKind;

    fn item(input: &str) -> ResolvedWorkItem {
        ResolvedWorkItem {
            input: input.into(),
            id: Uuid::new_v4(),
        }
    }

    #[test]
    fn partial_failure_keeps_error_kind_and_lists_removed() {
        let items = vec![item("ENG-1"), item("ENG-2")];
        let first = items[0].id;
        let err = remove_each(&items, |id| {
            if *id == first {
                Ok(())
            } else {
                Err(CoreError::Io(std::io::Error::other("connection reset")))
            }
        })
        .unwrap_err();
        assert_eq!(err.exit_kind(), ExitKind::Failure);
        assert_eq!(err.error_code(), "io_error");
        let env = plane_core::ResponseEnvelope::from_core_error(&err);
        let details = env.error.unwrap().details.unwrap();
        assert_eq!(details["failed"], "ENG-2");
        assert_eq!(details["removed"][0]["input"], "ENG-1");
    }

    #[test]
    fn first_failure_is_returned_unchanged() {
        let items = vec![item("ENG-1")];
        let err = remove_each(&items, |_| Err(CoreError::Resolve("nope".into()))).unwrap_err();
        assert!(matches!(err, CoreError::Resolve(_)));
    }

    use crate::dispatch::{Cli, Commands, CommentsCmd, WorkItemsCmd};
    use clap::Parser;

    const ID: &str = "550e8400-e29b-41d4-a716-446655440000";

    #[test]
    fn composite_without_project_is_accepted() {
        let r = classify_work_item(None, "ENG-335").unwrap();
        assert_eq!(
            r,
            WorkItemRef::Composite {
                prefix: "ENG".into(),
                seq: 335,
                project: None
            }
        );
    }

    #[test]
    fn uuid_without_project_has_its_own_code() {
        let err = classify_work_item(None, ID).unwrap_err();
        assert_eq!(err.error_code(), "project_required_for_uuid");
        assert_eq!(err.exit_kind().code(), 2);
        assert!(err.to_string().contains("PREFIX-N"));
    }

    #[test]
    fn uuid_with_project_is_accepted() {
        let r = classify_work_item(Some("ENG"), ID).unwrap();
        assert_eq!(
            r,
            WorkItemRef::Uuid {
                project: "ENG",
                id: Uuid::parse_str(ID).unwrap()
            }
        );
    }

    #[test]
    fn garbage_work_item_is_plain_usage_error() {
        let err = classify_work_item(None, "ENG").unwrap_err();
        assert_eq!(err.error_code(), "validation_error");
    }

    #[test]
    fn mismatched_project_names_both() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let err = ensure_same_project(a, b, "WEB", "ENG-3", "ENG", false).unwrap_err();
        assert_eq!(err.error_code(), "project_mismatch");
        assert_eq!(err.exit_kind().code(), 2);
        let msg = err.to_string();
        assert!(msg.contains("--project WEB") && msg.contains("ENG-3"));
        assert!(msg.contains("drop --project"));
        assert!(ensure_same_project(a, a, "ENG", "ENG-3", "ENG", false).is_ok());
    }

    #[test]
    fn mismatch_where_project_is_required_does_not_say_drop() {
        let err = ensure_same_project(Uuid::new_v4(), Uuid::new_v4(), "WEB", "ENG-3", "ENG", true)
            .unwrap_err();
        let msg = err.to_string();
        assert!(!msg.contains("drop --project"), "{msg}");
        assert!(msg.contains("make --project match the prefix (--project ENG)"));
    }

    fn prevalidate_args(args: &[&str]) -> Result<(), CoreError> {
        let argv: Vec<&str> = std::iter::once("plane-cli")
            .chain(args.iter().copied())
            .collect();
        prevalidate(&Cli::try_parse_from(&argv).expect("parses"))
    }

    #[test]
    fn prevalidate_catches_usage_errors_without_config_or_network() {
        // No PlaneApi or config exists here: these must fail on the arguments alone.
        let err = prevalidate_args(&["work-items", "comments", "list"]).unwrap_err();
        assert_eq!(err.exit_kind().code(), 2);
        let err =
            prevalidate_args(&["work-items", "comments", "list", "--work-item", ID]).unwrap_err();
        assert_eq!(err.error_code(), "project_required_for_uuid");
        let err = prevalidate_args(&["work-items", "get", "--work-item", ID]).unwrap_err();
        assert_eq!(err.error_code(), "project_required_for_uuid");
        let err =
            prevalidate_args(&["work-items", "update", "--work-item", "nonsense"]).unwrap_err();
        assert_eq!(err.error_code(), "validation_error");
    }

    #[test]
    fn prevalidate_checks_work_item_lists() {
        let err = prevalidate_args(&[
            "cycles",
            "items",
            "remove",
            "--project",
            "ENG",
            "--cycle",
            "S1",
            "--work-items",
            "ENG-1,nonsense",
        ])
        .unwrap_err();
        assert_eq!(err.error_code(), "validation_error");
        assert!(prevalidate_args(&[
            "modules",
            "issues",
            "remove",
            "--project",
            "ENG",
            "--module",
            ID,
            "--work-items",
            "ENG-1,ENG-2",
        ])
        .is_ok());
    }

    #[test]
    fn prevalidate_accepts_valid_targets() {
        assert!(
            prevalidate_args(&["work-items", "comments", "list", "--work-item", "ENG-1"]).is_ok()
        );
        assert!(prevalidate_args(&["work-items", "comments", "list", "ENG-1"]).is_ok());
        assert!(prevalidate_args(&[
            "work-items",
            "delete",
            "--project",
            "ENG",
            "--work-item",
            ID
        ])
        .is_ok());
        assert!(prevalidate_args(&["projects", "list"]).is_ok());
    }

    fn comments_list_target(args: &[&str]) -> WorkItemTarget {
        let cli = Cli::try_parse_from(args).expect("parses");
        match cli.command {
            Commands::WorkItems {
                cmd:
                    WorkItemsCmd::Comments {
                        cmd: CommentsCmd::List { target, .. },
                    },
            } => target,
            _ => panic!("expected comments list"),
        }
    }

    #[test]
    fn comments_list_parses_without_project() {
        let t = comments_list_target(&[
            "plane-cli",
            "work-items",
            "comments",
            "list",
            "--work-item",
            "ENG-335",
        ]);
        assert_eq!(t.project, None);
        assert_eq!(target_work_item(&t).unwrap(), "ENG-335");
    }

    #[test]
    fn comments_list_accepts_positional_work_item() {
        let t = comments_list_target(&["plane-cli", "work-items", "comments", "list", "ENG-335"]);
        assert_eq!(t.work_item, None);
        assert_eq!(target_work_item(&t).unwrap(), "ENG-335");
    }

    #[test]
    fn conflicting_flag_and_positional_is_refused() {
        let t = comments_list_target(&[
            "plane-cli",
            "work-items",
            "comments",
            "list",
            "--work-item",
            "ENG-1",
            "ENG-2",
        ]);
        assert!(target_work_item(&t).is_err());
    }

    #[test]
    fn missing_work_item_is_refused() {
        let t = comments_list_target(&["plane-cli", "work-items", "comments", "list"]);
        assert!(target_work_item(&t).is_err());
    }

    #[test]
    fn every_target_subcommand_parses_without_project() {
        for args in [
            vec!["work-items", "update", "--work-item", "ENG-1"],
            vec!["work-items", "delete", "--work-item", "ENG-1"],
            vec!["work-items", "activities", "list", "--work-item", "ENG-1"],
            vec![
                "work-items",
                "activities",
                "get",
                "--work-item",
                "ENG-1",
                "--id",
                ID,
            ],
            vec![
                "work-items",
                "comments",
                "get",
                "--work-item",
                "ENG-1",
                "--id",
                ID,
            ],
            vec![
                "work-items",
                "comments",
                "create",
                "--work-item",
                "ENG-1",
                "--body",
                "-",
            ],
            vec![
                "work-items",
                "comments",
                "update",
                "--work-item",
                "ENG-1",
                "--id",
                ID,
            ],
            vec![
                "work-items",
                "comments",
                "delete",
                "--work-item",
                "ENG-1",
                "--id",
                ID,
            ],
            vec!["work-items", "links", "list", "--work-item", "ENG-1"],
            vec![
                "work-items",
                "links",
                "get",
                "--work-item",
                "ENG-1",
                "--id",
                ID,
            ],
            vec![
                "work-items",
                "links",
                "create",
                "--work-item",
                "ENG-1",
                "--url",
                "https://x",
            ],
            vec![
                "work-items",
                "links",
                "update",
                "--work-item",
                "ENG-1",
                "--id",
                ID,
            ],
            vec![
                "work-items",
                "links",
                "delete",
                "--work-item",
                "ENG-1",
                "--id",
                ID,
            ],
            vec!["work-items", "relations", "list", "--work-item", "ENG-1"],
            vec!["work-items", "relations", "create", "--work-item", "ENG-1"],
        ] {
            let argv: Vec<&str> = std::iter::once("plane-cli").chain(args.clone()).collect();
            assert!(Cli::try_parse_from(&argv).is_ok(), "{args:?} should parse");
        }
    }
}
