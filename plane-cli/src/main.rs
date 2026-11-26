#![allow(clippy::result_large_err)]

mod plane_env_files;

mod body;
mod complete_catalog;
mod complete_cmd;
mod completion_hooks;
mod completions_install;
mod config_cmd;
mod dispatch;
mod identity;
mod net_diag;
mod output;
mod policy;
mod policy_cmd;
mod projects_context_cmd;
mod redact;
mod self_cmd;
mod skill_cmd;
mod telemetry;
mod tui;

use clap::{CommandFactory, Parser, ValueEnum};
use dispatch::Cli;
use dispatch::Commands;
use plane_core::{Config, CoreError, ExitKind, PlaneApi, ResponseEnvelope};
use std::io::{self, Write};

fn detect_completion_shell() -> clap_complete::Shell {
    let sh = std::env::var("SHELL").unwrap_or_default();
    let trimmed = sh.trim();
    if trimmed.is_empty() {
        return clap_complete::Shell::Bash;
    }
    let base = std::path::Path::new(trimmed)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match base.as_str() {
        "zsh" => clap_complete::Shell::Zsh,
        "fish" => clap_complete::Shell::Fish,
        "pwsh" | "powershell" => clap_complete::Shell::PowerShell,
        "elvish" => clap_complete::Shell::Elvish,
        _ => clap_complete::Shell::Bash,
    }
}

/// Env files in the documented order (process env → `--env-file` → `PLANE_DOTENV` →
/// `--dotenv` → local → global); see `plane_env_files::load_env_layers`.
fn load_dotenv_layers(cli: &Cli) -> Result<(), plane_env_files::EnvLoadError> {
    plane_env_files::load_env_layers(cli.env_file.as_deref(), cli.dotenv)
}

fn main() {
    // Before any env file is loaded: env files never override non-empty process vars, and
    // network errors can name the layer that set the URL.
    plane_env_files::snapshot_process_env();
    // Load env files before telemetry init so `PLANE_CLI_ANON_TELEMETRY` works from env files.
    // Clap has not run yet, so `--env-file` / `--dotenv` come from an argv pre-scan; the load
    // is repeated with the parsed flags below (it is idempotent) to surface errors.
    let (pre_env_file, pre_dotenv) =
        plane_env_files::prescan_env_args(&std::env::args().collect::<Vec<_>>());
    let _ = plane_env_files::load_env_layers(pre_env_file.as_deref(), pre_dotenv);
    redact::register_env_keys();

    telemetry::init();

    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e)
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayVersion | clap::error::ErrorKind::DisplayHelp
            ) =>
        {
            e.exit()
        }
        Err(e) => exit_clap_error(e),
    };

    // Near-miss spellings: forward `issue …` etc. to `work-items …`, warn on hidden flag aliases.
    let argv: Vec<String> = std::env::args().collect();
    let (cli, forwarded) = dispatch::forward_command_alias(cli, &argv);
    match forwarded {
        Ok(Some(w)) => plane_core::push_warning(w),
        Ok(None) => {}
        Err(e) => report_error_and_exit(&cli, e),
    }
    for w in dispatch::flag_alias_warnings(&cli.command_path(), &argv) {
        plane_core::push_warning(w);
    }

    telemetry::start_command_transaction(&cli.command_path(), cli.json_mode());

    if let dispatch::Commands::Completions {
        shell,
        list_shells,
        catalog_hooks,
        install,
        dry_run,
    } = &cli.command
    {
        if *list_shells {
            if *install {
                report_error_and_exit(
                    &cli,
                    CoreError::Usage("--list-shells cannot be used with --install".into()),
                );
            }
            for s in clap_complete::Shell::value_variants() {
                if let Some(pv) = s.to_possible_value() {
                    println!("{}", pv.get_name());
                }
            }
            telemetry::finish_command_transaction(sentry::protocol::SpanStatus::Ok);
            return;
        }
        let shell_eff = shell.unwrap_or_else(|| {
            if std::env::var("SHELL").unwrap_or_default().trim().is_empty() {
                eprintln!(
                    "note: SHELL unset or empty; emitting bash completions (pass bash|zsh|fish|elvish|powershell to select)"
                );
            }
            detect_completion_shell()
        });
        if *install {
            if let Err(e) = completions_install::run(shell_eff, *catalog_hooks, *dry_run) {
                report_error_and_exit(&cli, CoreError::Io(io::Error::other(e)));
            }
            telemetry::finish_command_transaction(sentry::protocol::SpanStatus::Ok);
            return;
        }
        let mut cmd = Cli::command();
        clap_complete::generate(shell_eff, &mut cmd, "plane-cli", &mut std::io::stdout());
        if *catalog_hooks {
            completion_hooks::print_catalog_hooks(shell_eff);
        }
        telemetry::finish_command_transaction(sentry::protocol::SpanStatus::Ok);
        return;
    }

    if let dispatch::Commands::SelfManage { cmd } = &cli.command {
        if let Err(e) = self_cmd::run(&cli, cmd) {
            report_error_and_exit(&cli, e);
        }
        telemetry::finish_command_transaction(sentry::protocol::SpanStatus::Ok);
        return;
    }

    if let Commands::Config { cmd } = &cli.command {
        if matches!(cmd, dispatch::ConfigCmd::Doctor { .. }) {
            if let Err(e) = load_dotenv_layers(&cli) {
                report_error_and_exit(&cli, e.into());
            }
        }
        if let Err(e) = config_cmd::run(&cli, cmd) {
            report_error_and_exit(&cli, e);
        }
        telemetry::finish_command_transaction(sentry::protocol::SpanStatus::Ok);
        return;
    }

    // Argument mistakes that need no config or network exit 2 even when Plane is down.
    if let Err(e) = dispatch::prevalidate(&cli) {
        report_error_and_exit(&cli, e);
    }

    if let Err(e) = load_dotenv_layers(&cli) {
        report_error_and_exit(&cli, e.into());
    }
    redact::register_env_keys();

    // `--as <member>` / PLANE_CLI_AS: that member's own key replaces PLANE_API_KEY.
    let member_key = match identity::resolve_acting(
        cli.as_member.as_deref(),
        cli.env_file.as_deref(),
        cli.dotenv,
    ) {
        Ok(Some(acting)) => {
            if let Err(e) = identity::refuse_cwd_url_for_member(
                plane_env_files::base_url_from_cwd(),
                &acting.member,
            ) {
                report_error_and_exit(&cli, e);
            }
            match identity::load_member_key(&acting.member, &identity::system_lookup()) {
                Ok(mk) => Some(mk),
                Err(e) => report_error_and_exit(&cli, e),
            }
        }
        Ok(None) => None,
        Err(e) => report_error_and_exit(&cli, e),
    };
    let config = match &member_key {
        Some(mk) => Config::from_env_with_key(cli.json, mk.key.clone(), Some(mk.member.clone())),
        None => Config::from_env(cli.json),
    };
    let config = match config {
        Ok(c) => c,
        Err(e) => report_error_and_exit(&cli, with_setup_hint(e)),
    };
    // Whichever key is sent is scrubbed from error output and telemetry.
    redact::register_secret(&config.api_key);
    let json_mode = cli.json_mode();
    let mut api = match PlaneApi::new(config) {
        Ok(a) => a,
        Err(e) => report_error_and_exit(&cli, e),
    };
    if let Some(mk) = &member_key {
        if let Err(e) = identity::verify_identity(&api, mk, cli.refresh) {
            report_error_and_exit(&cli, e);
        }
    }
    drop(member_key);

    if let Err(e) = dispatch::run(&cli, json_mode, &mut api) {
        if e.is_broken_pipe() {
            telemetry::shutdown();
            std::process::exit(ExitKind::Ok.code());
        }
        report_error_and_exit(&cli, e);
    }
    telemetry::finish_command_transaction(sentry::protocol::SpanStatus::Ok);
}

fn exit_clap_error(e: clap::Error) -> ! {
    let json_mode = telemetry::json_mode_from_argv();
    telemetry::start_command_transaction(&telemetry::argv_command_hint(), json_mode);
    if telemetry::should_capture_clap_error(&e) {
        telemetry::capture_clap_error(&e, json_mode);
    }
    telemetry::finish_command_transaction(sentry::protocol::SpanStatus::InvalidArgument);
    let mut cmd = Cli::command();
    // A stray positional could be a pasted key: never echo it.
    let positional = redact::clap_positional_value(&e);
    let usage = redact::redact_clap_text(positional.as_deref(), &e.format(&mut cmd).to_string());
    if json_mode {
        let err = CoreError::Usage(usage);
        let _ = output::emit_err(true, &err);
    } else {
        let _ = write_stderr(&usage);
        if let Some(hint) = hint_text_for_error(&CoreError::Usage(usage)) {
            let _ = write_stderr(&format!("hint: {hint}"));
        }
    }
    telemetry::shutdown();
    std::process::exit(ExitKind::Usage.code());
}

fn report_error_and_exit(cli: &Cli, err: CoreError) -> ! {
    if err.is_broken_pipe() {
        telemetry::shutdown();
        std::process::exit(ExitKind::Ok.code());
    }
    let err = net_diag::enrich_network_error(err, cli.env_file.as_deref(), cli.dotenv);
    let _ = output::emit_err(cli.json_mode(), &err);
    if !cli.json_mode() {
        if let Some(hint) = hint_text_for_error(&err) {
            let _ = write_stderr(&redact::redact_str(&format!("hint: {hint}")));
        }
    }
    telemetry::capture_core_error(&err, Some(&cli.command_path()), cli.json_mode());
    telemetry::finish_command_transaction(telemetry::span_status_for_core_error(&err));
    telemetry::shutdown();
    std::process::exit(err.exit_kind().code());
}

fn with_setup_hint(err: CoreError) -> CoreError {
    match err {
        CoreError::Config(msg) => CoreError::Config(format!(
            "{msg}. Next step: run `plane-cli config init` (or pass `--env-file PATH`)."
        )),
        other => other,
    }
}

fn write_stderr(text: &str) -> io::Result<()> {
    let stderr = io::stderr();
    let mut lock = stderr.lock();
    lock.write_all(text.as_bytes())?;
    lock.write_all(b"\n")?;
    Ok(())
}

fn hint_text_for_error(err: &CoreError) -> Option<String> {
    match err.root() {
        CoreError::Http {
            status: 404, path, ..
        } if path.contains("/projects/") && !path.contains("/work-items/") => {
            let slug = std::env::var("PLANE_WORKSPACE_SLUG").unwrap_or_else(|_| "<unset>".into());
            Some(format!(
                "workspace `{slug}` not found; verify PLANE_WORKSPACE_SLUG. \
                 Run `plane-cli auth whoami --json` to test connectivity."
            ))
        }
        CoreError::Http {
            status: 404, path, ..
        } if path.contains("/work-items/") || path.contains("/issues/") => Some(
            "work item not found; verify the id and workspace. \
                 If using a composite id, ensure the project prefix matches this workspace. \
                 Check PLANE_WORKSPACE_SLUG."
                .into(),
        ),
        CoreError::Http {
            status: 401 | 403, ..
        } if identity::current_member().is_some() => {
            let member = identity::current_member().unwrap_or("<member>");
            Some(format!(
                "acting as `{member}`: Plane refused this request with that member's key; check the member's project membership and role, or store a new key with `plane-cli config set-key --member {member}`."
            ))
        }
        CoreError::Http { status: 403, .. } => {
            let base = std::env::var("PLANE_API_BASE_URL").unwrap_or_else(|_| "<unset>".into());
            let base = base.trim_end_matches('/');
            Some(format!(
                "API token rejected; regenerate at {base}/profile/api-tokens/ and update PLANE_API_KEY."
            ))
        }
        CoreError::Http { status: 401, .. } => {
            Some("Authentication failed; verify PLANE_API_KEY is correct and not expired.".into())
        }
        CoreError::Unreachable {
            api_host,
            config_layer,
            config_path,
            ..
        } => {
            let from = match config_path {
                Some(p) => format!("{config_layer} ({p})"),
                None => config_layer.clone(),
            };
            Some(format!(
                "`{api_host}` did not answer; PLANE_API_BASE_URL comes from {from}. \
                 Run `plane-cli config where` and `plane-cli config doctor`."
            ))
        }
        CoreError::Network(e) => {
            let url = std::env::var("PLANE_API_BASE_URL").unwrap_or_else(|_| "<unset>".into());
            if e.is_connect() || e.is_request() || e.is_builder() {
                Some(format!(
                    "cannot reach `{url}`; check PLANE_API_BASE_URL (current: `{url}`). \
                     Verify the host is reachable and the URL has no typos."
                ))
            } else {
                None
            }
        }
        _ => {
            let env = ResponseEnvelope::from_core_error(err);
            env.error?
                .details?
                .get("hint")?
                .as_str()
                .map(str::to_string)
        }
    }
}
