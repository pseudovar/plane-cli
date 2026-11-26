//! Opt-in anonymous error telemetry (`PLANE_CLI_ANON_TELEMETRY=1`).

use plane_core::CoreError;
use sentry::{ClientInitGuard, Level};
use std::sync::{Mutex, OnceLock};

/// Sentry DSN, set at build time by the release workflow. A source build has none, so
/// telemetry stays inactive there even when `PLANE_CLI_ANON_TELEMETRY` is set.
const BUILD_DSN: Option<&str> = option_env!("PLANE_CLI_SENTRY_DSN");

/// Value reported instead of the machine's hostname, which Sentry would otherwise add.
const SERVER_NAME: &str = "plane-cli";

static GUARD: OnceLock<Option<ClientInitGuard>> = OnceLock::new();
static ACTIVE_TRANSACTION: OnceLock<Mutex<Option<sentry::Transaction>>> = OnceLock::new();

fn env_truthy(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let v = v.trim().to_lowercase();
            v == "1" || v == "true" || v == "yes"
        }
        Err(_) => false,
    }
}

/// Master switch for anonymous telemetry (`PLANE_CLI_ANON_TELEMETRY`).
pub fn enabled() -> bool {
    env_truthy("PLANE_CLI_ANON_TELEMETRY")
}

/// A usable DSN from a build-time value: blank (an unset CI secret) or malformed counts as none.
fn usable_dsn(raw: Option<&str>) -> Option<&str> {
    let dsn = raw.map(str::trim).filter(|d| !d.is_empty())?;
    dsn.parse::<sentry::types::Dsn>().ok().map(|_| dsn)
}

/// Whether this binary was built with a telemetry endpoint.
pub fn build_has_dsn() -> bool {
    usable_dsn(BUILD_DSN).is_some()
}

fn client_options() -> sentry::ClientOptions {
    sentry::ClientOptions {
        release: sentry::release_name!(),
        send_default_pii: false,
        server_name: Some(SERVER_NAME.into()),
        traces_sample_rate: 1.0,
        // Last line of defence on every event, panics included.
        before_send: Some(std::sync::Arc::new(|mut ev| {
            scrub_event(&mut ev);
            Some(ev)
        })),
        ..Default::default()
    }
}

/// Scrub keys from an event and drop exception text. Every error is sent as a fixed message, so
/// an exception here is a panic, whose message can quote workspace data. Its type, location and
/// stack trace stay.
fn scrub_event(ev: &mut sentry::protocol::Event<'static>) {
    crate::redact::redact_event(ev);
    for ex in ev.exception.values.iter_mut() {
        ex.value = Some("message omitted".into());
    }
}

/// Initialize Sentry when opted in and the build has a DSN. Safe to call multiple times; only
/// the first call configures the client.
pub fn init() {
    let dsn = usable_dsn(BUILD_DSN).filter(|_| enabled());
    let Some(dsn) = dsn else {
        let _ = GUARD.set(None);
        return;
    };
    let guard = sentry::init((dsn, client_options()));
    let _ = GUARD.set(Some(guard));
}

pub fn is_active() -> bool {
    GUARD.get().is_some_and(|g| g.is_some()) && sentry::Hub::current().client().is_some()
}

/// Flush pending events before `process::exit` (guard drop is skipped on immediate exit).
pub fn shutdown() {
    finish_command_transaction(sentry::protocol::SpanStatus::Ok);
    if let Some(client) = sentry::Hub::current().client() {
        client.close(None);
    }
}

pub fn start_command_transaction(command_path: &str, json_mode: bool) {
    if !is_active() {
        return;
    }
    let transaction = sentry::start_transaction(sentry::TransactionContext::new(
        transaction_name(command_path),
        "cli.command",
    ));
    transaction.set_tag("json_mode", bool_tag(json_mode));
    if !command_path.is_empty() {
        transaction.set_tag("command", command_path);
    }
    sentry::configure_scope(|scope| scope.set_span(Some(transaction.clone().into())));
    *active_transaction().lock().unwrap() = Some(transaction);
}

pub fn finish_command_transaction(status: sentry::protocol::SpanStatus) {
    let transaction = active_transaction().lock().unwrap().take();
    if let Some(transaction) = transaction {
        transaction.set_status(status);
        sentry::configure_scope(|scope| scope.set_span(None));
        transaction.finish();
    }
}

pub fn span_status_for_core_error(err: &CoreError) -> sentry::protocol::SpanStatus {
    match err.root() {
        CoreError::Usage(_)
        | CoreError::UsageCode { .. }
        | CoreError::Config(_)
        | CoreError::Resolve(_) => sentry::protocol::SpanStatus::InvalidArgument,
        CoreError::Http { status, .. } => match *status {
            401 => sentry::protocol::SpanStatus::Unauthenticated,
            403 => sentry::protocol::SpanStatus::PermissionDenied,
            404 => sentry::protocol::SpanStatus::NotFound,
            409 => sentry::protocol::SpanStatus::AlreadyExists,
            429 => sentry::protocol::SpanStatus::ResourceExhausted,
            400..=499 => sentry::protocol::SpanStatus::InvalidArgument,
            501 => sentry::protocol::SpanStatus::Unimplemented,
            503 => sentry::protocol::SpanStatus::Unavailable,
            500..=599 => sentry::protocol::SpanStatus::InternalError,
            _ => sentry::protocol::SpanStatus::UnknownError,
        },
        CoreError::Network(_) | CoreError::Unreachable { .. } => {
            sentry::protocol::SpanStatus::Unavailable
        }
        CoreError::Json(_)
        | CoreError::Url(_)
        | CoreError::Io(_)
        | CoreError::Cache(_)
        | CoreError::UnexpectedResponse(_) => sentry::protocol::SpanStatus::InternalError,
        // root() never returns this variant; listed for exhaustiveness.
        CoreError::WithDetails { .. } => sentry::protocol::SpanStatus::UnknownError,
    }
}

pub fn json_mode_from_argv() -> bool {
    plane_core::effective_json_output(std::env::args().any(|a| a == "--json"))
}

/// Best-effort command path from argv (used when clap parse fails).
pub fn argv_command_hint() -> String {
    command_path_from_argv(&std::env::args().collect::<Vec<_>>())
}

/// Subcommand path (`work-items comments list`) by walking argv against the clap tree.
/// Flag values and positionals (ids, names) are never included.
pub fn command_path_from_argv(argv: &[String]) -> String {
    use clap::CommandFactory;
    let root = crate::dispatch::Cli::command();
    let mut cur = &root;
    let mut parts: Vec<String> = Vec::new();
    for tok in argv.iter().skip(1) {
        if tok == "--" {
            break;
        }
        if tok.starts_with('-') {
            continue;
        }
        match cur.find_subcommand(tok) {
            Some(sc) => {
                parts.push(sc.get_name().to_string());
                cur = sc;
            }
            None if parts.is_empty() => continue,
            None => break,
        }
    }
    parts.join(" ")
}

/// Sentry fingerprint for runtime errors that should group by cause rather than message.
/// Network failures share one group: the host is never sent.
pub fn core_error_fingerprint(err: &CoreError) -> Option<Vec<String>> {
    match err {
        CoreError::Unreachable { .. } | CoreError::Network(_) => Some(vec!["network_error".into()]),
        CoreError::UsageCode { code, .. } => Some(vec!["usage".into(), (*code).into()]),
        // The message is a fixed string naming the missing field (no ids or paths).
        CoreError::UnexpectedResponse(msg) => Some(vec!["unexpected_response".into(), msg.clone()]),
        // A step failed after earlier steps succeeded: group by what failed, never by the
        // note (it names work items, asset ids and paths).
        CoreError::WithDetails { inner, .. } => {
            let root = inner.root();
            let mut fp = vec!["partial_failure".to_string(), root.error_code().to_string()];
            if let Some(inner_fp) = core_error_fingerprint(root) {
                fp.extend(inner_fp);
            }
            Some(fp)
        }
        _ => None,
    }
}

/// Sentry fingerprint for clap usage errors: command path plus the offending flag or
/// subcommand, so the same mistake groups together whatever values were passed.
pub fn clap_error_fingerprint(err: &clap::Error, command_path: &str) -> Vec<String> {
    use clap::error::ContextKind;
    // For a stray positional, clap's InvalidArg is the value itself (an id or name): use a
    // constant so the same mistake groups together and values stay out of the fingerprint.
    let offending = match err.get(ContextKind::InvalidArg).map(|v| v.to_string()) {
        Some(arg) if arg.starts_with('-') => safe_flag(&arg),
        Some(_) => "<positional>".to_string(),
        // A mistyped subcommand is free text: it could be a project or person's name.
        None if err.get(ContextKind::InvalidSubcommand).is_some() => "<subcommand>".to_string(),
        None => format!("{:?}", err.kind()),
    };
    let path = if command_path.is_empty() {
        "plane-cli"
    } else {
        command_path
    };
    vec!["clap_parse".into(), path.into(), offending]
}

/// A flag's name without any value, or `<flag>` if it does not look like a flag name.
fn safe_flag(arg: &str) -> String {
    let name = arg.split(['=', ' ']).next().unwrap_or("");
    let body = name.trim_start_matches('-');
    let ok = !body.is_empty()
        && name.len() <= 40
        && body
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        name.to_string()
    } else {
        "<flag>".to_string()
    }
}

pub fn capture_core_error(err: &CoreError, command_path: Option<&str>, json_mode: bool) {
    if !is_active() {
        return;
    }
    capture_core_error_inner(err, command_path, json_mode);
}

/// Refusals plane-cli makes on purpose (the project's write policy): the caller gets the
/// reason and the fix in the envelope; they are not faults and never go to Sentry.
pub fn is_expected_refusal(err: &CoreError) -> bool {
    matches!(
        err.root(),
        CoreError::UsageCode { code, .. }
            if matches!(*code, "policy_violation" | "policy_invalid" | "policy_unknown_state")
    )
}

fn capture_core_error_inner(err: &CoreError, command_path: Option<&str>, json_mode: bool) {
    if is_expected_refusal(err) {
        return;
    }
    let fingerprint = core_error_fingerprint(err);
    sentry::with_scope(
        |scope| {
            scope.set_tag("error.kind", "core_error");
            scope.set_tag("error.code", err.error_code());
            scope.set_tag("exit_code", err.exit_kind().code().to_string());
            scope.set_tag("json_mode", bool_tag(json_mode));
            if let Some(cmd) = command_path.filter(|s| !s.is_empty()) {
                scope.set_tag("command", cmd);
            }
            if let Some(fp) = &fingerprint {
                let parts: Vec<&str> = fp.iter().map(String::as_str).collect();
                scope.set_fingerprint(Some(&parts));
            }
            if matches!(
                err.root(),
                CoreError::Network(_) | CoreError::Unreachable { .. }
            ) {
                scope.set_level(Some(Level::Warning));
            }
            if let CoreError::Unreachable { config_layer, .. } = err.root() {
                scope.set_tag("config_layer", config_layer);
            }
            if let CoreError::Http {
                status,
                method,
                path,
                ..
            } = err.root()
            {
                scope.set_tag("http.status", status.to_string());
                scope.set_tag("http.method", method);
                scope.set_extra("http.path", normalize_path(path).into());
            }
        },
        || match err {
            // The user-facing message names the host and config file; send the layer only.
            CoreError::Unreachable { config_layer, .. } => {
                sentry::capture_message(
                    &format!(
                        "network error: cannot reach Plane API (PLANE_API_BASE_URL from {config_layer})"
                    ),
                    Level::Warning,
                );
            }
            // The message can carry a local path (e.g. `--body /home/x/…`); send the code only.
            CoreError::UsageCode { code, .. } => {
                sentry::capture_message(&format!("usage error: {code}"), Level::Error);
            }
            // The message is the server's `detail`, which can echo request data; send status,
            // method and normalised path only.
            CoreError::Http {
                status,
                method,
                path,
                ..
            } => {
                sentry::capture_message(
                    &format!(
                        "HTTP {status} {method} {}: {}",
                        normalize_path(path),
                        err.error_code()
                    ),
                    Level::Error,
                );
            }
            // The message names work items, attachment ids or local paths; send only the
            // failing error's code (HTTP status, method and normalized path are tags).
            CoreError::WithDetails { inner, .. } => {
                sentry::capture_message(
                    &format!(
                        "partial failure: {} after earlier steps succeeded",
                        inner.root().error_code()
                    ),
                    Level::Error,
                );
            }
            // Resolve, usage, config, I/O, JSON and transport messages can name projects,
            // states, members, ids, paths or URLs; send the code only.
            _ => {
                sentry::capture_message(&format!("error: {}", err.error_code()), Level::Error);
            }
        },
    );
}

pub fn capture_clap_error(err: &clap::Error, json_mode: bool) {
    if !is_active() || !should_capture_clap_error(err) {
        return;
    }
    capture_clap_error_inner(err, &argv_command_hint(), json_mode);
}

fn capture_clap_error_inner(err: &clap::Error, command_hint: &str, json_mode: bool) {
    let command_hint = command_hint.to_string();
    let fingerprint: Vec<String> = clap_error_fingerprint(err, &command_hint)
        .iter()
        .map(|s| crate::redact::redact_str(s))
        .collect();
    sentry::with_scope(
        |scope| {
            scope.set_tag("error.kind", "clap_parse");
            scope.set_tag("error.code", "validation_error");
            scope.set_tag("exit_code", "2");
            scope.set_tag("json_mode", bool_tag(json_mode));
            if !command_hint.is_empty() {
                scope.set_tag("command", &command_hint);
            }
            let parts: Vec<&str> = fingerprint.iter().map(String::as_str).collect();
            scope.set_fingerprint(Some(&parts));
        },
        || {
            sentry::capture_message(&clap_error_message(err), Level::Error);
        },
    );
}

/// Sentry message for a clap error: the error kind and the offending flag or subcommand, with
/// a stray positional shown as `<positional>`. Never clap's full text, which echoes argv
/// (a pasted key, ids, paths).
pub fn clap_error_message(err: &clap::Error) -> String {
    let fp = clap_error_fingerprint(err, "");
    crate::redact::redact_str(&format!("clap {:?}: {}", err.kind(), fp[2]))
}

pub fn should_capture_clap_error(err: &clap::Error) -> bool {
    if matches!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelp
            | clap::error::ErrorKind::DisplayVersion
            | clap::error::ErrorKind::MissingSubcommand
            | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    ) {
        return false;
    }
    let msg = err.to_string();
    if msg.trim().starts_with("plane-cli ") && !msg.contains('\n') {
        return false;
    }
    if msg.starts_with("CLI for Plane (self-hosted).")
        || msg.contains("\nUsage: plane-cli [OPTIONS] <COMMAND>\n\nCommands:\n")
        || msg.contains("requires a subcommand but one was not provided")
    {
        return false;
    }
    true
}

/// The worker's error text can carry server detail or local paths, so only a fixed message is
/// sent.
pub fn capture_tui_worker() {
    if !is_active() {
        return;
    }
    sentry::with_scope(
        |scope| {
            scope.set_tag("error.kind", "tui_worker");
            scope.set_tag("surface", "tui_worker");
        },
        || {
            sentry::capture_message("tui worker job failed", Level::Error);
        },
    );
}

fn active_transaction() -> &'static Mutex<Option<sentry::Transaction>> {
    ACTIVE_TRANSACTION.get_or_init(|| Mutex::new(None))
}

fn transaction_name(command_path: &str) -> &str {
    if command_path.is_empty() {
        "plane-cli"
    } else {
        command_path
    }
}

fn bool_tag(v: bool) -> &'static str {
    if v {
        "true"
    } else {
        "false"
    }
}

/// Fixed words of Plane API paths. Every other segment is a slug, identifier or id.
const API_PATH_WORDS: &[&str] = &[
    "activities",
    "api",
    "archive",
    "archived-cycles",
    "archived-modules",
    "attachments",
    "comments",
    "cycle-issues",
    "cycles",
    "features",
    "intake-issues",
    "invitations",
    "issues",
    "labels",
    "links",
    "me",
    "members",
    "module-issues",
    "modules",
    "project-members",
    "project-members-lite",
    "projects",
    "relations",
    "search",
    "states",
    "summary",
    "transfer-issues",
    "unarchive",
    "update",
    "users",
    "v1",
    "work-items",
    "workspaces",
];

/// Keep only fixed API words: workspace slugs, project identifiers and ids become `{id}`, and
/// any query string is dropped.
pub fn normalize_path(path: &str) -> String {
    path.split('?')
        .next()
        .unwrap_or("")
        .split('/')
        .map(|seg| {
            if seg.is_empty() || API_PATH_WORDS.contains(&seg) {
                seg
            } else {
                "{id}"
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::Cli;
    use clap::Parser;

    #[test]
    fn normalize_path_replaces_uuids() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        let path = format!("/api/v1/workspaces/acme/projects/{uuid}/issues/");
        assert_eq!(
            normalize_path(&path),
            "/api/v1/workspaces/{id}/projects/{id}/issues/"
        );
    }

    #[test]
    fn normalize_path_keeps_only_fixed_api_words() {
        assert_eq!(
            normalize_path("/api/v1/workspaces/acme/projects/ENG/cycles/Sprint%2012/?q=alice"),
            "/api/v1/workspaces/{id}/projects/{id}/cycles/{id}/"
        );
        assert_eq!(normalize_path("/api/v1/users/me/"), "/api/v1/users/me/");
    }

    #[test]
    fn workspace_data_never_reaches_an_event() {
        let private = [
            "Secret Project",
            "alice@example.com",
            "plane.example.com",
            "/home/alice/notes.json",
            "550e8400-e29b-41d4-a716-446655440000",
        ];
        let text = private.join(" ");
        let errors = [
            CoreError::Resolve(text.clone()),
            CoreError::Usage(text.clone()),
            CoreError::Config(text.clone()),
            CoreError::Cache(text.clone()),
            CoreError::Io(std::io::Error::other(text.clone())),
            CoreError::Unreachable {
                message: text.clone(),
                api_host: "plane.example.com:443".into(),
                config_layer: "global".into(),
                config_path: Some("/home/alice/notes.json".into()),
                reason: text.clone(),
            },
            CoreError::Http {
                status: 404,
                method: "GET".into(),
                path: "/api/v1/workspaces/alice@example.com/projects/Secret Project/".into(),
                message: text.clone(),
                body: Some(serde_json::json!({ "detail": text })),
                rate_limit: None,
            },
        ];
        let events = sentry::test::with_captured_events(|| {
            for e in &errors {
                capture_core_error_inner(e, Some("work-items get"), true);
            }
        });
        assert_eq!(events.len(), errors.len());
        for ev in &events {
            let s = serde_json::to_string(ev).unwrap();
            for p in private {
                assert!(!s.contains(p), "{p} leaked: {s}");
            }
        }
        assert_eq!(
            events[0].message.as_deref(),
            Some("error: validation_error")
        );
    }

    #[test]
    fn panic_text_is_dropped_but_the_stack_trace_stays() {
        let mut ev = sentry::protocol::Event::default();
        ev.exception.values.push(sentry::protocol::Exception {
            ty: "panic".into(),
            value: Some("called unwrap on: project Secret Project".into()),
            stacktrace: Some(sentry::protocol::Stacktrace {
                frames: vec![sentry::protocol::Frame {
                    function: Some("plane_cli::main".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        scrub_event(&mut ev);
        let s = serde_json::to_string(&ev).unwrap();
        assert!(!s.contains("Secret Project"), "{s}");
        assert!(s.contains("plane_cli::main"), "{s}");
    }

    #[test]
    fn flag_names_lose_values() {
        assert_eq!(safe_flag("--bogus-flag"), "--bogus-flag");
        assert_eq!(safe_flag("--name=Secret"), "--name");
        assert_eq!(safe_flag("--priority <PRIORITY>"), "--priority");
        assert_eq!(safe_flag("--Plane_api_x"), "<flag>");
        assert_eq!(safe_flag("-"), "<flag>");
    }

    #[test]
    fn blank_or_malformed_dsn_counts_as_none() {
        assert_eq!(usable_dsn(None), None);
        assert_eq!(usable_dsn(Some("")), None);
        assert_eq!(usable_dsn(Some("   ")), None);
        assert_eq!(usable_dsn(Some("not a dsn")), None);
        let ok = "https://public@sentry.example.com/1";
        assert_eq!(usable_dsn(Some(ok)), Some(ok));
    }

    #[test]
    fn client_options_do_not_report_the_machine_hostname() {
        let o = client_options();
        assert_eq!(o.server_name.as_deref(), Some(SERVER_NAME));
        assert!(!o.send_default_pii);
    }

    #[test]
    fn enabled_false_when_env_unset() {
        std::env::remove_var("PLANE_CLI_ANON_TELEMETRY");
        assert!(!enabled());
    }

    #[test]
    fn help_and_version_errors_are_not_captured() {
        let help = clap::Error::raw(clap::error::ErrorKind::DisplayHelp, "help");
        let version = clap::Error::raw(clap::error::ErrorKind::DisplayVersion, "plane-cli 0.1.13");
        assert!(!should_capture_clap_error(&help));
        assert!(!should_capture_clap_error(&version));
    }

    #[test]
    fn bare_cli_help_like_error_is_not_captured() {
        let err = Cli::try_parse_from(["plane-cli"]).err().unwrap();
        assert!(!should_capture_clap_error(&err));
    }

    #[test]
    fn missing_subcommand_is_not_captured() {
        let err = Cli::try_parse_from(["plane-cli", "projects"])
            .err()
            .unwrap();
        assert!(!should_capture_clap_error(&err));
    }

    #[test]
    fn unknown_subcommand_error_is_captured() {
        let err = Cli::try_parse_from(["plane-cli", "nonexistent-command", "foo"])
            .err()
            .unwrap();
        assert!(should_capture_clap_error(&err));
    }

    #[test]
    fn span_status_maps_usage_and_http_errors() {
        assert_eq!(
            span_status_for_core_error(&CoreError::Usage("bad args".into())),
            sentry::protocol::SpanStatus::InvalidArgument
        );
        assert_eq!(
            span_status_for_core_error(&CoreError::Http {
                status: 404,
                method: "GET".into(),
                path: "/api/v1/projects/demo".into(),
                message: "missing".into(),
                body: None,
                rate_limit: None,
            }),
            sentry::protocol::SpanStatus::NotFound
        );
    }

    fn argv(args: &[&str]) -> Vec<String> {
        std::iter::once("plane-cli")
            .chain(args.iter().copied())
            .map(String::from)
            .collect()
    }

    #[test]
    fn command_path_from_argv_skips_values() {
        let a = argv(&[
            "--json",
            "--env-file",
            "/tmp/x.env",
            "work-items",
            "comments",
            "list",
            "ENG-335",
        ]);
        assert_eq!(command_path_from_argv(&a), "work-items comments list");
        assert_eq!(command_path_from_argv(&argv(&["nonexistent"])), "");
    }

    #[test]
    fn network_fingerprint_is_constant_and_has_no_host() {
        let err = CoreError::Unreachable {
            message: "x".into(),
            api_host: "192.0.2.10:8087".into(),
            config_layer: "global".into(),
            config_path: None,
            reason: "No route to host (os error 113)".into(),
        };
        assert_eq!(
            core_error_fingerprint(&err),
            Some(vec!["network_error".into()])
        );
        let other_reason = CoreError::Unreachable {
            message: "y".into(),
            api_host: "192.0.2.10:8087".into(),
            config_layer: "local".into(),
            config_path: None,
            reason: "Connection refused (os error 111)".into(),
        };
        assert_eq!(
            core_error_fingerprint(&other_reason),
            Some(vec!["network_error".into()])
        );
        assert_eq!(core_error_fingerprint(&CoreError::Usage("x".into())), None);
    }

    #[test]
    fn clap_fingerprint_uses_command_path_and_flag() {
        let a = argv(&[
            "work-items",
            "comments",
            "delete",
            "--work-item",
            "ENG-1",
            "--bogus-flag",
            "v1",
        ]);
        let err = Cli::try_parse_from(&a).err().unwrap();
        let fp = clap_error_fingerprint(&err, &command_path_from_argv(&a));
        assert_eq!(
            fp,
            vec![
                "clap_parse".to_string(),
                "work-items comments delete".into(),
                "--bogus-flag".into()
            ]
        );
        let b = argv(&[
            "work-items",
            "comments",
            "delete",
            "--work-item",
            "OTHER-9",
            "--bogus-flag",
            "v2",
        ]);
        let err_b = Cli::try_parse_from(&b).err().unwrap();
        assert_eq!(
            fp,
            clap_error_fingerprint(&err_b, &command_path_from_argv(&b))
        );
    }

    #[test]
    fn clap_fingerprint_for_unknown_subcommand() {
        let a = argv(&["frobnicate", "x"]);
        let err = Cli::try_parse_from(&a).err().unwrap();
        let fp = clap_error_fingerprint(&err, &command_path_from_argv(&a));
        assert_eq!(fp[1], "plane-cli");
        assert_eq!(fp[2], "<subcommand>");
    }

    /// Restores env vars on drop.
    struct EnvGuard(Vec<(&'static str, Option<String>)>);
    impl EnvGuard {
        fn set(vars: &[(&'static str, &str)]) -> Self {
            let saved = vars
                .iter()
                .map(|(k, v)| {
                    let old = std::env::var(k).ok();
                    std::env::set_var(k, v);
                    (*k, old)
                })
                .collect();
            EnvGuard(saved)
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, old) in &self.0 {
                match old {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    #[test]
    fn captured_events_never_contain_the_api_key() {
        let _lock = crate::plane_env_files::lock_env();
        let secret = "plane-test-secret-api-key-7f3a9c";
        let _env = EnvGuard::set(&[
            ("PLANE_API_KEY", secret),
            ("PLANE_API_BASE_URL", "http://127.0.0.1:9"),
            ("PLANE_WORKSPACE_SLUG", "ws"),
        ]);
        let config = plane_core::Config::from_env(true).unwrap();
        let client = plane_core::HttpClient::new(&config).unwrap();
        // Port 9 (discard) on loopback: refused immediately, no external network.
        let raw = client.get("/api/v1/users/me/", &[]).unwrap_err();
        assert_eq!(raw.error_code(), "network_error");
        // Built directly (not via enrich_network_error) so no real config file is read.
        let config_path = "/home/x/.config/plane-cli/.plane-cli.env";
        let enriched = CoreError::Unreachable {
            message: format!(
                "cannot reach Plane API at 127.0.0.1:9 (PLANE_API_BASE_URL from global file {config_path}): Connection refused"
            ),
            api_host: "127.0.0.1:9".into(),
            config_layer: "global".into(),
            config_path: Some(config_path.into()),
            reason: "Connection refused (os error 111)".into(),
        };
        let body_path = "/home/x/secret-dir/payload.json";
        let usage = CoreError::usage_code(
            "body_file_not_found",
            format!("`--body` file not found: {body_path}"),
        );
        let http = CoreError::Http {
            status: 403,
            method: "GET".into(),
            path: "/api/v1/users/me/".into(),
            message: "forbidden".into(),
            body: Some(serde_json::json!({ "echo": secret })),
            rate_limit: None,
        };

        let partial = CoreError::Http {
            status: 404,
            method: "DELETE".into(),
            path: "/api/v1/workspaces/ws/projects/211751d8-8fc6-4df8-9b1d-757550e5180b/cycles/x/"
                .into(),
            message: "gone".into(),
            body: Some(serde_json::json!({ "echo": secret })),
            rate_limit: None,
        }
        .with_details(
            "failed on WEB-8 after removing WEB-7 /home/x/file",
            serde_json::json!({ "removed": ["WEB-7"], "failed": "WEB-8" }),
        );
        let unexpected =
            CoreError::UnexpectedResponse("attachment create response has no asset_id".into());

        let events = sentry::test::with_captured_events(|| {
            capture_core_error_inner(&enriched, Some("auth whoami"), true);
            capture_core_error_inner(&raw, Some("auth whoami"), true);
            capture_core_error_inner(&http, Some("auth whoami"), true);
            capture_core_error_inner(&usage, Some("work-items create"), true);
            capture_core_error_inner(&partial, Some("cycles items remove"), true);
            capture_core_error_inner(&unexpected, Some("work-items attachments create"), true);
        });

        assert_eq!(events.len(), 6);
        let partial_event = serde_json::to_string(&events[4]).unwrap();
        assert!(
            !partial_event.contains("WEB-8"),
            "work item id leaked: {partial_event}"
        );
        assert!(
            !partial_event.contains("211751d8-8fc6-4df8-9b1d-757550e5180b"),
            "uuid leaked: {partial_event}"
        );
        for ev in &events {
            let s = serde_json::to_string(ev).unwrap();
            assert!(!s.contains(secret), "API key leaked into event: {s}");
            assert!(!s.contains(config_path), "config path leaked: {s}");
            assert!(!s.contains("/home/x"), "home path leaked: {s}");
        }
        let fp = |i: usize| -> Vec<String> {
            events[i]
                .fingerprint
                .iter()
                .map(|c| c.to_string())
                .collect()
        };
        assert_eq!(fp(0), vec!["network_error"]);
        assert_eq!(events[0].level, Level::Warning);
        assert_eq!(fp(1), vec!["network_error"]);
        assert_eq!(fp(3), vec!["usage", "body_file_not_found"]);
        assert_eq!(fp(4), vec!["partial_failure", "not_found"]);
        assert_eq!(
            fp(5),
            vec![
                "unexpected_response",
                "attachment create response has no asset_id"
            ]
        );
    }

    /// An `--as` call whose identity check or request fails must not put the member key
    /// in any Sentry event, whatever the server echoes back.
    #[test]
    fn captured_events_during_an_as_call_contain_no_member_key() {
        use crate::identity::{self, tests as id};
        let _lock = crate::plane_env_files::lock_env();
        let key = "plane_api_member_secret_9d41c2e7";
        let mut server = mockito::Server::new();
        let _env = EnvGuard::set(&[("PLANE_API_BASE_URL", server.url().as_str())]);

        // Key from the injected process env, as a process supervisor would provide it.
        let lookup = id::lookup_with(&[("PLANE_API_KEY_BOB", key)], None, None);
        let mk = identity::load_member_key("bob", &lookup).unwrap();

        // 1. users/me answers as someone else: mismatch.
        let mismatch_mock = server
            .mock("GET", "/api/v1/users/me/")
            .match_header("x-api-key", key)
            .with_status(200)
            .with_body(format!(
                r#"{{"id":"u1","display_name":"alice","echo":"{key}"}}"#
            ))
            .create();
        let m = id::MockApi::new(&server.url(), "bob", key);
        let mismatch = identity::verify_identity(&m.api, &mk, true).unwrap_err();
        assert_eq!(mismatch.error_code(), "member_identity_mismatch");
        mismatch_mock.remove();

        // 2. The key is rejected, and the server echoes it in the body.
        let _rejected_mock = server
            .mock("GET", "/api/v1/users/me/")
            .with_status(401)
            .with_body(format!(r#"{{"detail":"bad token {key}"}}"#))
            .create();
        let rejected = identity::verify_identity(&m.api, &mk, true).unwrap_err();
        assert_eq!(rejected.error_code(), "member_key_rejected");

        // 3. A later request fails with an HTTP error whose body echoes the key.
        let raw_http = m.api.whoami().unwrap_err();
        assert_eq!(raw_http.error_code(), "authentication_failed");

        // 4. An insecure key file.
        let insecure = {
            let dir = tempfile::tempdir().unwrap();
            let p = dir.path().join("bob");
            std::fs::write(&p, key).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
                identity::read_key_file(&p).unwrap_err()
            }
            #[cfg(not(unix))]
            {
                CoreError::usage_code("member_key_file_insecure", "n/a")
            }
        };

        // 5. Errors whose own message carries the key: a plain usage/config message (as an
        //    argv echo would), and a non-`plane_api_` key that main registers as sent.
        let odd_key = "sk-bob-odd-shape-55aa";
        crate::redact::register_secret(odd_key);
        let usage_echo = CoreError::Usage(format!("unexpected value '{key}' for --member"));
        let config_echo = CoreError::Config(format!("bad key {odd_key} in env"));
        let io_echo = CoreError::Io(std::io::Error::other(format!("open {key}: denied")));

        // 6. Clap errors: the key as a stray positional and as an unknown subcommand.
        let clap_pos = crate::dispatch::Cli::try_parse_from([
            "plane-cli",
            "config",
            "set-key",
            "--member",
            "bob",
            key,
        ])
        .err()
        .unwrap();
        let clap_sub = crate::dispatch::Cli::try_parse_from(["plane-cli", key])
            .err()
            .unwrap();
        assert!(clap_pos.to_string().contains(key) && clap_sub.to_string().contains(key));

        let events = sentry::test::with_captured_events(|| {
            capture_core_error_inner(&mismatch, Some("auth whoami"), true);
            capture_core_error_inner(&rejected, Some("auth whoami"), true);
            capture_core_error_inner(&raw_http, Some("work-items comments create"), true);
            capture_core_error_inner(&insecure, Some("auth whoami"), true);
            capture_core_error_inner(&usage_echo, Some("config set-key"), true);
            capture_core_error_inner(&config_echo, Some("auth whoami"), true);
            capture_core_error_inner(&io_echo, Some("auth whoami"), true);
            capture_clap_error_inner(&clap_pos, "config set-key", true);
            capture_clap_error_inner(&clap_sub, "", true);
        });
        assert_eq!(events.len(), 9);
        for ev in &events {
            let s = serde_json::to_string(ev).unwrap();
            assert!(!s.contains(key), "member key leaked into event: {s}");
            assert!(!s.contains("member_secret"), "key fragment leaked: {s}");
            assert!(!s.contains(odd_key), "registered key leaked: {s}");
        }
        let clap_msg = events[7].message.clone().unwrap_or_default();
        assert!(clap_msg.contains("<positional>"), "{clap_msg}");
        let fp = |i: usize| -> Vec<String> {
            events[i]
                .fingerprint
                .iter()
                .map(|c| c.to_string())
                .collect()
        };
        assert_eq!(fp(0), vec!["usage", "member_identity_mismatch"]);
        assert_eq!(fp(1), vec!["usage", "member_key_rejected"]);
    }

    #[test]
    fn clap_fingerprint_hides_stray_positional_values() {
        let fp_for = |extra: &str| {
            let a = argv(&[
                "work-items",
                "update",
                "--work-item",
                "ENG-1",
                "ENG-2",
                extra,
            ]);
            let err = Cli::try_parse_from(&a)
                .err()
                .expect("stray positional fails");
            clap_error_fingerprint(&err, &command_path_from_argv(&a))
        };
        let a = fp_for("extra");
        let b = fp_for("other");
        assert_eq!(a, b);
        assert_eq!(a[2], "<positional>");
        assert!(!a.iter().any(|s| s.contains("extra") || s.contains("ENG-")));
    }

    #[test]
    fn transaction_name_falls_back_to_binary_name() {
        assert_eq!(transaction_name(""), "plane-cli");
        assert_eq!(transaction_name("projects list"), "projects list");
    }

    #[test]
    fn policy_refusals_send_no_sentry_event() {
        let violation = CoreError::usage_code(
            "policy_violation",
            "policy for WEB refused work-items create: description_shape: …",
        )
        .with_details(
            "",
            serde_json::json!({"rule": "description_shape", "policy_file": "/home/x/p.toml"}),
        );
        let invalid = CoreError::usage_code("policy_invalid", "policy file /home/x/p.toml: bad");
        let unknown = CoreError::usage_code("policy_unknown_state", "state \"Nope\" …");
        let other = CoreError::usage_code("body_file_not_found", "no such file");
        assert!(is_expected_refusal(&violation));
        assert_eq!(violation.exit_kind().code(), 2);
        let events = sentry::test::with_captured_events(|| {
            capture_core_error_inner(&violation, Some("work-items create"), true);
            capture_core_error_inner(&invalid, Some("work-items update"), true);
            capture_core_error_inner(&unknown, Some("work-items update"), true);
            capture_core_error_inner(&other, Some("work-items create"), true);
        });
        assert_eq!(events.len(), 1, "only the non-policy error is captured");
        let s = serde_json::to_string(&events[0]).unwrap();
        assert!(s.contains("body_file_not_found") && !s.contains("partial failure"));
    }
}
