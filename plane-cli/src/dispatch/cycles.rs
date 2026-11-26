//! `plane-cli cycles …`: cycle CRUD, archive, work-item membership and transfer.
//!
//! Checked against Plane Community Edition 1.4.2 (see CRUD-REFERENCE.md).

use super::helpers::{archived_data, remove_each, resolve_work_items_in_project};
use super::{nested_cmd_name, PageArgs};
use crate::body::merge_body;
use crate::output;
use clap::{Subcommand, ValueHint};
use plane_core::{
    annotate_cycle_statuses, cycle_date_flag, now_utc_timestamp, resolve_named, resolve_project_id,
    CoreError, NameScope, PageParams, PlaneApi, ProjectEntry, CYCLE_VIEWS,
};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Subcommand)]
pub enum CyclesCmd {
    /// List cycles (adds a computed `status`: draft, upcoming, current, completed)
    List {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            value_name = "STATUS",
            value_parser = clap::builder::PossibleValuesParser::new(CYCLE_VIEWS),
            conflicts_with = "archived",
            help = "Filter by status (Plane `cycle_view`)"
        )]
        status: Option<String>,
        #[arg(long, help = "List archived cycles instead")]
        archived: bool,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Show one cycle (adds a computed `status`)
    Get {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            visible_alias = "id",
            value_name = "CYCLE",
            value_hint = ValueHint::Other,
            help = "Cycle UUID or exact name"
        )]
        cycle: String,
    },
    /// Create a cycle. `--start` and `--end` are dates (YYYY-MM-DD), both or neither
    Create {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(long, value_name = "NAME")]
        name: Option<String>,
        #[arg(long, value_name = "TEXT")]
        description: Option<String>,
        #[command(flatten)]
        dates: CycleDateArgs,
        #[arg(long, value_name = "USER_UUID", help = "Owner (`owned_by`) user UUID")]
        owner: Option<String>,
        #[arg(long = "body", value_name = "PATH|-", value_hint = ValueHint::AnyPath)]
        json: Option<String>,
    },
    /// Update (rename, re-date, re-own) a cycle
    Update {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            visible_alias = "id",
            value_name = "CYCLE",
            value_hint = ValueHint::Other,
            help = "Cycle UUID or exact name"
        )]
        cycle: String,
        #[arg(long, value_name = "NAME")]
        name: Option<String>,
        #[arg(long, value_name = "TEXT")]
        description: Option<String>,
        #[command(flatten)]
        dates: CycleDateArgs,
        #[arg(long, value_name = "USER_UUID", help = "Owner (`owned_by`) user UUID")]
        owner: Option<String>,
        #[arg(long = "body", value_name = "PATH|-", value_hint = ValueHint::AnyPath)]
        json: Option<String>,
    },
    /// Delete a cycle (no confirmation, like `modules delete`)
    Delete {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            visible_alias = "id",
            value_name = "CYCLE",
            value_hint = ValueHint::Other,
            help = "Cycle UUID or exact name"
        )]
        cycle: String,
    },
    /// Archive a completed cycle
    Archive {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            visible_alias = "id",
            value_name = "CYCLE",
            value_hint = ValueHint::Other,
            help = "Cycle UUID or exact name"
        )]
        cycle: String,
    },
    /// Restore an archived cycle
    Unarchive {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            visible_alias = "id",
            value_name = "CYCLE",
            value_hint = ValueHint::Other,
            help = "Cycle UUID or exact name"
        )]
        cycle: String,
    },
    /// Move the unfinished work items of a completed cycle to another cycle
    Transfer {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            value_name = "CYCLE",
            value_hint = ValueHint::Other,
            help = "Source cycle UUID or exact name (must be completed)"
        )]
        cycle: String,
        #[arg(
            long,
            value_name = "CYCLE",
            value_hint = ValueHint::Other,
            help = "Target cycle UUID or exact name"
        )]
        to: String,
    },
    /// Work items in a cycle
    Items {
        #[command(subcommand)]
        cmd: CycleItemsCmd,
    },
}

#[derive(clap::Args, Clone, Default)]
pub struct CycleDateArgs {
    #[arg(
        long,
        value_name = "YYYY-MM-DD",
        help = "Start date; Plane stores 00:00 in the project's time zone"
    )]
    pub start: Option<String>,
    #[arg(
        long,
        value_name = "YYYY-MM-DD",
        help = "End date; Plane stores 23:59 in the project's time zone"
    )]
    pub end: Option<String>,
}

#[derive(Subcommand)]
pub enum CycleItemsCmd {
    /// Work items in the cycle
    List {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            value_name = "CYCLE",
            value_hint = ValueHint::Other,
            help = "Cycle UUID or exact name"
        )]
        cycle: String,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Add work items; an item already in another cycle is moved here
    Add {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            value_name = "CYCLE",
            value_hint = ValueHint::Other,
            help = "Cycle UUID or exact name"
        )]
        cycle: String,
        #[arg(
            long,
            alias = "work-item",
            value_delimiter = ',',
            required = true,
            value_name = "ITEM",
            value_hint = ValueHint::Other,
            help = "Composite ids (PROJ-123) or work item UUIDs, comma-separated"
        )]
        work_items: Vec<String>,
    },
    /// Remove work items from the cycle (one call per item; stops at the first failure)
    Remove {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(
            long,
            value_name = "CYCLE",
            value_hint = ValueHint::Other,
            help = "Cycle UUID or exact name"
        )]
        cycle: String,
        #[arg(
            long,
            alias = "work-item",
            value_delimiter = ',',
            required = true,
            value_name = "ITEM",
            value_hint = ValueHint::Other,
            help = "Composite ids (PROJ-123) or work item UUIDs, comma-separated"
        )]
        work_items: Vec<String>,
    },
}

/// Telemetry name, e.g. `items remove`.
pub fn cycles_cmd_name(cmd: &CyclesCmd) -> String {
    match cmd {
        CyclesCmd::List { .. } => "list".into(),
        CyclesCmd::Get { .. } => "get".into(),
        CyclesCmd::Create { .. } => "create".into(),
        CyclesCmd::Update { .. } => "update".into(),
        CyclesCmd::Delete { .. } => "delete".into(),
        CyclesCmd::Archive { .. } => "archive".into(),
        CyclesCmd::Unarchive { .. } => "unarchive".into(),
        CyclesCmd::Transfer { .. } => "transfer".into(),
        CyclesCmd::Items { cmd } => nested_cmd_name("items", cmd, cycle_items_cmd_name),
    }
}

fn cycle_items_cmd_name(cmd: &CycleItemsCmd) -> &'static str {
    match cmd {
        CycleItemsCmd::List { .. } => "list",
        CycleItemsCmd::Add { .. } => "add",
        CycleItemsCmd::Remove { .. } => "remove",
    }
}

/// Resolve `--cycle` (UUID or exact name) in the lists `scope` names; see
/// [`plane_core::resolve_named`] for the matching rules.
fn resolve_cycle(
    api: &PlaneApi,
    project_id: &Uuid,
    arg: &str,
    scope: NameScope,
) -> Result<Uuid, CoreError> {
    resolve_named(
        arg,
        "cycle",
        scope,
        "cycles list --project <PROJECT> [--archived]",
        || Ok(api.list_cycles(project_id, None, &PageParams::all())?.0),
        || Ok(api.list_archived_cycles(project_id, &PageParams::all())?.0),
    )
}

/// Body for create/update from `--body` plus flags (flags win).
fn cycle_body(
    json_path: Option<&str>,
    name: &Option<String>,
    description: &Option<String>,
    dates: &CycleDateArgs,
    owner: &Option<String>,
) -> Result<Value, CoreError> {
    let mut p = serde_json::Map::new();
    if let Some(n) = name {
        p.insert("name".into(), json!(n));
    }
    if let Some(d) = description {
        p.insert("description".into(), json!(d));
    }
    match (&dates.start, &dates.end) {
        (Some(s), Some(e)) => {
            let s = cycle_date_flag("--start", s)?;
            let e = cycle_date_flag("--end", e)?;
            if e < s {
                return Err(CoreError::Usage(
                    "--end is before --start; pass dates in order".into(),
                ));
            }
            p.insert("start_date".into(), json!(s));
            p.insert("end_date".into(), json!(e));
        }
        (None, None) => {}
        _ => {
            return Err(CoreError::Usage(
                "pass both --start and --end (or neither): Plane only applies the project's time zone when it gets both dates"
                    .into(),
            ))
        }
    }
    if let Some(o) = owner {
        Uuid::parse_str(o.trim())
            .map_err(|_| CoreError::Usage(format!("--owner {o:?} is not a user UUID")))?;
        p.insert("owned_by".into(), json!(o.trim()));
    }
    merge_body(json_path, Value::Object(p))
}

pub(super) fn run(
    cmd: &CyclesCmd,
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
) -> Result<(), CoreError> {
    match cmd {
        CyclesCmd::List {
            project,
            status,
            archived,
            page,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let (mut v, nc, tr) = if *archived {
                api.list_archived_cycles(&pid, &page.params())?
            } else {
                api.list_cycles(&pid, status.as_deref(), &page.params())?
            };
            annotate_cycle_statuses(&mut v, &now_utc_timestamp());
            output::emit_success(json_mode, v, nc, tr)?;
        }
        CyclesCmd::Get { project, cycle } => {
            let pid = resolve_project_id(project, catalog)?;
            let cid = resolve_cycle(api, &pid, cycle, NameScope::Either)?;
            let mut v = api.get_cycle(&pid, &cid)?;
            let status = plane_core::cycle_status(&v, &now_utc_timestamp());
            if let Some(obj) = v.as_object_mut() {
                obj.entry("status").or_insert_with(|| json!(status));
            }
            output::emit_success(json_mode, v, None, false)?;
        }
        CyclesCmd::Create {
            project,
            name,
            description,
            dates,
            owner,
            json,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let body = cycle_body(json.as_deref(), name, description, dates, owner)?;
            if body.get("name").and_then(|x| x.as_str()).is_none() {
                return Err(CoreError::Usage(
                    "cycles create needs --name (or `name` in --body)".into(),
                ));
            }
            let v = api.create_cycle(&pid, &body)?;
            output::emit_success(json_mode, v, None, false)?;
        }
        CyclesCmd::Update {
            project,
            cycle,
            name,
            description,
            dates,
            owner,
            json,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let cid = resolve_cycle(api, &pid, cycle, NameScope::Either)?;
            let body = cycle_body(json.as_deref(), name, description, dates, owner)?;
            if body.as_object().map(|m| m.is_empty()).unwrap_or(true) {
                return Err(CoreError::Usage(
                    "nothing to update; pass --name, --description, --start/--end, --owner or --body"
                        .into(),
                ));
            }
            let v = api.update_cycle(&pid, &cid, &body)?;
            output::emit_success(json_mode, v, None, false)?;
        }
        CyclesCmd::Delete { project, cycle } => {
            let pid = resolve_project_id(project, catalog)?;
            let cid = resolve_cycle(api, &pid, cycle, NameScope::Either)?;
            super::policy_gate::gate_delete(api, catalog, &pid, "cycle", "cycles delete")?;
            api.delete_cycle(&pid, &cid)?;
            output::emit_empty_success(json_mode)?;
        }
        CyclesCmd::Archive { project, cycle } => {
            let pid = resolve_project_id(project, catalog)?;
            let cid = resolve_cycle(api, &pid, cycle, NameScope::Active)?;
            super::policy_gate::gate_archive(api, catalog, &pid, "cycle", "cycles archive")?;
            let v = api.archive_cycle(&pid, &cid)?;
            let data = archived_data(v, &cid, true);
            output::emit_success(json_mode, data, None, false)?;
        }
        CyclesCmd::Unarchive { project, cycle } => {
            let pid = resolve_project_id(project, catalog)?;
            let cid = resolve_cycle(api, &pid, cycle, NameScope::Archived)?;
            api.unarchive_cycle(&pid, &cid)?;
            output::emit_success(
                json_mode,
                archived_data(Value::Null, &cid, false),
                None,
                false,
            )?;
        }
        CyclesCmd::Transfer { project, cycle, to } => {
            let pid = resolve_project_id(project, catalog)?;
            let from = resolve_cycle(api, &pid, cycle, NameScope::Either)?;
            let target = resolve_cycle(api, &pid, to, NameScope::Either)?;
            if from == target {
                return Err(CoreError::Usage(
                    "--cycle and --to are the same cycle".into(),
                ));
            }
            let v = api.transfer_cycle_work_items(&pid, &from, &target)?;
            let mut data = json!({ "from": from.to_string(), "to": target.to_string() });
            if let (Some(obj), Some(resp)) = (data.as_object_mut(), v.as_object()) {
                for (k, val) in resp {
                    obj.insert(k.clone(), val.clone());
                }
            }
            output::emit_success(json_mode, data, None, false)?;
        }
        CyclesCmd::Items { cmd } => match cmd {
            CycleItemsCmd::List {
                project,
                cycle,
                page,
            } => {
                let pid = resolve_project_id(project, catalog)?;
                let cid = resolve_cycle(api, &pid, cycle, NameScope::Either)?;
                let (v, nc, tr) = api.list_cycle_work_items(&pid, &cid, &page.params())?;
                output::emit_success(json_mode, v, nc, tr)?;
            }
            CycleItemsCmd::Add {
                project,
                cycle,
                work_items,
            } => {
                let pid = resolve_project_id(project, catalog)?;
                let cid = resolve_cycle(api, &pid, cycle, NameScope::Either)?;
                let items =
                    resolve_work_items_in_project(api, catalog, refresh, project, pid, work_items)?;
                let ids: Vec<Uuid> = items.iter().map(|i| i.id).collect();
                let v = api.add_cycle_work_items(&pid, &cid, &ids)?;
                output::emit_success(json_mode, v, None, false)?;
            }
            CycleItemsCmd::Remove {
                project,
                cycle,
                work_items,
            } => {
                let pid = resolve_project_id(project, catalog)?;
                let cid = resolve_cycle(api, &pid, cycle, NameScope::Either)?;
                let items =
                    resolve_work_items_in_project(api, catalog, refresh, project, pid, work_items)?;
                let removed =
                    remove_each(&items, |iid| api.remove_cycle_work_item(&pid, &cid, iid))?;
                output::emit_success(
                    json_mode,
                    json!({ "cycle": cid.to_string(), "removed": removed }),
                    None,
                    false,
                )?;
            }
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::policy_gate::tests::{mock_me, PolicyEnv, DENY_DELETES};
    use mockito::Matcher;

    const PID: &str = "211751d8-8fc6-4df8-9b1d-757550e5180b";
    const ACTIVE: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const OLD: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";

    fn catalog() -> Vec<ProjectEntry> {
        vec![ProjectEntry {
            id: Uuid::parse_str(PID).unwrap(),
            name: "Lab".into(),
            identifier: "WEB".into(),
        }]
    }

    fn lists(server: &mut mockito::ServerGuard) {
        server
            .mock(
                "GET",
                Matcher::Regex(format!("^/api/v1/workspaces/ws/projects/{PID}/cycles/")),
            )
            .with_body(format!(
                r#"{{"results":[{{"id":"{ACTIVE}","name":"Sprint 1"}}],"next_page_results":false}}"#
            ))
            .create();
        server
            .mock(
                "GET",
                Matcher::Regex(format!(
                    "^/api/v1/workspaces/ws/projects/{PID}/archived-cycles/"
                )),
            )
            .with_body(format!(
                r#"{{"results":[{{"id":"{OLD}","name":"Sprint 1"}}],"next_page_results":false}}"#
            ))
            .create();
    }

    #[test]
    fn same_name_cycles_resolve_by_scope_and_are_ambiguous_otherwise() {
        let _g = crate::plane_env_files::lock_env();
        let mut server = mockito::Server::new();
        lists(&mut server);
        let m = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
        let pid = Uuid::parse_str(PID).unwrap();
        let r = |scope| resolve_cycle(&m.api, &pid, "Sprint 1", scope);
        assert_eq!(r(NameScope::Active).unwrap().to_string(), ACTIVE);
        assert_eq!(r(NameScope::Archived).unwrap().to_string(), OLD);
        let e = r(NameScope::Either).unwrap_err();
        assert!(
            e.to_string().contains(ACTIVE) && e.to_string().contains(OLD),
            "{e}"
        );
        assert_eq!(e.exit_kind().code(), 2);
    }

    #[test]
    fn deletes_policy_refuses_cycle_archive_and_delete_but_not_unarchive() {
        let _g = crate::plane_env_files::lock_env();
        let _p = PolicyEnv::new("WEB", Some(DENY_DELETES));
        let mut server = mockito::Server::new();
        mock_me(&mut server);
        let post = server.mock("POST", Matcher::Any).expect(0).create();
        let delete = server
            .mock(
                "DELETE",
                format!("/api/v1/workspaces/ws/projects/{PID}/cycles/{ACTIVE}/").as_str(),
            )
            .expect(0)
            .create();
        let unarchive = server
            .mock(
                "DELETE",
                format!("/api/v1/workspaces/ws/projects/{PID}/archived-cycles/{OLD}/unarchive/")
                    .as_str(),
            )
            .with_status(204)
            .expect(1)
            .create();
        let mut m = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
        let cat = catalog();
        for cmd in [
            CyclesCmd::Archive {
                project: "WEB".into(),
                cycle: ACTIVE.into(),
            },
            CyclesCmd::Delete {
                project: "WEB".into(),
                cycle: ACTIVE.into(),
            },
        ] {
            let e = run(&cmd, true, &mut m.api, &cat, false).unwrap_err();
            assert_eq!(e.error_code(), "policy_violation");
        }
        run(
            &CyclesCmd::Unarchive {
                project: "WEB".into(),
                cycle: OLD.into(),
            },
            true,
            &mut m.api,
            &cat,
            false,
        )
        .unwrap();
        post.assert();
        delete.assert();
        unarchive.assert();
    }
}
