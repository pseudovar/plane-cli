use crate::dispatch::{Cli, ConfigCmd};
use crate::identity;
use crate::net_diag::{self, Reachability};
use crate::plane_env_files::{
    base_url_source, global_plane_cli_env_path, keys_in_env_file, load_env_layers,
    local_plane_cli_env_path, read_env_file, value_source,
};
use plane_core::{print_json, CoreError, ResponseEnvelope};
use serde_json::json;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn default_env_file() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".plane-cli.env")
}

fn prompt_line(prompt: &str, default: Option<&str>) -> Result<String, CoreError> {
    if let Some(d) = default {
        eprint!("{prompt} [{d}]: ");
    } else {
        eprint!("{prompt}: ");
    }
    io::stderr().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let s = buf.trim().to_string();
    if s.is_empty() {
        Ok(default.unwrap_or("").to_string())
    } else {
        Ok(s)
    }
}

fn prompt_yes_no(prompt: &str, default_no: bool) -> Result<bool, CoreError> {
    let hint = if default_no { "y/N" } else { "Y/n" };
    eprint!("{prompt} [{hint}]: ");
    io::stderr().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let s = buf.trim().to_lowercase();
    if s.is_empty() {
        return Ok(!default_no);
    }
    Ok(matches!(s.as_str(), "y" | "yes"))
}

/// Ask about telemetry, defaulting to `current`. A build with no DSN cannot send anything, so
/// it says so and keeps the current setting without asking.
fn prompt_telemetry(current: bool, path: &Path) -> Result<bool, CoreError> {
    if crate::plane_env_files::is_in_cwd(path) {
        eprintln!(
            "Error telemetry: not read from a file in the current directory. To opt in, use `config update --global`."
        );
        return Ok(false);
    }
    if !crate::telemetry::build_has_dsn() {
        eprintln!(
            "Error telemetry: not available in this build (no telemetry endpoint compiled in)."
        );
        return Ok(current);
    }
    eprintln!("Optional error telemetry (Sentry) sends the command name, error codes, version, OS details and panic stack traces. It never sends API keys, hostnames or workspace data. Details: https://pseudovar.github.io/plane-cli/telemetry.html");
    prompt_yes_no("Enable PLANE_CLI_ANON_TELEMETRY", !current)
}

/// Empty input keeps the current secret; a new value replaces it.
/// Input is hidden on a terminal (rpassword); the current value is never shown.
fn prompt_secret_or_keep(prompt: &str, has_current: bool) -> Result<Option<String>, CoreError> {
    use std::io::IsTerminal;
    let label = if has_current {
        format!("{prompt} (input hidden) [keep current]: ")
    } else {
        format!("{prompt} (input hidden): ")
    };
    let raw = if io::stdin().is_terminal() {
        rpassword::prompt_password(label)?
    } else {
        eprint!("{label}");
        io::stderr().flush()?;
        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        buf
    };
    let s = raw.trim();
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s.to_string()))
    }
}

fn env_truthy_value(s: &str) -> bool {
    let v = s.trim().to_lowercase();
    v == "1" || v == "true" || v == "yes"
}

fn resolve_config_path(
    env_file: Option<PathBuf>,
    global: bool,
    subcommand: &str,
) -> Result<PathBuf, CoreError> {
    if global && env_file.is_some() {
        return Err(CoreError::Usage(format!(
            "`config {subcommand}` cannot combine `--global` with `--env-file PATH` (explicit path)"
        )));
    }
    if global {
        let p = global_plane_cli_env_path().ok_or_else(|| {
            CoreError::Usage(
                "cannot resolve global config directory (set HOME on Unix or APPDATA on Windows)"
                    .into(),
            )
        })?;
        Ok(p)
    } else {
        Ok(env_file.unwrap_or_else(default_env_file))
    }
}

fn write_env_file(
    path: &Path,
    base_url: &str,
    api_key: &str,
    workspace_slug: &str,
    anon_telemetry: bool,
) -> Result<(), CoreError> {
    // Writing through a link placed in the current directory would overwrite its target.
    if crate::plane_env_files::is_in_cwd(path)
        && fs::symlink_metadata(path).is_ok_and(|m| !m.is_file())
    {
        return Err(CoreError::usage_code(
            "env_file_unsafe",
            format!(
                "refusing to write {}: it is not a regular file",
                path.display()
            ),
        ));
    }
    let mut body = format!(
        "PLANE_API_BASE_URL={base_url}\nPLANE_API_KEY={api_key}\nPLANE_WORKSPACE_SLUG={workspace_slug}\n"
    );
    if anon_telemetry {
        body.push_str("PLANE_CLI_ANON_TELEMETRY=1\n");
    }
    fs::write(path, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

pub fn run_init_interactive(
    env_file: Option<PathBuf>,
    global: bool,
    overwrite: bool,
) -> Result<PathBuf, CoreError> {
    let path = resolve_config_path(env_file, global, "init")?;
    if global {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
    }
    if path.exists() && !overwrite {
        return Err(CoreError::Usage(format!(
            "{} already exists.\n\
             - To edit in place: `{} config update{}`\n\
             - To replace all values: `{} config init --overwrite{}`\n\
             - To use existing file: run `{} auth whoami` (loads user-global then `./.plane-cli.env` automatically), or pass `{} --env-file \"{}\" …`",
            path.display(),
            env!("CARGO_PKG_NAME"),
            if global { " --global" } else { "" },
            env!("CARGO_PKG_NAME"),
            if global { " --global" } else { "" },
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_NAME"),
            path.display()
        )));
    }

    eprintln!("plane-cli config init");
    eprintln!("This writes PLANE_* variables to an env file.");
    let base_default = std::env::var("PLANE_API_BASE_URL").ok();
    let key_default = std::env::var("PLANE_API_KEY").ok();
    let workspace_default = std::env::var("PLANE_WORKSPACE_SLUG").ok();

    let base_url = prompt_line("PLANE_API_BASE_URL", base_default.as_deref())?;
    if base_url.trim().is_empty() {
        return Err(CoreError::Usage(
            "PLANE_API_BASE_URL cannot be empty".into(),
        ));
    }
    eprintln!(
        "PLANE_API_KEY is written to this plain-text file. To keep it in the Keychain or a mode-600 key file instead, use `plane-cli config set-key --member <you>` and set PLANE_CLI_AS (https://pseudovar.github.io/plane-cli/agent-identity.html)."
    );
    let key_default = key_default.filter(|k| !k.trim().is_empty());
    let api_key = match prompt_secret_or_keep("PLANE_API_KEY", key_default.is_some())? {
        Some(v) => v,
        None => key_default.unwrap_or_default(),
    };
    if api_key.trim().is_empty() {
        return Err(CoreError::Usage("PLANE_API_KEY cannot be empty".into()));
    }
    let workspace_slug = prompt_line("PLANE_WORKSPACE_SLUG", workspace_default.as_deref())?;
    if workspace_slug.trim().is_empty() {
        return Err(CoreError::Usage(
            "PLANE_WORKSPACE_SLUG cannot be empty".into(),
        ));
    }

    let anon_telemetry = prompt_telemetry(false, &path)?;

    write_env_file(
        &path,
        base_url.trim(),
        api_key.trim(),
        workspace_slug.trim(),
        anon_telemetry,
    )?;
    eprintln!("Wrote {}", path.display());
    if anon_telemetry && crate::telemetry::build_has_dsn() {
        eprintln!("Error telemetry: enabled (PLANE_CLI_ANON_TELEMETRY=1)");
    }
    eprintln!("Next step:");
    eprintln!("  plane-cli auth whoami");
    eprintln!(
        "(or explicit: plane-cli --env-file \"{}\" auth whoami)",
        path.display()
    );
    Ok(path)
}

pub fn run_update_interactive(
    env_file: Option<PathBuf>,
    global: bool,
) -> Result<PathBuf, CoreError> {
    let path = resolve_config_path(env_file, global, "update")?;
    if !path.exists() {
        return Err(CoreError::Usage(format!(
            "{} does not exist.\n\
             - Create it: `{} config init{}`\n\
             - See paths: `{} config where`",
            path.display(),
            env!("CARGO_PKG_NAME"),
            if global { " --global" } else { "" },
            env!("CARGO_PKG_NAME"),
        )));
    }

    let current = read_env_file(&path).map_err(CoreError::Usage)?;

    eprintln!("plane-cli config update");
    eprintln!(
        "Editing {} — press Enter to keep each current value.",
        path.display()
    );
    eprintln!();

    let base_default = current.get("PLANE_API_BASE_URL").map(String::as_str);
    let base_url = prompt_line("PLANE_API_BASE_URL", base_default)?;
    if base_url.trim().is_empty() {
        return Err(CoreError::Usage(
            "PLANE_API_BASE_URL cannot be empty".into(),
        ));
    }

    eprintln!(
        "API key: press Enter to keep the current value (not shown). To move it out of this file, use `plane-cli config set-key --member <you>` (https://pseudovar.github.io/plane-cli/agent-identity.html)."
    );
    let has_key = current
        .get("PLANE_API_KEY")
        .is_some_and(|v| !v.trim().is_empty());
    let api_key = match prompt_secret_or_keep("PLANE_API_KEY", has_key)? {
        Some(v) if v.trim().is_empty() => {
            return Err(CoreError::Usage("PLANE_API_KEY cannot be empty".into()));
        }
        Some(v) => v,
        None => current.get("PLANE_API_KEY").cloned().ok_or_else(|| {
            CoreError::Usage("PLANE_API_KEY is not set in this file; enter a value".into())
        })?,
    };

    let workspace_default = current.get("PLANE_WORKSPACE_SLUG").map(String::as_str);
    let workspace_slug = prompt_line("PLANE_WORKSPACE_SLUG", workspace_default)?;
    if workspace_slug.trim().is_empty() {
        return Err(CoreError::Usage(
            "PLANE_WORKSPACE_SLUG cannot be empty".into(),
        ));
    }

    let anon_current = current
        .get("PLANE_CLI_ANON_TELEMETRY")
        .is_some_and(|v| env_truthy_value(v));
    let anon_telemetry = prompt_telemetry(anon_current, &path)?;

    write_env_file(
        &path,
        base_url.trim(),
        api_key.trim(),
        workspace_slug.trim(),
        anon_telemetry,
    )?;
    eprintln!("Updated {}", path.display());
    if !crate::telemetry::build_has_dsn() {
        eprintln!("Error telemetry: not available in this build");
    } else if anon_telemetry {
        eprintln!("Error telemetry: enabled (PLANE_CLI_ANON_TELEMETRY=1)");
    } else {
        eprintln!("Error telemetry: disabled");
    }
    eprintln!("Next: plane-cli auth whoami");
    Ok(path)
}

fn error_json(e: &CoreError) -> serde_json::Value {
    json!({"code": e.error_code(), "message": e.to_string()})
}

/// `--as` / `PLANE_CLI_AS` part of `config doctor`: which member, where its key comes from
/// (never the key), and the key itself for the reachability probe.
fn doctor_identity(cli: &Cli) -> (serde_json::Value, Option<identity::MemberKey>) {
    match identity::resolve_acting(
        cli.as_member.as_deref(),
        cli.env_file.as_deref(),
        cli.dotenv,
    ) {
        Ok(None) => (serde_json::Value::Null, None),
        Ok(Some(a)) => match identity::refuse_cwd_url_for_member(
            crate::plane_env_files::base_url_from_cwd(),
            &a.member,
        )
        .and_then(|()| identity::load_member_key(&a.member, &identity::system_lookup()))
        {
            Ok(mk) => (
                json!({
                    "member": a.member,
                    "origin": a.origin.as_str(),
                    "key_found": true,
                    "key_source": mk.source.to_json(),
                }),
                Some(mk),
            ),
            Err(e) => (
                json!({
                    "member": a.member,
                    "origin": a.origin.as_str(),
                    "key_found": false,
                    "error": error_json(&e),
                }),
                None,
            ),
        },
        Err(e) => (json!({"key_found": false, "error": error_json(&e)}), None),
    }
}

/// Warn when `PLANE_API_KEY` is read from a plain-text env file.
fn plain_text_key_warning(cli: &Cli) -> (serde_json::Value, Option<String>) {
    let src = value_source("PLANE_API_KEY", cli.env_file.as_deref(), cli.dotenv);
    let from_file = !matches!(src.layer, "process_env" | "unset" | "unknown");
    let warning = from_file.then(|| {
        format!(
            "PLANE_API_KEY is read from a plain-text env file ({}). Move it to a secret store: `plane-cli config set-key --member <you>`, set PLANE_CLI_AS=<you>, then delete PLANE_API_KEY from that file (see https://pseudovar.github.io/plane-cli/agent-identity.html)",
            src.describe()
        )
    });
    (src.to_json(), warning)
}

fn doctor_payload(cli: &Cli, offline: bool) -> serde_json::Value {
    let base = std::env::var("PLANE_API_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let key = std::env::var("PLANE_API_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let ws = std::env::var("PLANE_WORKSPACE_SLUG")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let anon_telemetry = crate::telemetry::enabled();

    let (mut identity_json, member_key) = doctor_identity(cli);
    let acting = !identity_json.is_null();
    let (api_key_source, key_warning) = plain_text_key_warning(cli);
    if let Some(w) = key_warning {
        plane_core::push_warning(w);
    }

    // Acting as a member, its key replaces PLANE_API_KEY.
    let key_ok = if acting {
        member_key.is_some()
    } else {
        key.is_some()
    };
    let vars_ok = base.is_some() && key_ok && ws.is_some();
    let config = || match &member_key {
        Some(mk) => {
            plane_core::Config::from_env_with_key(false, mk.key.clone(), Some(mk.member.clone()))
        }
        None => plane_core::Config::from_env(false),
    };
    let reachability = if offline {
        Reachability::Skipped {
            reason: "--offline".into(),
        }
    } else if !vars_ok {
        Reachability::Skipped {
            reason: "missing required variables".into(),
        }
    } else {
        match config() {
            Ok(config) => net_diag::probe(&config),
            Err(e) => Reachability::Unreachable {
                reason: e.to_string(),
            },
        }
    };
    // The key answers; check it belongs to the member asked for.
    let mut identity_ok = !acting || member_key.is_some();
    if let (Some(mk), Reachability::Reachable) = (&member_key, &reachability) {
        let verified = config()
            .and_then(plane_core::PlaneApi::new)
            .and_then(|api| identity::verify_identity(&api, mk, true));
        match verified {
            Ok(summary) => {
                identity_json["verified"] = json!(true);
                identity_json["user_id"] = summary["user_id"].clone();
            }
            Err(e) => {
                identity_ok = false;
                identity_json["verified"] = json!(false);
                identity_json["error"] = error_json(&e);
            }
        }
    }
    let source = base_url_source(cli.env_file.as_deref(), cli.dotenv);
    let ok = vars_ok
        && identity_ok
        && matches!(
            reachability,
            Reachability::Reachable | Reachability::Skipped { .. }
        );
    let next_step = match (&reachability, vars_ok) {
        (_, false) if acting && base.is_some() && ws.is_some() => {
            "plane-cli config set-key --member <member>"
        }
        (_, true) if !identity_ok => "plane-cli config set-key --member <member>",
        (_, false) => "plane-cli config init",
        (Reachability::Unreachable { .. } | Reachability::NotPlane { .. }, _) => {
            "plane-cli config where (find the file that sets PLANE_API_BASE_URL), then plane-cli config update"
        }
        (Reachability::Unauthorized { .. }, _) => {
            "regenerate the API key in Plane, then plane-cli config update"
        }
        _ => "plane-cli auth whoami",
    };
    json!({
        "ok": ok,
        "reachability": reachability.to_json(&net_diag::configured_host(), &source),
        "vars": {
            "PLANE_API_BASE_URL": base.is_some(),
            "PLANE_API_KEY": key.is_some(),
            "PLANE_WORKSPACE_SLUG": ws.is_some(),
            "PLANE_CLI_ANON_TELEMETRY": anon_telemetry
        },
        "vars_ok": vars_ok,
        "api_key_source": api_key_source,
        "identity": identity_json,
        "anon_telemetry": {
            "enabled": anon_telemetry,
            "build_has_endpoint": crate::telemetry::build_has_dsn(),
            "active": anon_telemetry && crate::telemetry::build_has_dsn(),
            "opt_in_var": "PLANE_CLI_ANON_TELEMETRY"
        },
        "next_step": next_step,
    })
}

fn env_file_layer(kind: &str, path: PathBuf) -> Result<serde_json::Value, CoreError> {
    let exists = path.exists();
    let keys = keys_in_env_file(&path).map_err(CoreError::Usage)?;
    Ok(json!({
        "kind": kind,
        "path": path.display().to_string(),
        "exists": exists,
        "keys": keys,
    }))
}

fn where_payload(cli: &Cli) -> Result<serde_json::Value, CoreError> {
    let cwd = std::env::current_dir()
        .map_err(|e| CoreError::Usage(format!("could not read current directory: {e}")))?;
    let dotenv_cwd = cwd.join(".env");

    let mut layers = Vec::new();
    if let Some(p) = &cli.env_file {
        layers.push(env_file_layer("cli_env_file", p.clone())?);
    }
    if let Some(l) = crate::plane_env_files::plane_dotenv_layer() {
        layers.push(env_file_layer("plane_dotenv", l.path)?);
    }
    if cli.dotenv {
        layers.push(env_file_layer("dotenv_cwd", dotenv_cwd.clone())?);
    }
    let local = local_plane_cli_env_path().map_err(CoreError::Usage)?;
    // In the config directory, `./.plane-cli.env` is the global file: list it once, as global.
    if !crate::plane_env_files::is_global_path(&local) {
        layers.push(env_file_layer("local", local)?);
    }
    if let Some(p) = global_plane_cli_env_path() {
        layers.push(env_file_layer("global", p)?);
    }

    Ok(json!({
        "cwd": cwd.display().to_string(),
        "load_order_note": "process env (non-empty) → --env-file → PLANE_DOTENV → --dotenv → ./.plane-cli.env → global .plane-cli.env (the first layer that sets a key wins; files in the current directory are restricted, see the configuration docs)",
        "layers": layers,
        "base_url_source": base_url_source(cli.env_file.as_deref(), cli.dotenv).to_json(),
        "invocation": {
            "env_file": cli.env_file.as_ref().map(|p| p.display().to_string()),
            "dotenv": cli.dotenv,
            "plane_dotenv": std::env::var("PLANE_DOTENV").ok().filter(|v| !v.trim().is_empty()),
        }
    }))
}

fn print_where_human(payload: &serde_json::Value) -> Result<(), CoreError> {
    let cwd = payload["cwd"].as_str().unwrap_or("?");
    eprintln!("Working directory: {cwd}");
    eprintln!();
    eprintln!(
        "Config files (load order: https://pseudovar.github.io/plane-cli/agent-contract.html):"
    );
    if let Some(layers) = payload["layers"].as_array() {
        for layer in layers {
            let kind = layer["kind"].as_str().unwrap_or("?");
            let path = layer["path"].as_str().unwrap_or("?");
            let exists = layer["exists"].as_bool().unwrap_or(false);
            let status = if exists { "exists" } else { "missing" };
            eprint!("  {kind}: {path} ({status})");
            if exists {
                if let Some(keys) = layer["keys"].as_array() {
                    if keys.is_empty() {
                        eprintln!();
                    } else {
                        let names: Vec<_> = keys.iter().filter_map(|k| k.as_str()).collect();
                        eprintln!("; keys: {}", names.join(", "));
                    }
                } else {
                    eprintln!();
                }
            } else {
                eprintln!();
            }
        }
    }
    eprintln!();
    let inv = &payload["invocation"];
    let env_file = inv["env_file"].as_str().unwrap_or("(none)");
    let dotenv = inv["dotenv"].as_bool().unwrap_or(false);
    let plane_dotenv = inv["plane_dotenv"].as_str().unwrap_or("(unset)");
    eprintln!("This invocation:");
    eprintln!("  --env-file: {env_file}");
    eprintln!("  --dotenv: {dotenv}");
    eprintln!("  PLANE_DOTENV: {plane_dotenv}");
    Ok(())
}

/// The key from a hidden TTY prompt, or all of stdin when it is not a terminal. Never echoed.
fn read_secret_input(member: &str) -> Result<String, CoreError> {
    use std::io::{IsTerminal, Read};
    let raw = if io::stdin().is_terminal() {
        rpassword::prompt_password(format!("API key for `{member}` (input hidden): "))?
    } else {
        let mut s = String::new();
        io::stdin().read_to_string(&mut s)?;
        s
    };
    Ok(raw.trim().to_string())
}

/// Keychain on macOS, else the mode-600 key file.
fn store_member_key(member: &str, key: &str) -> Result<identity::KeySource, CoreError> {
    if cfg!(target_os = "macos") {
        let service = identity::keychain_store(member, key)?;
        return Ok(identity::KeySource::Keychain { service });
    }
    let dir = identity::keys_dir().ok_or_else(|| {
        CoreError::usage_code(
            "member_key_store_unsupported",
            "cannot resolve the plane-cli config directory (set HOME or XDG_CONFIG_HOME)",
        )
    })?;
    let path = identity::write_key_file(&dir, member, key)?;
    Ok(identity::KeySource::KeyFile { path })
}

fn run_set_key(cli: &Cli, member: &str) -> Result<(), CoreError> {
    let member = identity::validate_member(member, "--member")?;
    let key = read_secret_input(&member)?;
    identity::validate_key(&key)?;
    let stored = store_member_key(&member, &key)?;
    let pin_reset = reset_identity_pin(cli, &member);
    let next = format!("plane-cli --as {member} auth whoami");
    if cli.json_mode() {
        print_json(&ResponseEnvelope::success(json!({
            "member": member,
            "stored_in": stored.to_json(),
            "identity_pin_reset": pin_reset,
            "next_step": next,
        })))?;
    } else {
        eprintln!(
            "Stored the API key for `{member}` in {}.",
            stored.describe()
        );
        if pin_reset {
            eprintln!(
                "Reset the remembered Plane user for `{member}`; the next call pins it again."
            );
        }
        eprintln!("Next: {next}");
    }
    Ok(())
}

/// Storing a key is the explicit act that accepts whichever Plane user it belongs to: drop the
/// member's cached check and user-id pin for the configured instance and workspace. `false`
/// when the base URL or workspace is not configured (nothing to reset).
fn reset_identity_pin(cli: &Cli, member: &str) -> bool {
    if load_env_layers(cli.env_file.as_deref(), cli.dotenv).is_err() {
        return false;
    }
    // The key value is irrelevant here; only the cache location is derived.
    let Ok(config) =
        plane_core::Config::from_env_with_key(false, "unused".into(), Some(member.into()))
    else {
        return false;
    };
    match plane_core::Cache::new(
        config.api_base.as_str(),
        &config.workspace_slug,
        Some(member),
        config.cache_enabled,
    ) {
        Ok(cache) => {
            identity::reset_identity(&cache, member);
            config.cache_enabled
        }
        Err(_) => false,
    }
}

fn load_doctor_env(cli: &Cli) -> Result<(), CoreError> {
    load_env_layers(cli.env_file.as_deref(), cli.dotenv).map_err(CoreError::from)
}

pub fn run(cli: &Cli, cmd: &ConfigCmd) -> Result<(), CoreError> {
    match cmd {
        ConfigCmd::Init {
            env_file,
            global,
            overwrite,
        } => {
            if cli.json_mode() {
                return Err(CoreError::Usage(
                    "machine-readable JSON mode cannot be combined with `config init` (interactive wizard); unset PLANE_CLI_JSON or omit `--json`"
                        .into(),
                ));
            }
            let _ = run_init_interactive(env_file.clone(), *global, *overwrite)?;
            Ok(())
        }
        ConfigCmd::Update { env_file, global } => {
            if cli.json_mode() {
                return Err(CoreError::Usage(
                    "machine-readable JSON mode cannot be combined with `config update` (interactive wizard); unset PLANE_CLI_JSON or omit `--json`"
                        .into(),
                ));
            }
            let _ = run_update_interactive(env_file.clone(), *global)?;
            Ok(())
        }
        ConfigCmd::SetKey { member } => run_set_key(cli, member),
        ConfigCmd::Policy { cmd } => crate::policy_cmd::run(cli, cmd),
        ConfigCmd::Where => {
            let payload = where_payload(cli)?;
            if cli.json_mode() {
                print_json(&ResponseEnvelope::success(payload))?;
            } else {
                print_where_human(&payload)?;
            }
            Ok(())
        }
        ConfigCmd::Doctor { offline } => {
            load_doctor_env(cli)?;
            let payload = doctor_payload(cli, *offline);
            if cli.json_mode() {
                print_json(&ResponseEnvelope::success(payload))?;
            } else {
                let ok = payload.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                let reach = &payload["reachability"];
                let status = reach["status"].as_str().unwrap_or("?");
                let host = reach["api_host"].as_str().unwrap_or("?");
                let layer = reach["base_url_source"]["layer"].as_str().unwrap_or("?");
                let mut line =
                    format!("API reachability: {status} ({host}; PLANE_API_BASE_URL from {layer}");
                if let Some(p) = reach["base_url_source"]["path"].as_str() {
                    line.push_str(&format!(" {p}"));
                }
                line.push(')');
                if let Some(r) = reach["reason"].as_str() {
                    line.push_str(&format!(": {r}"));
                }
                if let Some(s) = reach["http_status"].as_u64() {
                    line.push_str(&format!(": HTTP {s}"));
                }
                eprintln!("{line}");
                for w in plane_core::take_warnings() {
                    eprintln!("warning: {w}");
                }
                let ident = &payload["identity"];
                if !ident.is_null() {
                    let member = ident["member"].as_str().unwrap_or("?");
                    match (ident["key_found"].as_bool(), ident["verified"].as_bool()) {
                        (Some(true), Some(true)) => eprintln!(
                            "Acting as `{member}`: key from {}, verified with users/me",
                            ident["key_source"]["kind"].as_str().unwrap_or("?")
                        ),
                        (Some(true), None) => eprintln!(
                            "Acting as `{member}`: key from {} (not verified)",
                            ident["key_source"]["kind"].as_str().unwrap_or("?")
                        ),
                        _ => eprintln!(
                            "Acting as `{member}`: {}",
                            ident["error"]["message"].as_str().unwrap_or("key problem")
                        ),
                    }
                }
                let vars_ok = payload["vars_ok"].as_bool().unwrap_or(false);
                if vars_ok && !ok {
                    eprintln!(
                        "Next: {}",
                        payload["next_step"]
                            .as_str()
                            .unwrap_or("plane-cli config where")
                    );
                } else if ok {
                    eprintln!("Configuration looks good.");
                    let flag = |k: &str| payload["anon_telemetry"][k].as_bool().unwrap_or(false);
                    if !flag("build_has_endpoint") {
                        eprintln!(
                            "Error telemetry: inactive (this build has no telemetry endpoint)"
                        );
                    } else if flag("enabled") {
                        eprintln!("Error telemetry: enabled (PLANE_CLI_ANON_TELEMETRY)");
                    } else {
                        eprintln!(
                            "Error telemetry: off (optional: `config update` or set PLANE_CLI_ANON_TELEMETRY=1)"
                        );
                    }
                    eprintln!("Next: plane-cli auth whoami");
                } else {
                    eprintln!("Missing one or more required variables:");
                    let vars = &payload["vars"];
                    for name in [
                        "PLANE_API_BASE_URL",
                        "PLANE_API_KEY",
                        "PLANE_WORKSPACE_SLUG",
                    ] {
                        let set = vars.get(name).and_then(|v| v.as_bool()).unwrap_or(false);
                        let mark = if set { "set" } else { "missing" };
                        eprintln!("- {name}: {mark}");
                    }
                    eprintln!(
                        "Next step: {}",
                        payload["next_step"]
                            .as_str()
                            .unwrap_or("plane-cli config init")
                    );
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// Sets env vars for the test and restores them on drop.
    struct EnvSet(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl EnvSet {
        fn new(vars: &[(&'static str, Option<&str>)]) -> Self {
            let saved = vars
                .iter()
                .map(|(k, v)| {
                    let old = std::env::var_os(k);
                    match v {
                        Some(v) => std::env::set_var(k, v),
                        None => std::env::remove_var(k),
                    }
                    (*k, old)
                })
                .collect();
            EnvSet(saved)
        }
    }
    impl Drop for EnvSet {
        fn drop(&mut self) {
            for (k, v) in &self.0 {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    #[test]
    fn doctor_reports_and_verifies_the_member_identity() {
        let _g = crate::plane_env_files::lock_env();
        let key = "plane_api_docbot_mock_key";
        let mut server = mockito::Server::new();
        let _me = server
            .mock("GET", "/api/v1/users/me/")
            .match_header("x-api-key", key)
            .with_status(200)
            .with_body(r#"{"id":"u-docbot","display_name":"docbot"}"#)
            .create();
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("cfg");
        let cache = tmp.path().join("cache");
        let url = server.url();
        let _env = EnvSet::new(&[
            ("PLANE_API_BASE_URL", Some(url.as_str())),
            ("PLANE_WORKSPACE_SLUG", Some("ws")),
            ("PLANE_API_KEY", None),
            ("PLANE_CLI_AS", None),
            ("PLANE_DOTENV", None),
            ("PLANE_API_KEY_DOCBOT", Some(key)),
            ("XDG_CONFIG_HOME", cfg.to_str()),
            ("XDG_CACHE_HOME", cache.to_str()),
        ]);
        let cli =
            Cli::try_parse_from(["plane-cli", "--json", "--as", "docbot", "config", "doctor"])
                .unwrap();
        let p = doctor_payload(&cli, false);
        let s = p.to_string();
        assert!(!s.contains(key), "key in doctor payload: {s}");
        assert_eq!(p["ok"], json!(true), "{s}");
        assert_eq!(p["vars_ok"], json!(true));
        let id = &p["identity"];
        assert_eq!(id["member"], json!("docbot"));
        assert_eq!(id["origin"], json!("flag"));
        assert_eq!(id["key_found"], json!(true));
        assert_eq!(id["key_source"]["kind"], json!("process_env"));
        assert_eq!(id["key_source"]["var"], json!("PLANE_API_KEY_DOCBOT"));
        assert_eq!(id["verified"], json!(true));
        assert_eq!(id["user_id"], json!("u-docbot"));

        // A member with no key: the code is reported, doctor is not ok, next step is set-key.
        let cli = Cli::try_parse_from([
            "plane-cli",
            "--json",
            "--as",
            "nokeybot",
            "config",
            "doctor",
        ])
        .unwrap();
        let p = doctor_payload(&cli, true);
        assert_eq!(p["ok"], json!(false));
        assert_eq!(p["identity"]["key_found"], json!(false));
        assert_eq!(
            p["identity"]["error"]["code"],
            json!("member_key_not_found")
        );
        assert_eq!(
            p["next_step"],
            json!("plane-cli config set-key --member <member>")
        );
        let _ = plane_core::take_warnings();
    }
}
