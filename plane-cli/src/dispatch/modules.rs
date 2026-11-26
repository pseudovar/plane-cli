//! `plane-cli modules …`: module CRUD, archive, and the work items in a module.
//!
//! Archive, unarchive and `list --archived` checked against Plane Community Edition 1.4.2
//! (see CRUD-REFERENCE.md).

use super::{nested_cmd_name, PageArgs};
use crate::output;
use clap::{Subcommand, ValueHint};
use plane_core::{
    load_json_source, resolve_named, resolve_project_id, CoreError, NameScope, PageParams,
    PlaneApi, ProjectEntry,
};
use serde_json::json;
use uuid::Uuid;

#[derive(Subcommand)]
pub enum ModulesCmd {
    List {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(long, help = "List archived modules instead")]
        archived: bool,
        #[command(flatten)]
        page: PageArgs,
    },
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
            value_name = "MODULE_UUID",
            value_hint = ValueHint::Other,
            help = "Module UUID"
        )]
        id: String,
    },
    Create {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
    },
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
            value_name = "MODULE_UUID",
            value_hint = ValueHint::Other,
            help = "Module UUID"
        )]
        id: String,
        #[arg(long = "body", value_name = "JSON", value_hint = ValueHint::Other)]
        json: Option<String>,
    },
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
            value_name = "MODULE_UUID",
            value_hint = ValueHint::Other,
            help = "Module UUID"
        )]
        id: String,
    },
    /// Archive a module (Plane only archives completed or cancelled modules)
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
            value_name = "MODULE",
            value_hint = ValueHint::Other,
            help = "Module UUID or exact name"
        )]
        module: String,
    },
    /// Restore an archived module
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
            value_name = "MODULE",
            value_hint = ValueHint::Other,
            help = "Module UUID or exact name"
        )]
        module: String,
    },
    Issues {
        #[command(subcommand)]
        cmd: ModuleIssuesCmd,
    },
}

#[derive(Subcommand)]
pub enum ModuleIssuesCmd {
    /// Work items in the module
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
            value_name = "MODULE",
            value_hint = ValueHint::Other,
            help = "Module UUID"
        )]
        module: String,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Add work items to the module
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
            value_name = "MODULE",
            value_hint = ValueHint::Other,
            help = "Module UUID"
        )]
        module: String,
        #[arg(
            long,
            value_delimiter = ',',
            required = true,
            alias = "work-item",
            value_name = "ITEM",
            value_hint = ValueHint::Other,
            help = "Composite ids (PROJ-123) or work item UUIDs, comma-separated"
        )]
        work_items: Vec<String>,
    },
    /// Remove work items from the module (one call per item; stops at the first failure)
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
            value_name = "MODULE",
            value_hint = ValueHint::Other,
            help = "Module UUID"
        )]
        module: String,
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

/// Telemetry name, e.g. `issues add`.
pub fn modules_cmd_name(cmd: &ModulesCmd) -> String {
    match cmd {
        ModulesCmd::List { .. } => "list".into(),
        ModulesCmd::Get { .. } => "get".into(),
        ModulesCmd::Create { .. } => "create".into(),
        ModulesCmd::Update { .. } => "update".into(),
        ModulesCmd::Delete { .. } => "delete".into(),
        ModulesCmd::Archive { .. } => "archive".into(),
        ModulesCmd::Unarchive { .. } => "unarchive".into(),
        ModulesCmd::Issues { cmd } => nested_cmd_name("issues", cmd, module_issues_cmd_name),
    }
}

fn module_issues_cmd_name(cmd: &ModuleIssuesCmd) -> &'static str {
    match cmd {
        ModuleIssuesCmd::List { .. } => "list",
        ModuleIssuesCmd::Add { .. } => "add",
        ModuleIssuesCmd::Remove { .. } => "remove",
    }
}

/// Resolve `--module` (UUID or exact name) in the lists `scope` names; see
/// [`plane_core::resolve_named`] for the matching rules.
pub(crate) fn resolve_module(
    api: &PlaneApi,
    project_id: &Uuid,
    arg: &str,
    scope: NameScope,
) -> Result<Uuid, CoreError> {
    resolve_named(
        arg,
        "module",
        scope,
        "modules list --project <PROJECT> [--archived]",
        || Ok(api.list_modules(project_id, &PageParams::all())?.0),
        || Ok(api.list_archived_modules(project_id, &PageParams::all())?.0),
    )
}

pub(super) fn run(
    cmd: &ModulesCmd,
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
) -> Result<(), CoreError> {
    match cmd {
        ModulesCmd::List {
            project,
            archived,
            page,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let (v, nc, tr) = if *archived {
                api.list_archived_modules(&pid, &page.params())?
            } else {
                api.list_modules(&pid, &page.params())?
            };
            output::emit_success(json_mode, v, nc, tr)?;
        }
        ModulesCmd::Archive { project, module } => {
            let pid = resolve_project_id(project, catalog)?;
            let mid = resolve_module(api, &pid, module, NameScope::Active)?;
            super::policy_gate::gate_archive(api, catalog, &pid, "module", "modules archive")?;
            let v = api.archive_module(&pid, &mid)?;
            let data = super::helpers::archived_data(v, &mid, true);
            output::emit_success(json_mode, data, None, false)?;
        }
        ModulesCmd::Unarchive { project, module } => {
            let pid = resolve_project_id(project, catalog)?;
            let mid = resolve_module(api, &pid, module, NameScope::Archived)?;
            api.unarchive_module(&pid, &mid)?;
            output::emit_success(
                json_mode,
                super::helpers::archived_data(serde_json::Value::Null, &mid, false),
                None,
                false,
            )?;
        }
        ModulesCmd::Get { project, id } => {
            let pid = resolve_project_id(project, catalog)?;
            let mid = Uuid::parse_str(id)
                .map_err(|_| CoreError::Usage(format!("invalid module UUID: {id:?}")))?;
            let v = api.get_module(&pid, &mid)?;
            output::emit_success(json_mode, v, None, false)?;
        }
        ModulesCmd::Create { project, json } => {
            let pid = resolve_project_id(project, catalog)?;
            let body = load_json_source(json.as_deref())?.unwrap_or(json!({}));
            let v = api.create_module(&pid, &body)?;
            output::emit_success(json_mode, v, None, false)?;
        }
        ModulesCmd::Update { project, id, json } => {
            let pid = resolve_project_id(project, catalog)?;
            let mid = Uuid::parse_str(id)
                .map_err(|_| CoreError::Usage(format!("invalid module UUID: {id:?}")))?;
            let body = load_json_source(json.as_deref())?.unwrap_or(json!({}));
            let v = api.update_module(&pid, &mid, &body)?;
            output::emit_success(json_mode, v, None, false)?;
        }
        ModulesCmd::Delete { project, id } => {
            let pid = resolve_project_id(project, catalog)?;
            let mid = Uuid::parse_str(id)
                .map_err(|_| CoreError::Usage(format!("invalid module UUID: {id:?}")))?;
            super::policy_gate::gate_delete(api, catalog, &pid, "module", "modules delete")?;
            api.delete_module(&pid, &mid)?;
            output::emit_empty_success(json_mode)?;
        }
        ModulesCmd::Issues { cmd: sub } => match sub {
            ModuleIssuesCmd::List {
                project,
                module,
                page,
            } => {
                let pid = resolve_project_id(project, catalog)?;
                let mid = Uuid::parse_str(module)
                    .map_err(|_| CoreError::Usage(format!("invalid module UUID: {module:?}")))?;
                let (v, nc, tr) = api.list_module_work_items(&pid, &mid, &page.params())?;
                output::emit_success(json_mode, v, nc, tr)?;
            }
            ModuleIssuesCmd::Add {
                project,
                module,
                work_items,
            } => {
                let pid = resolve_project_id(project, catalog)?;
                let mid = Uuid::parse_str(module)
                    .map_err(|_| CoreError::Usage(format!("invalid module UUID: {module:?}")))?;
                let items = super::helpers::resolve_work_items_in_project(
                    api, catalog, refresh, project, pid, work_items,
                )?;
                let uuids: Vec<Uuid> = items.iter().map(|i| i.id).collect();
                let v = api.add_module_work_items(&pid, &mid, &uuids)?;
                output::emit_success(json_mode, v, None, false)?;
            }
            ModuleIssuesCmd::Remove {
                project,
                module,
                work_items,
            } => {
                let pid = resolve_project_id(project, catalog)?;
                let mid = Uuid::parse_str(module)
                    .map_err(|_| CoreError::Usage(format!("invalid module UUID: {module:?}")))?;
                let items = super::helpers::resolve_work_items_in_project(
                    api, catalog, refresh, project, pid, work_items,
                )?;
                let removed = super::helpers::remove_each(&items, |iid| {
                    api.remove_module_work_item(&pid, &mid, iid)
                })?;
                output::emit_success(
                    json_mode,
                    json!({ "module": mid.to_string(), "removed": removed }),
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
    const MID: &str = "77777777-7777-4777-8777-777777777777";
    const LIVE: &str = "88888888-8888-4888-8888-888888888888";

    fn catalog() -> Vec<ProjectEntry> {
        vec![ProjectEntry {
            id: Uuid::parse_str(PID).unwrap(),
            name: "Lab".into(),
            identifier: "WEB".into(),
        }]
    }

    fn list_mock(server: &mut mockito::ServerGuard, sub: &str, body: &str) -> mockito::Mock {
        server
            .mock(
                "GET",
                Matcher::Regex(format!("^/api/v1/workspaces/ws/projects/{PID}/{sub}/")),
            )
            .with_body(body)
            .create()
    }

    #[test]
    fn same_name_active_and_archived_resolves_by_scope() {
        let _g = crate::plane_env_files::lock_env();
        let mut server = mockito::Server::new();
        list_mock(
            &mut server,
            "modules",
            &format!(r#"{{"results":[{{"id":"{LIVE}","name":"Q4"}}],"next_page_results":false}}"#),
        );
        list_mock(
            &mut server,
            "archived-modules",
            &format!(
                r#"{{"results":[{{"id":"{MID}","name":"Q4"}},{{"id":"99999999-9999-4999-8999-999999999999","name":"old"}}],"next_page_results":false}}"#
            ),
        );
        let m = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
        let pid = Uuid::parse_str(PID).unwrap();
        let r = |name: &str, scope| resolve_module(&m.api, &pid, name, scope);
        assert_eq!(r("Q4", NameScope::Active).unwrap().to_string(), LIVE);
        assert_eq!(r("Q4", NameScope::Archived).unwrap().to_string(), MID);
        let e = r("Q4", NameScope::Either).unwrap_err();
        assert!(
            e.to_string().contains(LIVE) && e.to_string().contains(MID),
            "{e}"
        );
        // Case-only match, archived list only.
        assert_eq!(
            r("OLD", NameScope::Archived).unwrap().to_string(),
            "99999999-9999-4999-8999-999999999999"
        );
        // archive searches active modules only, and says so.
        let e = r("old", NameScope::Active).unwrap_err();
        assert!(e.to_string().contains("active modules: Q4"), "{e}");
        assert!(!e.to_string().contains("archived modules"), "{e}");
        let e = r("nope", NameScope::Either).unwrap_err();
        assert!(e.to_string().contains("archived modules: Q4, old"), "{e}");
    }

    #[test]
    fn archive_unarchive_and_archived_list_hit_the_ce_paths() {
        let _g = crate::plane_env_files::lock_env();
        let _p = PolicyEnv::new("WEB", None);
        let mut server = mockito::Server::new();
        let archive = server
            .mock(
                "POST",
                format!("/api/v1/workspaces/ws/projects/{PID}/modules/{MID}/archive/").as_str(),
            )
            .with_status(204)
            .expect(1)
            .create();
        let unarchive = server
            .mock(
                "DELETE",
                format!("/api/v1/workspaces/ws/projects/{PID}/archived-modules/{MID}/unarchive/")
                    .as_str(),
            )
            .with_status(204)
            .expect(1)
            .create();
        let archived = list_mock(
            &mut server,
            "archived-modules",
            r#"{"results":[],"next_page_results":false}"#,
        );
        let mut m = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
        let cat = catalog();
        for cmd in [
            ModulesCmd::Archive {
                project: "WEB".into(),
                module: MID.into(),
            },
            ModulesCmd::Unarchive {
                project: "WEB".into(),
                module: MID.into(),
            },
            ModulesCmd::List {
                project: "WEB".into(),
                archived: true,
                page: PageArgs::default(),
            },
        ] {
            run(&cmd, true, &mut m.api, &cat, false).unwrap();
        }
        archive.assert();
        unarchive.assert();
        archived.assert();
    }

    #[test]
    fn deletes_policy_refuses_module_archive_and_delete_but_not_unarchive() {
        let _g = crate::plane_env_files::lock_env();
        let _p = PolicyEnv::new("WEB", Some(DENY_DELETES));
        let mut server = mockito::Server::new();
        mock_me(&mut server);
        let post = server.mock("POST", Matcher::Any).expect(0).create();
        let unarchive = server
            .mock(
                "DELETE",
                format!("/api/v1/workspaces/ws/projects/{PID}/archived-modules/{MID}/unarchive/")
                    .as_str(),
            )
            .with_status(204)
            .expect(1)
            .create();
        let delete = server
            .mock(
                "DELETE",
                format!("/api/v1/workspaces/ws/projects/{PID}/modules/{MID}/").as_str(),
            )
            .expect(0)
            .create();
        let mut m = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
        let cat = catalog();
        for cmd in [
            ModulesCmd::Archive {
                project: "WEB".into(),
                module: MID.into(),
            },
            ModulesCmd::Delete {
                project: "WEB".into(),
                id: MID.into(),
            },
        ] {
            let e = run(&cmd, true, &mut m.api, &cat, false).unwrap_err();
            assert_eq!(e.error_code(), "policy_violation");
        }
        run(
            &ModulesCmd::Unarchive {
                project: "WEB".into(),
                module: MID.into(),
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
