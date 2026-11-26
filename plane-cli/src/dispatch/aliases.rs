//! Near-miss spellings agents type, accepted with a warning.
//!
//! Flag aliases are hidden clap aliases (so `--help` shows only the canonical flag); clap does
//! not report which spelling was used, so [`flag_alias_warnings`] scans argv for them.
//! Command aliases (`issue`, `issues`, `ticket`, `work-item`) are forwarded to `work-items`
//! by [`forward_command_alias`] when the rest of the call parses.

use super::{Cli, Commands};
use clap::Parser;
use plane_core::CoreError;

/// (command path, hidden alias, canonical flag). Must match the `alias = "…"` attributes in
/// `dispatch/mod.rs`.
const FLAG_ALIASES: &[(&str, &str, &str)] = &[
    ("work-items comments get", "--comment-id", "--id"),
    ("work-items comments update", "--comment-id", "--id"),
    ("work-items comments delete", "--comment-id", "--id"),
    ("work-items links get", "--link-id", "--id"),
    ("work-items links update", "--link-id", "--id"),
    ("work-items links delete", "--link-id", "--id"),
    ("modules issues add", "--work-item", "--work-items"),
    ("modules issues remove", "--work-item", "--work-items"),
    ("cycles items add", "--work-item", "--work-items"),
    ("cycles items remove", "--work-item", "--work-items"),
];

fn argv_has_flag(argv: &[String], flag: &str) -> bool {
    let with_value = format!("{flag}=");
    argv.iter()
        .skip(1)
        .take_while(|a| a.as_str() != "--")
        .any(|a| a == flag || a.starts_with(&with_value))
}

/// One warning per hidden flag alias used in `argv` for this command path.
pub fn flag_alias_warnings(command_path: &str, argv: &[String]) -> Vec<String> {
    FLAG_ALIASES
        .iter()
        .filter(|(path, alias, _)| *path == command_path && argv_has_flag(argv, alias))
        .map(|(path, alias, canonical)| {
            format!("`{alias}` accepted as `{canonical}` on `{path}`; use `{canonical}` to suppress this warning")
        })
        .collect()
}

/// Names that the hidden alias command may have been invoked as.
fn alias_names(cmd: &Commands) -> Option<&'static [&'static str]> {
    match cmd {
        Commands::IssueAlias { .. } => Some(&["issue", "issues"]),
        Commands::TicketAlias { .. } => Some(&["ticket"]),
        Commands::WorkItemAlias { .. } => Some(&["work-item"]),
        _ => None,
    }
}

/// Guidance error text for the hidden alias commands that point at `work-items`.
pub fn alias_guidance(cmd: &Commands) -> Option<&'static str> {
    match cmd {
        Commands::WorkItemAlias { .. } => Some(
            "unknown command 'work-item'; use: plane-cli work-items <list|get|create|update|delete> --json\n\
             The command is plural: `work-items`, not `work-item`.",
        ),
        Commands::IssueAlias { .. } => Some(
            "unknown command 'issue'; use: plane-cli work-items <list|get|create|update|delete> --json\n\
             Plane work items are managed under `work-items`.",
        ),
        Commands::TicketAlias { .. } => Some(
            "unknown command 'ticket'; use: plane-cli work-items <list|get|create|update|delete> --json\n\
             Plane work items are managed under `work-items`.",
        ),
        _ => None,
    }
}

/// Re-parse `issue …` / `issues …` / `ticket …` / `work-item …` as `work-items …`.
///
/// Returns the CLI to run and an outcome:
/// - `Ok(Some(warning))`: forwarded to a valid `work-items` call;
/// - `Ok(None)`: not an alias call (or a bare alias); `cli` is unchanged;
/// - `Err(_)`: the forwarded call does not parse; the guidance error includes clap's message
///   for the forwarded call (e.g. an unknown `--per-page`).
pub fn forward_command_alias(
    cli: Cli,
    argv: &[String],
) -> (Cli, Result<Option<String>, CoreError>) {
    let Some(names) = alias_names(&cli.command) else {
        return (cli, Ok(None));
    };
    let Some(pos) = argv
        .iter()
        .skip(1)
        .position(|a| names.contains(&a.as_str()))
        .map(|i| i + 1)
    else {
        return (cli, Ok(None));
    };
    if pos + 1 >= argv.len() {
        // Bare `issue`: nothing to forward; dispatch emits the guidance error.
        return (cli, Ok(None));
    }
    let typed = argv[pos].clone();
    let mut forwarded = argv.to_vec();
    forwarded[pos] = "work-items".into();
    match Cli::try_parse_from(&forwarded) {
        Ok(new_cli) if matches!(new_cli.command, Commands::WorkItems { .. }) => {
            let rest = argv[pos + 1..].first().map(String::as_str).unwrap_or("…");
            let warning = format!(
                "`{typed}` is not a command; forwarded to `work-items {rest}`. Use `plane-cli work-items …` to suppress this warning"
            );
            (new_cli, Ok(Some(warning)))
        }
        Ok(_) => (cli, Ok(None)),
        Err(e)
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp
                    | clap::error::ErrorKind::DisplayVersion
                    | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            ) =>
        {
            (cli, Ok(None))
        }
        Err(e) => {
            let guidance = alias_guidance(&cli.command).unwrap_or("unknown command");
            let err = CoreError::Usage(format!(
                "{guidance}\nForwarded as `work-items …`, the call fails with:\n{}",
                e.to_string().trim_end()
            ));
            (cli, Err(err))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::{ModuleIssuesCmd, ModulesCmd};

    const ID: &str = "550e8400-e29b-41d4-a716-446655440000";

    fn argv(args: &[&str]) -> Vec<String> {
        std::iter::once("plane-cli")
            .chain(args.iter().copied())
            .map(String::from)
            .collect()
    }

    fn parse(args: &[String]) -> Cli {
        Cli::try_parse_from(args).expect("parses")
    }

    #[test]
    fn comment_id_is_accepted_with_one_warning() {
        let a = argv(&[
            "--json",
            "work-items",
            "comments",
            "delete",
            "--work-item",
            "ENG-1",
            "--comment-id",
            ID,
        ]);
        let cli = parse(&a);
        let w = flag_alias_warnings(&cli.command_path(), &a);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("`--comment-id` accepted as `--id`"));
    }

    #[test]
    fn comment_id_equals_form_is_detected() {
        let a = argv(&[
            "work-items",
            "comments",
            "get",
            "--work-item",
            "ENG-1",
            &format!("--comment-id={ID}"),
        ]);
        let cli = parse(&a);
        assert_eq!(flag_alias_warnings(&cli.command_path(), &a).len(), 1);
    }

    #[test]
    fn canonical_id_has_no_warning() {
        let a = argv(&[
            "work-items",
            "comments",
            "delete",
            "--work-item",
            "ENG-1",
            "--id",
            ID,
        ]);
        let cli = parse(&a);
        assert!(flag_alias_warnings(&cli.command_path(), &a).is_empty());
    }

    #[test]
    fn link_id_is_accepted_with_one_warning() {
        let a = argv(&[
            "work-items",
            "links",
            "get",
            "--work-item",
            "ENG-1",
            "--link-id",
            ID,
        ]);
        let cli = parse(&a);
        assert_eq!(flag_alias_warnings(&cli.command_path(), &a).len(), 1);
    }

    #[test]
    fn modules_issues_add_accepts_work_item_with_one_warning() {
        let a = argv(&[
            "modules",
            "issues",
            "add",
            "--project",
            "ENG",
            "--module",
            ID,
            "--work-item",
            ID,
        ]);
        let cli = parse(&a);
        match &cli.command {
            Commands::Modules {
                cmd:
                    ModulesCmd::Issues {
                        cmd: ModuleIssuesCmd::Add { work_items, .. },
                    },
            } => assert_eq!(work_items, &vec![ID.to_string()]),
            _ => panic!("expected modules issues add"),
        }
        let w = flag_alias_warnings(&cli.command_path(), &a);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("`--work-items`"));
    }

    #[test]
    fn work_item_on_other_commands_is_canonical() {
        let a = argv(&["work-items", "comments", "list", "--work-item", "ENG-1"]);
        let cli = parse(&a);
        assert!(flag_alias_warnings(&cli.command_path(), &a).is_empty());
    }

    #[test]
    fn issue_is_forwarded_with_one_warning() {
        let a = argv(&["--json", "issue", "list", "--project", "ENG"]);
        let (cli, w) = forward_command_alias(parse(&a), &a);
        assert_eq!(cli.command_path(), "work-items list");
        assert!(cli.json);
        assert!(w
            .unwrap()
            .unwrap()
            .contains("forwarded to `work-items list`"));
        assert!(flag_alias_warnings(&cli.command_path(), &a).is_empty());
    }

    #[test]
    fn issues_ticket_and_work_item_are_forwarded() {
        for name in ["issues", "ticket", "work-item"] {
            let a = argv(&[name, "get", "--work-item", "ENG-1"]);
            let (cli, w) = forward_command_alias(parse(&a), &a);
            assert_eq!(cli.command_path(), "work-items get", "{name}");
            assert!(w.unwrap().is_some());
        }
    }

    #[test]
    fn unparsable_alias_call_shows_guidance_and_forwarded_error() {
        let a = argv(&["issue", "list", "--project", "ENG", "--per-page", "5"]);
        let (cli, out) = forward_command_alias(parse(&a), &a);
        assert_eq!(cli.command_path(), "alias:issue");
        let err = out.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown command 'issue'"), "{msg}");
        assert!(msg.contains("--per-page"), "{msg}");
        assert_eq!(err.exit_kind().code(), 2);
    }

    #[test]
    fn bare_alias_is_left_to_dispatch() {
        let a = argv(&["issue"]);
        let (cli, out) = forward_command_alias(parse(&a), &a);
        assert_eq!(cli.command_path(), "alias:issue");
        assert!(matches!(out, Ok(None)));
    }

    #[test]
    fn hidden_aliases_stay_out_of_help() {
        use clap::CommandFactory;
        let mut cmd = Cli::command();
        let help = cmd
            .find_subcommand_mut("work-items")
            .and_then(|c| c.find_subcommand_mut("comments"))
            .and_then(|c| c.find_subcommand_mut("delete"))
            .expect("subcommand")
            .render_help()
            .to_string();
        assert!(help.contains("--id"));
        assert!(!help.contains("comment-id"));
    }
}
