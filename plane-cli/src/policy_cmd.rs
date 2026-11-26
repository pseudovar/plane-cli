//! `plane-cli config policy …`: write and show per-project write policies. No network.

use crate::dispatch::Cli;
use crate::policy;
use clap::{Subcommand, ValueHint};
use plane_core::{print_json, CoreError, ResponseEnvelope};
use serde_json::json;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum PolicyCmd {
    /// Write a starter policy for a project (search first, description sections, required fields, member rules)
    Init {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. WEB); the file is <PROJECT>.toml"
        )]
        project: String,
        #[arg(
            long = "owner",
            value_name = "MEMBER",
            action = clap::ArgAction::Append,
            help = "Member allowed to set Todo/Done/Cancelled and to delete (repeatable); without it those rules are written commented out"
        )]
        owners: Vec<String>,
        #[arg(
            long = "require",
            value_name = "FIELD",
            action = clap::ArgAction::Append,
            help = "Field work-items create must set (repeatable): priority, module, labels, assignees, state, start_date, target_date; default priority"
        )]
        required: Vec<String>,
        #[arg(
            long = "label",
            value_name = "NAME",
            action = clap::ArgAction::Append,
            help = "Allowed label name (repeatable); without it any label is allowed"
        )]
        labels: Vec<String>,
        #[arg(
            long,
            help = "Write ./.plane-cli/policy/<PROJECT>.toml (repo-local override) instead of the user config dir"
        )]
        local: bool,
        #[arg(long, help = "Replace an existing policy file")]
        overwrite: bool,
    },
    /// Show the active policy for a project and the paths searched
    Show {
        #[arg(
            long,
            value_name = "PROJECT",
            value_hint = ValueHint::Other,
            help = "Project identifier (e.g. WEB)"
        )]
        project: String,
    },
}

pub fn policy_cmd_name(cmd: &PolicyCmd) -> &'static str {
    match cmd {
        PolicyCmd::Init { .. } => "policy init",
        PolicyCmd::Show { .. } => "policy show",
    }
}

fn emit(cli: &Cli, data: serde_json::Value) -> Result<(), CoreError> {
    if cli.json_mode() {
        print_json(&ResponseEnvelope::success(data))?;
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&data).map_err(CoreError::Json)?
        );
    }
    Ok(())
}

fn init_path(project: &str, local: bool) -> Result<PathBuf, CoreError> {
    let dir = if local {
        std::env::current_dir()?.join(".plane-cli").join("policy")
    } else {
        policy::global_policy_dir().ok_or_else(|| {
            CoreError::Config(
                "cannot find the user config dir (HOME / XDG_CONFIG_HOME unset)".into(),
            )
        })?
    };
    Ok(dir.join(format!("{project}.toml")))
}

pub fn run(cli: &Cli, cmd: &PolicyCmd) -> Result<(), CoreError> {
    match cmd {
        PolicyCmd::Init {
            project,
            owners,
            required,
            labels,
            local,
            overwrite,
        } => {
            let project = policy::normalize_identifier(project)?;
            let owners = owners
                .iter()
                .map(|o| crate::identity::validate_member(o, "--owner"))
                .collect::<Result<Vec<_>, _>>()?;
            let path = init_path(&project, *local)?;
            if path.exists() && !overwrite {
                return Err(CoreError::usage_code(
                    "policy_exists",
                    format!(
                        "{} already exists; pass --overwrite to replace it",
                        path.display()
                    ),
                ));
            }
            let opts = policy::TemplateOptions {
                owners,
                required: required.clone(),
                labels: labels.clone(),
            };
            let text = policy::template(&project, &opts);
            // Never write a file plane-cli itself would refuse to load.
            let parsed = policy::Policy::parse(&text, &path)?;
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&path, text)?;
            emit(
                cli,
                json!({
                    "project": project,
                    "written": path.display().to_string(),
                    "source": if *local { "repo" } else { "global" },
                    "summary": parsed.summary(),
                }),
            )
        }
        PolicyCmd::Show { project } => {
            let project = policy::normalize_identifier(project)?;
            let (candidates, loaded) = policy::resolve(&project)?;
            let searched: Vec<_> = candidates.iter().map(|c| c.to_json()).collect();
            for w in plane_core::take_warnings() {
                if !cli.json_mode() {
                    eprintln!("warning: {w}");
                } else {
                    plane_core::push_warning(w);
                }
            }
            emit(
                cli,
                json!({
                    "project": project,
                    "active": loaded.is_some(),
                    "policy": loaded.map(|l| l.to_json()),
                    "searched": searched,
                }),
            )
        }
    }
}
