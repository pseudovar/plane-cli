//! `plane-cli work-items attachments …`: files on a work item.
//!
//! Upload is three calls on CE 1.4.2: register the file (returns a presigned storage POST),
//! send the bytes to storage, then mark the attachment uploaded. `get` answers with a
//! presigned download URL (the API itself redirects there).
//! Checked against Plane Community Edition 1.4.2 (see CRUD-REFERENCE.md).

use super::helpers::{parse_uuid_flag, resolve_target};
use super::WorkItemTarget;
use crate::output;
use clap::{Subcommand, ValueHint};
use plane_core::{guess_mime, push_warning, CoreError, PlaneApi, ProjectEntry};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Subcommand)]
pub enum AttachmentsCmd {
    /// Attachments on a work item
    List {
        #[command(flatten)]
        target: WorkItemTarget,
    },
    /// One attachment's metadata and a short-lived `download_url`; `--output` saves the file
    Get {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(
            long,
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Attachment id"
        )]
        id: String,
        #[arg(
            long,
            value_name = "PATH",
            value_hint = ValueHint::FilePath,
            help = "Download the file to PATH (refuses to overwrite)"
        )]
        output: Option<String>,
    },
    /// Upload a file and attach it
    Create {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(long, value_name = "PATH", value_hint = ValueHint::FilePath)]
        file: String,
        #[arg(
            long,
            value_name = "NAME",
            help = "File name to show in Plane (default: PATH's name)"
        )]
        name: Option<String>,
        #[arg(
            long = "type",
            value_name = "MIME",
            help = "MIME type (default: guessed from the file name)"
        )]
        mime: Option<String>,
    },
    /// Delete an attachment
    Delete {
        #[command(flatten)]
        target: WorkItemTarget,
        #[arg(
            long,
            value_name = "UUID",
            value_hint = ValueHint::Other,
            help = "Attachment id"
        )]
        id: String,
    },
}

pub fn attachments_cmd_name(cmd: &AttachmentsCmd) -> &'static str {
    match cmd {
        AttachmentsCmd::List { .. } => "list",
        AttachmentsCmd::Get { .. } => "get",
        AttachmentsCmd::Create { .. } => "create",
        AttachmentsCmd::Delete { .. } => "delete",
    }
}

fn rows(v: &Value) -> Vec<Value> {
    v.as_array()
        .or_else(|| v.get("results").and_then(|x| x.as_array()))
        .cloned()
        .unwrap_or_default()
}

pub(super) fn run(
    cmd: &AttachmentsCmd,
    json_mode: bool,
    api: &mut PlaneApi,
    catalog: &[ProjectEntry],
    refresh: bool,
) -> Result<(), CoreError> {
    match cmd {
        AttachmentsCmd::List { target } => {
            let (pid, iid) = resolve_target(api, catalog, refresh, target)?;
            let v = api.list_attachments(&pid, &iid)?;
            output::emit_success(json_mode, v, None, false)?;
        }
        AttachmentsCmd::Get {
            target,
            id,
            output: out_path,
        } => {
            let (pid, iid) = resolve_target(api, catalog, refresh, target)?;
            let aid = parse_uuid_flag("--id", id)?;
            let list = api.list_attachments(&pid, &iid)?;
            let id_s = aid.to_string();
            let mut meta = rows(&list)
                .into_iter()
                .find(|r| r.get("id").and_then(|x| x.as_str()) == Some(id_s.as_str()))
                .ok_or_else(|| {
                    CoreError::Resolve(format!(
                        "no attachment {aid} on this work item; see `work-items attachments list`"
                    ))
                })?;
            let url = api.attachment_download_url(&pid, &iid, &aid)?;
            if let Some(path) = out_path {
                let p = Path::new(path);
                // create_new refuses existing paths, including dangling symlinks, in one
                // step (no exists-then-write race).
                let mut file = match std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(p)
                {
                    Ok(f) => f,
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        return Err(CoreError::Usage(format!(
                            "--output {path:?} already exists; choose a new path"
                        )));
                    }
                    Err(e) => return Err(e.into()),
                };
                let bytes = match api.download_presigned(&url) {
                    Ok(b) => b,
                    Err(e) => {
                        drop(file);
                        let _ = std::fs::remove_file(p);
                        return Err(e);
                    }
                };
                std::io::Write::write_all(&mut file, &bytes)?;
                if let Some(obj) = meta.as_object_mut() {
                    obj.insert("saved_to".into(), json!(path));
                    obj.insert("saved_bytes".into(), json!(bytes.len()));
                }
            } else {
                push_warning("download_url is presigned and expires; fetch it soon");
            }
            if let Some(obj) = meta.as_object_mut() {
                obj.insert("download_url".into(), json!(url));
            }
            output::emit_success(json_mode, meta, None, false)?;
        }
        AttachmentsCmd::Create {
            target,
            file,
            name,
            mime,
        } => {
            let (pid, iid) = resolve_target(api, catalog, refresh, target)?;
            let path = Path::new(file);
            let bytes = std::fs::read(path)
                .map_err(|e| CoreError::Usage(format!("--file {file:?}: {e}")))?;
            if bytes.is_empty() {
                return Err(CoreError::Usage(format!("--file {file:?} is empty")));
            }
            let file_name = match name {
                Some(n) => n.clone(),
                None => path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(String::from)
                    .ok_or_else(|| CoreError::Usage(format!("--file {file:?} has no file name")))?,
            };
            let mime = mime.clone().unwrap_or_else(|| guess_mime(&file_name));
            let v = api.upload_attachment(&pid, &iid, &file_name, &mime, bytes)?;
            output::emit_success(json_mode, v, None, false)?;
        }
        AttachmentsCmd::Delete { target, id } => {
            let (pid, iid) = resolve_target(api, catalog, refresh, target)?;
            let aid = parse_uuid_flag("--id", id)?;
            super::policy_gate::gate_delete(
                api,
                catalog,
                &pid,
                "attachment",
                "work-items attachments delete",
            )?;
            api.delete_attachment(&pid, &iid, &aid)?;
            output::emit_empty_success(json_mode)?;
        }
    }
    Ok(())
}
