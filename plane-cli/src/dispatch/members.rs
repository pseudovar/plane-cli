//! `plane-cli members list` (workspace), `projects members …` and `invitations …`.
//!
//! Checked against Plane Community Edition 1.4.2 (see CRUD-REFERENCE.md).
//! CE quirks: the project member list returns users (no membership id), update and remove
//! need the membership id that `add` returns, and remove only deactivates the membership,
//! after which the API cannot add that user back.

use super::helpers::parse_uuid_flag;
use super::PageArgs;
use crate::output;
use clap::{Subcommand, ValueHint};
use plane_core::{
    annotate_role_names, parse_role, resolve_member_ref, resolve_project_id, CoreError, PageParams,
    PlaneApi, ProjectEntry,
};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Subcommand)]
pub enum MembersCmd {
    /// Workspace members with their workspace role (adds `role_name`)
    List,
}

#[derive(Subcommand)]
pub enum ProjectMembersCmd {
    /// Project members with role and `is_active` (row `id` is the user id)
    List {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. ENG) or UUID"
        )]
        project: String,
        #[arg(long, help = "Also list removed (inactive) members")]
        include_inactive: bool,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Add a workspace member to the project; the result `id` is the membership id
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
            value_name = "USER",
            value_hint = ValueHint::Other,
            help = "Workspace member: user UUID, email or display name"
        )]
        member: String,
        #[arg(
            long,
            value_name = "ROLE",
            default_value = "member",
            help = "admin, member or guest"
        )]
        role: String,
    },
    /// Change a member's project role
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
            value_name = "MEMBERSHIP_UUID",
            value_hint = ValueHint::Other,
            help = "Membership id returned by `projects members add` (not the user id)"
        )]
        membership: String,
        #[arg(long, value_name = "ROLE", help = "admin, member or guest")]
        role: String,
    },
    /// Remove a member (Plane deactivates the membership; the API cannot re-add them)
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
            visible_alias = "id",
            value_name = "MEMBERSHIP_UUID",
            value_hint = ValueHint::Other,
            help = "Membership id returned by `projects members add` (not the user id)"
        )]
        membership: String,
    },
}

#[derive(Subcommand)]
pub enum InvitationsCmd {
    /// Pending workspace invitations
    List,
    /// Invite an email address to the workspace (Plane sends the email)
    Create {
        #[arg(long, value_name = "EMAIL")]
        email: String,
        #[arg(
            long,
            value_name = "ROLE",
            default_value = "member",
            help = "admin, member or guest"
        )]
        role: String,
    },
    /// Change the role of a pending invitation
    Update {
        #[arg(
            long,
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Invitation id from `invitations list`"
        )]
        id: String,
        #[arg(long, value_name = "ROLE", help = "admin, member or guest")]
        role: String,
    },
    /// Withdraw an invitation
    Delete {
        #[arg(
            long,
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Invitation id from `invitations list`"
        )]
        id: String,
    },
}

pub fn members_cmd_name(cmd: &MembersCmd) -> &'static str {
    match cmd {
        MembersCmd::List => "list",
    }
}

pub fn project_members_cmd_name(cmd: &ProjectMembersCmd) -> &'static str {
    match cmd {
        ProjectMembersCmd::List { .. } => "list",
        ProjectMembersCmd::Add { .. } => "add",
        ProjectMembersCmd::Update { .. } => "update",
        ProjectMembersCmd::Remove { .. } => "remove",
    }
}

pub fn invitations_cmd_name(cmd: &InvitationsCmd) -> &'static str {
    match cmd {
        InvitationsCmd::List => "list",
        InvitationsCmd::Create { .. } => "create",
        InvitationsCmd::Update { .. } => "update",
        InvitationsCmd::Delete { .. } => "delete",
    }
}

fn all_project_members(api: &PlaneApi, pid: &Uuid) -> Result<Vec<Value>, CoreError> {
    let page = PageParams::all();
    let (v, _, _) = api.list_project_members(pid, &page)?;
    Ok(v.get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default())
}

/// GET the membership; on 404, explain when the caller passed a user id instead.
fn check_membership(api: &PlaneApi, pid: &Uuid, mid: &Uuid) -> Result<Value, CoreError> {
    match api.get_project_membership(pid, mid) {
        Ok(user) => Ok(user),
        Err(CoreError::Http { status: 404, .. }) => {
            let rows = all_project_members(api, pid)?;
            let is_user = rows
                .iter()
                .any(|r| r.get("id").and_then(|x| x.as_str()) == Some(mid.to_string().as_str()));
            let msg = if is_user {
                format!(
                    "{mid} is a user id, not a membership id. The CE API only returns membership ids from `projects members add`; for members added in the Plane UI, change roles there"
                )
            } else {
                format!("no membership {mid} in this project; use the `id` returned by `projects members add`")
            };
            Err(CoreError::Resolve(msg))
        }
        Err(e) => Err(e),
    }
}

pub(super) fn run_workspace(
    cmd: &MembersCmd,
    json_mode: bool,
    api: &mut PlaneApi,
) -> Result<(), CoreError> {
    match cmd {
        MembersCmd::List => {
            let mut v = api.list_workspace_members()?;
            annotate_role_names(&mut v);
            output::emit_success(json_mode, v, None, false)?;
        }
    }
    Ok(())
}

pub(super) fn run_project(
    cmd: &ProjectMembersCmd,
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
) -> Result<(), CoreError> {
    match cmd {
        ProjectMembersCmd::List {
            project,
            include_inactive,
            page,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let (mut v, nc, tr) = api.list_project_members(&pid, &page.params())?;
            // Keep Plane's `count` (all memberships, active or not, on this page) and add
            // `active_count` for the rows that are still active.
            let mut active = 0usize;
            if let Some(arr) = v.get_mut("results").and_then(|x| x.as_array_mut()) {
                active = arr
                    .iter()
                    .filter(|r| r.get("is_active").and_then(|x| x.as_bool()) != Some(false))
                    .count();
                if !include_inactive {
                    arr.retain(|r| r.get("is_active").and_then(|x| x.as_bool()) != Some(false));
                }
            }
            if let Some(obj) = v.as_object_mut() {
                obj.insert("active_count".into(), json!(active));
            }
            annotate_role_names(&mut v);
            output::emit_success(json_mode, v, nc, tr)?;
        }
        ProjectMembersCmd::Add {
            project,
            member,
            role,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let role = parse_role(role)?;
            let ws = api.list_workspace_members()?;
            let uid = resolve_member_ref(&ws, member)?;
            let uid_s = uid.to_string();
            let in_workspace = ws
                .as_array()
                .map(|a| {
                    a.iter()
                        .any(|r| r.get("id").and_then(|x| x.as_str()) == Some(uid_s.as_str()))
                })
                .unwrap_or(false);
            if !in_workspace {
                return Err(CoreError::Resolve(format!(
                    "{uid} is not a workspace member; invite them first (`invitations create`)"
                )));
            }
            let existing = all_project_members(api, &pid)?;
            if let Some(row) = existing
                .iter()
                .find(|r| r.get("id").and_then(|x| x.as_str()) == Some(uid_s.as_str()))
            {
                let active = row.get("is_active").and_then(|x| x.as_bool()) != Some(false);
                let msg = if active {
                    format!("{uid} is already a member of this project")
                } else {
                    format!(
                        "{uid} was removed from this project earlier; Plane CE keeps the inactive membership and its API cannot re-add them. Re-add them in the Plane UI"
                    )
                };
                return Err(CoreError::Usage(msg));
            }
            let mut v = api.add_project_member(&pid, &json!({ "member": uid_s, "role": role }))?;
            annotate_role_names(&mut v);
            output::emit_success(json_mode, v, None, false)?;
        }
        ProjectMembersCmd::Update {
            project,
            membership,
            role,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let role = parse_role(role)?;
            let mid = parse_uuid_flag("--membership", membership)?;
            check_membership(api, &pid, &mid)?;
            let mut v = api.update_project_member(&pid, &mid, &json!({ "role": role }))?;
            annotate_role_names(&mut v);
            output::emit_success(json_mode, v, None, false)?;
        }
        ProjectMembersCmd::Remove {
            project,
            membership,
        } => {
            let pid = resolve_project_id(project, catalog)?;
            let mid = parse_uuid_flag("--membership", membership)?;
            let user = check_membership(api, &pid, &mid)?;
            api.remove_project_member(&pid, &mid)?;
            output::emit_success(
                json_mode,
                json!({
                    "id": mid.to_string(),
                    "member": user.get("id").cloned().unwrap_or(Value::Null),
                    "is_active": false,
                }),
                None,
                false,
            )?;
        }
    }
    Ok(())
}

pub(super) fn run_invitations(
    cmd: &InvitationsCmd,
    json_mode: bool,
    api: &mut PlaneApi,
) -> Result<(), CoreError> {
    match cmd {
        InvitationsCmd::List => {
            let mut v = api.list_invitations()?;
            annotate_role_names(&mut v);
            output::emit_success(json_mode, v, None, false)?;
        }
        InvitationsCmd::Create { email, role } => {
            let role = parse_role(role)?;
            if !email.contains('@') {
                return Err(CoreError::Usage(format!(
                    "--email {email:?} is not an email address"
                )));
            }
            let mut v = api.create_invitation(&json!({ "email": email.trim(), "role": role }))?;
            annotate_role_names(&mut v);
            output::emit_success(json_mode, v, None, false)?;
        }
        InvitationsCmd::Update { id, role } => {
            let iid = parse_uuid_flag("--id", id)?;
            let role = parse_role(role)?;
            let mut v = api.update_invitation(&iid, &json!({ "role": role }))?;
            annotate_role_names(&mut v);
            output::emit_success(json_mode, v, None, false)?;
        }
        InvitationsCmd::Delete { id } => {
            let iid = parse_uuid_flag("--id", id)?;
            api.delete_invitation(&iid)?;
            output::emit_empty_success(json_mode)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Matcher;

    const INV: &str = "66666666-6666-4666-8666-666666666666";

    #[test]
    fn invitation_update_patches_the_role() {
        let _g = crate::plane_env_files::lock_env();
        let mut server = mockito::Server::new();
        let patch = server
            .mock(
                "PATCH",
                format!("/api/v1/workspaces/ws/invitations/{INV}/").as_str(),
            )
            .match_body(Matcher::Json(json!({"role": 5})))
            .with_body(format!(
                r#"{{"id":"{INV}","email":"a@example.com","role":5,"accepted":false}}"#
            ))
            .expect(1)
            .create();
        let mut m = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
        let cmd = InvitationsCmd::Update {
            id: INV.into(),
            role: "guest".into(),
        };
        run_invitations(&cmd, true, &mut m.api).unwrap();
        patch.assert();
    }

    #[test]
    fn invitation_update_rejects_bad_flags_before_calling_plane() {
        let _g = crate::plane_env_files::lock_env();
        let mut server = mockito::Server::new();
        let any = server.mock("PATCH", Matcher::Any).expect(0).create();
        let mut m = crate::identity::tests::MockApi::new(&server.url(), "bob", "plane_api_k");
        let bad_role = InvitationsCmd::Update {
            id: INV.into(),
            role: "owner".into(),
        };
        let e = run_invitations(&bad_role, true, &mut m.api).unwrap_err();
        assert_eq!(e.exit_kind().code(), 2);
        let bad_id = InvitationsCmd::Update {
            id: "not-a-uuid".into(),
            role: "member".into(),
        };
        let e = run_invitations(&bad_id, true, &mut m.api).unwrap_err();
        assert_eq!(e.exit_kind().code(), 2);
        any.assert();
        assert_eq!(
            invitations_cmd_name(&InvitationsCmd::Update {
                id: INV.into(),
                role: "member".into()
            }),
            "update"
        );
    }
}
