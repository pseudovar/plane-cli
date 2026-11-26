//! `plane-cli projects archive|unarchive|summary|delete`.
//!
//! Checked against Plane Community Edition 1.4.2 (see CRUD-REFERENCE.md).

use super::helpers::archived_data;
use super::policy_gate;
use crate::output;
use plane_core::{resolve_project_id, CoreError, PlaneApi, ProjectEntry};
use serde_json::Value;

pub(super) fn archive(
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    id: &str,
) -> Result<(), CoreError> {
    let pid = resolve_project_id(id, catalog)?;
    policy_gate::gate_archive(api, catalog, &pid, "project", "projects archive")?;
    let v = api.archive_project(&pid)?;
    output::emit_success(json_mode, archived_data(v, &pid, true), None, false)?;
    Ok(())
}

pub(super) fn unarchive(
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    id: &str,
) -> Result<(), CoreError> {
    let pid = resolve_project_id(id, catalog)?;
    api.unarchive_project(&pid)?;
    output::emit_success(
        json_mode,
        archived_data(Value::Null, &pid, false),
        None,
        false,
    )?;
    Ok(())
}

/// `projects delete`, behind the project's `[deletes]` policy rule.
pub(super) fn delete(
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    id: &str,
) -> Result<(), CoreError> {
    let pid = resolve_project_id(id, catalog)?;
    policy_gate::gate_delete(api, catalog, &pid, "project", "projects delete")?;
    api.delete_project(&pid)?;
    output::emit_empty_success(json_mode)?;
    Ok(())
}

pub(super) fn summary(
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    id: &str,
) -> Result<(), CoreError> {
    let pid = resolve_project_id(id, catalog)?;
    let v = api.project_summary(&pid)?;
    output::emit_success(json_mode, v, None, false)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::policy_gate::tests::{mock_me, PolicyEnv, DENY_DELETES};
    use mockito::Matcher;
    use uuid::Uuid;

    const PID: &str = "a16f3e34-1f6d-4109-9dd0-a87f651d53b4";

    fn catalog() -> Vec<ProjectEntry> {
        vec![ProjectEntry {
            id: Uuid::parse_str(PID).unwrap(),
            name: "Sandbox".into(),
            identifier: "WEB".into(),
        }]
    }

    #[test]
    fn archive_unarchive_and_summary_hit_the_ce_paths() {
        let _g = crate::plane_env_files::lock_env();
        let _p = PolicyEnv::new("WEB", None);
        let mut server = mockito::Server::new();
        let base = format!("/api/v1/workspaces/ws/projects/{PID}");
        let archive_mock = server
            .mock("POST", format!("{base}/archive/").as_str())
            .with_status(204)
            .expect(1)
            .create();
        let unarchive_mock = server
            .mock("DELETE", format!("{base}/archive/").as_str())
            .with_status(204)
            .expect(1)
            .create();
        let summary_mock = server
            .mock("GET", format!("{base}/summary/").as_str())
            .with_body(r#"{"counts":{"issues":1},"id":"x"}"#)
            .expect(1)
            .create();
        let mut m = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
        let cat = catalog();
        // A cached state catalog must not survive archive/unarchive.
        m.api
            .cache()
            .put(&format!("states_{PID}"), serde_json::json!({"results": []}))
            .unwrap();
        archive(true, &mut m.api, &cat, "WEB").unwrap();
        assert!(m
            .api
            .cache()
            .get(&format!("states_{PID}"), false)
            .unwrap()
            .is_none());
        unarchive(true, &mut m.api, &cat, "web").unwrap();
        summary(true, &mut m.api, &cat, PID).unwrap();
        archive_mock.assert();
        unarchive_mock.assert();
        summary_mock.assert();
    }

    #[test]
    fn deletes_policy_refuses_project_archive_and_delete() {
        let _g = crate::plane_env_files::lock_env();
        let _p = PolicyEnv::new("WEB", Some(DENY_DELETES));
        let mut server = mockito::Server::new();
        mock_me(&mut server);
        let post = server.mock("POST", Matcher::Any).expect(0).create();
        let del = server.mock("DELETE", Matcher::Any).expect(0).create();
        let mut m = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
        let cat = catalog();
        let e = archive(true, &mut m.api, &cat, "WEB").unwrap_err();
        assert_eq!(e.error_code(), "policy_violation");
        assert!(e.to_string().contains("projects archive"), "{e}");
        let e = delete(true, &mut m.api, &cat, "WEB").unwrap_err();
        assert_eq!(e.error_code(), "policy_violation");
        post.assert();
        del.assert();
    }
}
