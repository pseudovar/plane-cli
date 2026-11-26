//! Default `plane-cli` env files: user-global then per-directory (like `gh` hosts vs repo config).

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

/// The only keys read from a file in the current directory (`./.plane-cli.env`, `./.env`). A
/// cloned repository controls those files, so it must not set the acting member, telemetry,
/// `PLANE_DOTENV`, or anything that steers the process (`HTTPS_PROXY`, `HOME`, `PATH`, ...).
const CWD_ALLOWED_KEYS: &[&str] = &[
    BASE_URL_KEY,
    API_KEY_KEY,
    "PLANE_WORKSPACE_SLUG",
    "PLANE_CACHE",
    "PLANE_CLI_JSON",
];
const BASE_URL_KEY: &str = "PLANE_API_BASE_URL";
const API_KEY_KEY: &str = "PLANE_API_KEY";

/// Whether the effective `PLANE_API_BASE_URL` came from a file in the current directory.
static BASE_URL_FROM_CWD: AtomicBool = AtomicBool::new(false);

/// XDG-style global env file (Unix: `$XDG_CONFIG_HOME/plane-cli/.plane-cli.env` or `~/.config/plane-cli/.plane-cli.env`; Windows: `%APPDATA%\\plane-cli\\.plane-cli.env`).
pub fn global_plane_cli_env_path() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(config_home.join("plane-cli").join(".plane-cli.env"))
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .filter(|s| !s.is_empty())
            .map(|a| PathBuf::from(a).join("plane-cli").join(".plane-cli.env"))
    }
}

pub fn local_plane_cli_env_path() -> Result<PathBuf, String> {
    Ok(std::env::current_dir()
        .map_err(|e| format!("could not read current directory: {e}"))?
        .join(".plane-cli.env"))
}

/// One env file layer: `config where` kind, path, and whether it was asked for explicitly
/// (a missing explicit file is an error; a missing default file is skipped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvLayer {
    pub kind: &'static str,
    pub path: PathBuf,
    pub explicit: bool,
}

/// Env file layers in precedence order, highest first: `--env-file`, `PLANE_DOTENV`,
/// `--dotenv` (`./.env`), `./.plane-cli.env` (local), then the user-global file.
/// Non-empty process env beats all of them.
pub fn env_file_layers(env_file: Option<&Path>, dotenv: bool) -> Vec<EnvLayer> {
    let mut layers = Vec::new();
    if let Some(p) = env_file {
        layers.push(EnvLayer {
            kind: "cli_env_file",
            path: p.to_path_buf(),
            explicit: true,
        });
    }
    layers.extend(plane_dotenv_layer());
    if dotenv {
        if let Ok(cwd) = std::env::current_dir() {
            layers.push(EnvLayer {
                kind: "dotenv_cwd",
                path: cwd.join(".env"),
                explicit: false,
            });
        }
    }
    // In the config directory, `./.plane-cli.env` is the global file: list it once, as global.
    if let Some(p) = local_plane_cli_env_path()
        .ok()
        .filter(|p| !is_global_path(p))
    {
        layers.push(EnvLayer {
            kind: "local",
            path: p,
            explicit: false,
        });
    }
    if let Some(p) = global_plane_cli_env_path() {
        layers.push(EnvLayer {
            kind: "global",
            path: p,
            explicit: false,
        });
    }
    layers
}

/// The layer `PLANE_DOTENV` asks for: `1`/`true`/`yes` is `./.env` (optional), `0`/`false`/`no`
/// is off, and any other value is a file path (required). Read from the process environment
/// only, so no env file can turn it on.
pub fn plane_dotenv_layer() -> Option<EnvLayer> {
    if PROCESS_LOCK
        .get()
        .is_some_and(|lock| !lock.contains("PLANE_DOTENV"))
    {
        return None;
    }
    let value = nonempty_env("PLANE_DOTENV")?;
    match value.to_lowercase().as_str() {
        "0" | "false" | "no" => None,
        "1" | "true" | "yes" => std::env::current_dir().ok().map(|cwd| EnvLayer {
            kind: "plane_dotenv",
            path: cwd.join(".env"),
            explicit: false,
        }),
        _ => Some(EnvLayer {
            kind: "plane_dotenv",
            path: PathBuf::from(value),
            explicit: true,
        }),
    }
}

/// Whether `path` names the user's own global env file: that file name, in the canonical
/// global config directory. The file itself may be a symlink (dotfile managers). A link
/// elsewhere that points at it does not count.
pub fn is_global_path(path: &Path) -> bool {
    let Some(global) = global_plane_cli_env_path() else {
        return false;
    };
    if path.file_name() != global.file_name() {
        return false;
    }
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    match (
        global.parent().and_then(|g| g.canonicalize().ok()),
        dir.canonicalize(),
    ) {
        (Some(g), Ok(d)) => g == d,
        _ => false,
    }
}

/// Whether `path`, as named or once resolved, is inside the current directory. The parent is
/// checked too: a symlink placed in the directory is controlled by whoever owns the directory,
/// wherever it points.
fn path_under_cwd(path: &Path) -> bool {
    if path.is_relative() {
        return true;
    }
    let Some(cwd) = std::env::current_dir()
        .ok()
        .and_then(|c| c.canonicalize().ok())
    else {
        return true;
    };
    let resolved_under = path.canonicalize().is_ok_and(|p| p.starts_with(&cwd));
    let parent_under = path
        .parent()
        .and_then(|p| p.canonicalize().ok())
        .is_some_and(|p| p.starts_with(&cwd));
    resolved_under || parent_under || path.starts_with(&cwd)
}

/// Whether a repository checked out in the current directory could control this layer. The
/// user's own global file stays trusted even when the current directory holds it.
fn is_cwd_layer(layer: &EnvLayer) -> bool {
    if is_global_path(&layer.path) {
        return false;
    }
    match layer.kind {
        "local" | "dotenv_cwd" => true,
        "plane_dotenv" => path_under_cwd(&layer.path),
        _ => false,
    }
}

/// Largest env file read from the current directory.
const MAX_CWD_ENV_FILE: u64 = 64 * 1024;

/// Why a file in the current directory could not be read. Never carries file content.
#[derive(Debug, PartialEq, Eq)]
enum CwdReadError {
    Missing,
    NotRegular,
    TooLarge,
    Unreadable,
}

#[cfg(unix)]
fn open_no_follow(path: &Path) -> Result<std::fs::File, CwdReadError> {
    use std::os::unix::fs::OpenOptionsExt;
    // O_NOFOLLOW refuses a symlink at open, so there is no check-then-open race. O_NONBLOCK
    // keeps a FIFO from hanging the open.
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| match (e.kind(), e.raw_os_error()) {
            (std::io::ErrorKind::NotFound, _) => CwdReadError::Missing,
            (_, Some(libc::ELOOP)) => CwdReadError::NotRegular,
            _ => CwdReadError::Unreadable,
        })
}

#[cfg(not(unix))]
fn open_no_follow(path: &Path) -> Result<std::fs::File, CwdReadError> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => CwdReadError::Missing,
        _ => CwdReadError::Unreadable,
    })?;
    if !meta.is_file() {
        return Err(CwdReadError::NotRegular);
    }
    std::fs::File::open(path).map_err(|_| CwdReadError::Unreadable)
}

/// One bounded read of a regular file, checked on the opened handle.
fn read_cwd_file(path: &Path) -> Result<Vec<u8>, CwdReadError> {
    use std::io::Read;
    let file = open_no_follow(path)?;
    let meta = file.metadata().map_err(|_| CwdReadError::Unreadable)?;
    if !meta.is_file() {
        return Err(CwdReadError::NotRegular);
    }
    if meta.len() > MAX_CWD_ENV_FILE {
        return Err(CwdReadError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(MAX_CWD_ENV_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CwdReadError::Unreadable)?;
    if bytes.len() as u64 > MAX_CWD_ENV_FILE {
        return Err(CwdReadError::TooLarge);
    }
    Ok(bytes)
}

/// Parse env-file bytes. The error is empty on purpose: dotenvy's quotes the offending line.
fn parse_bytes(bytes: Vec<u8>) -> Result<Vec<(String, String)>, ()> {
    dotenvy::from_read_iter(std::io::Cursor::new(bytes))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ())
}

/// Key/value pairs from a file in the current directory, or a refusal that names the file and
/// never its content.
///
/// A symlink or other non-regular file is ignored with a warning (a required `PLANE_DOTENV`
/// path is an error). A file with `$` that sets an allowed key is an error rather than ignored:
/// dotenvy would expand `$VAR` from the process environment, and skipping the file could send a
/// write to the global server instead. With `$` and no allowed key there is nothing to use, so
/// it is ignored with a warning.
fn load_cwd_items(layer: &EnvLayer) -> Result<Vec<(String, String)>, EnvLoadError> {
    let name = layer.path.display();
    let refuse = |code, why: String| EnvLoadError {
        code: Some(code),
        message: format!("refusing {name}: {why}"),
    };
    let bytes = match read_cwd_file(&layer.path) {
        Ok(b) => b,
        Err(CwdReadError::Missing) if layer.explicit => {
            return Err(layer_error(layer, "file not found"))
        }
        Err(CwdReadError::Missing) => return Ok(Vec::new()),
        Err(CwdReadError::NotRegular) if layer.explicit => return Err(refuse(
            "env_file_unsafe",
            "a file in the current directory must be a regular file, not a symlink, FIFO or device"
                .into(),
        )),
        Err(CwdReadError::NotRegular) => {
            warn_once(format!(
                "{name} was ignored: a file in the current directory must be a regular file, not a symlink, FIFO or device. Use the global env file or --env-file instead"
            ));
            return Ok(Vec::new());
        }
        Err(CwdReadError::TooLarge) => {
            return Err(refuse(
                "env_file_too_large",
                format!("a file in the current directory may be at most {MAX_CWD_ENV_FILE} bytes"),
            ))
        }
        Err(CwdReadError::Unreadable) => {
            return Err(refuse("env_file_unreadable", "it cannot be read".into()))
        }
    };
    let has_dollar = bytes.contains(&b'$');
    let items = parse_bytes(bytes)
        .map_err(|()| refuse("env_file_invalid", "it is not a valid env file".into()))?;
    if !has_dollar {
        return Ok(items);
    }
    if items
        .iter()
        .any(|(k, _)| CWD_ALLOWED_KEYS.contains(&k.as_str()))
    {
        return Err(refuse(
            "env_file_unsafe",
            "it sets a plane-cli setting and contains `$`, which is not allowed in a file in the current directory (variable expansion). Remove the `$`, or pass the file with --env-file if you trust it".into(),
        ));
    }
    warn_once(format!(
        "{name} was ignored: it contains `$` and sets no plane-cli settings"
    ));
    Ok(Vec::new())
}

/// Whether `path` is a file directly in the current directory, other than the global file.
pub fn is_in_cwd(path: &Path) -> bool {
    if is_global_path(path) {
        return false;
    }
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => return true,
    };
    match (parent.canonicalize(), std::env::current_dir()) {
        (Ok(p), Ok(cwd)) => cwd.canonicalize().is_ok_and(|c| c == p),
        _ => false,
    }
}

/// Whether the effective base URL came from a file in the current directory. Such a URL is
/// only ever paired with a key from that same file, never with a member key.
pub fn base_url_from_cwd() -> bool {
    BASE_URL_FROM_CWD.load(Ordering::Relaxed)
}

/// Env files load more than once per run; each warning is queued once.
fn warn_once(message: String) {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let seen = SEEN.get_or_init(|| Mutex::new(HashSet::new()));
    if seen
        .lock()
        .map(|mut s| s.insert(message.clone()))
        .unwrap_or(true)
    {
        plane_core::push_warning(message);
    }
}

/// A failure loading env files. `code` is a stable `error.code` when the failure is a refusal
/// of a file in the current directory.
#[derive(Debug)]
pub struct EnvLoadError {
    pub code: Option<&'static str>,
    pub message: String,
}

impl std::fmt::Display for EnvLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<EnvLoadError> for plane_core::CoreError {
    fn from(e: EnvLoadError) -> Self {
        match e.code {
            Some(code) => plane_core::CoreError::usage_code(code, e.message),
            None => plane_core::CoreError::Usage(e.message),
        }
    }
}

fn layer_error(layer: &EnvLayer, e: impl std::fmt::Display) -> EnvLoadError {
    let message = match layer.kind {
        "cli_env_file" => format!("--env-file {}: {e}", layer.path.display()),
        "plane_dotenv" => format!("PLANE_DOTENV ({}): {e}", layer.path.display()),
        _ => format!("could not load {}: {e}", layer.path.display()),
    };
    EnvLoadError {
        code: None,
        message,
    }
}

/// Apply `layers` in precedence order: for each key, the first layer that defines it wins,
/// and keys in `lock` (non-empty in the process env at startup) are never touched.
/// Every layer is attempted; the first error is returned after loading the rest.
///
/// A file in the current directory is untrusted: only [`CWD_ALLOWED_KEYS`] are read from it,
/// and its `PLANE_API_BASE_URL` counts only when the same file also supplies the API key that
/// will be used. Otherwise a cloned repository could send the user's key to its own server.
fn load_layers(layers: &[EnvLayer], lock: &HashSet<String>) -> Result<(), EnvLoadError> {
    let mut chosen: HashSet<String> = HashSet::new();
    let mut first_err: Option<EnvLoadError> = None;
    let mut url_from_cwd = false;
    for layer in layers {
        let from_cwd = is_cwd_layer(layer);
        let items = if from_cwd {
            match load_cwd_items(layer) {
                Ok(items) => items,
                Err(e) => {
                    first_err.get_or_insert(e);
                    continue;
                }
            }
        } else {
            if !layer.path.exists() {
                if layer.explicit {
                    first_err.get_or_insert_with(|| layer_error(layer, "file not found"));
                }
                continue;
            }
            let iter = match dotenvy::from_path_iter(&layer.path) {
                Ok(it) => it,
                Err(e) => {
                    first_err.get_or_insert_with(|| layer_error(layer, e));
                    continue;
                }
            };
            let mut items = Vec::new();
            for item in iter {
                match item {
                    Ok(pair) => items.push(pair),
                    Err(e) => {
                        first_err.get_or_insert_with(|| layer_error(layer, e));
                        break;
                    }
                }
            }
            items
        };
        // Decided before this file's keys apply: is this file's key the one that will be sent?
        let file_key_is_used = !lock.contains(API_KEY_KEY)
            && !chosen.contains(API_KEY_KEY)
            && items
                .iter()
                .find(|(k, _)| k == API_KEY_KEY)
                .is_some_and(|(_, v)| !v.trim().is_empty());
        for (key, value) in items {
            if lock.contains(&key) || chosen.contains(&key) {
                continue;
            }
            if from_cwd {
                if !CWD_ALLOWED_KEYS.contains(&key.as_str()) {
                    if key.starts_with("PLANE_") {
                        warn_once(format!(
                            "{key} in {} was ignored: this setting is not read from a file in the current directory",
                            layer.path.display()
                        ));
                    }
                    continue;
                }
                if key == BASE_URL_KEY {
                    if !file_key_is_used {
                        warn_once(format!(
                            "{BASE_URL_KEY} in {} was ignored: a URL in the current directory is used only when the same file also supplies the API key",
                            layer.path.display()
                        ));
                        continue;
                    }
                    url_from_cwd = true;
                    let host = crate::net_diag::host_of(&value)
                        .unwrap_or_else(|| "an unparseable host".into());
                    warn_once(format!(
                        "requests go to {host}, set by {} in the current directory, using that file's API key. Treat that server's responses as untrusted content",
                        layer.path.display()
                    ));
                }
            }
            std::env::set_var(&key, value);
            chosen.insert(key);
        }
    }
    BASE_URL_FROM_CWD.store(url_from_cwd, Ordering::Relaxed);
    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Load env files in the documented order (process env → `--env-file` → `PLANE_DOTENV` →
/// `--dotenv` → local → global; first layer wins per key). Idempotent: the lock is the
/// process env captured by [`snapshot_process_env`], so calling it again with the parsed CLI
/// flags after an early argv pre-scan gives the same result.
pub fn load_env_layers(env_file: Option<&Path>, dotenv: bool) -> Result<(), EnvLoadError> {
    let lock = PROCESS_LOCK
        .get()
        .cloned()
        .unwrap_or_else(env_keys_nonempty);
    load_layers(&env_file_layers(env_file, dotenv), &lock)
}

/// `--env-file PATH` / `--env-file=PATH` and `--dotenv` from raw argv, for loading env files
/// before clap runs (telemetry opt-in can come from an env file).
pub fn prescan_env_args(argv: &[String]) -> (Option<PathBuf>, bool) {
    let mut env_file = None;
    let mut dotenv = false;
    let mut it = argv.iter().skip(1);
    while let Some(a) = it.next() {
        if a == "--" {
            break;
        }
        if a == "--dotenv" {
            dotenv = true;
        } else if a == "--env-file" {
            env_file = it.next().map(PathBuf::from);
        } else if let Some(v) = a.strip_prefix("--env-file=") {
            env_file = Some(PathBuf::from(v));
        }
    }
    (env_file, dotenv)
}

pub fn env_keys_nonempty() -> HashSet<String> {
    std::env::vars()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, _)| k)
        .collect()
}

/// Key names defined in an env file (values are not returned).
pub fn keys_in_env_file(path: &Path) -> Result<Vec<String>, String> {
    let mut keys = Vec::new();
    if is_cwd_controlled(path) {
        // Diagnostics stay usable with a bad file: report it, list no keys.
        let parsed = match read_cwd_file(path) {
            Err(CwdReadError::Missing) => return Ok(keys),
            Ok(bytes) => parse_bytes(bytes).ok(),
            Err(_) => None,
        };
        match parsed {
            Some(items) => keys.extend(items.into_iter().map(|(k, _)| k)),
            None => warn_once(format!(
                "{} was not read: it must be a valid, regular env file of at most {MAX_CWD_ENV_FILE} bytes",
                path.display()
            )),
        }
    } else {
        if !path.exists() {
            return Ok(keys);
        }
        let iter = dotenvy::from_path_iter(path)
            .map_err(|e| format!("could not read {}: {e}", path.display()))?;
        for item in iter {
            let (key, _) = item.map_err(|e| format!("could not parse {}: {e}", path.display()))?;
            keys.push(key);
        }
    }
    keys.sort();
    keys.dedup();
    Ok(keys)
}

/// Whether a repository in the current directory could control the file at `path`.
fn is_cwd_controlled(path: &Path) -> bool {
    !is_global_path(path) && path_under_cwd(path)
}

static PROCESS_BASE_URL: OnceLock<Option<String>> = OnceLock::new();
static PROCESS_LOCK: OnceLock<HashSet<String>> = OnceLock::new();

/// Record the process environment before any env file is loaded: the keys that are
/// non-empty (env files never override them) and `PLANE_API_BASE_URL` (for
/// [`base_url_source`]). Call first thing in `main`.
pub fn snapshot_process_env() {
    let _ = PROCESS_BASE_URL.set(nonempty_env("PLANE_API_BASE_URL"));
    let _ = PROCESS_LOCK.set(env_keys_nonempty());
}

/// Whether `key` was non-empty in the process environment before any env file loaded
/// (from [`snapshot_process_env`]; falls back to the current env if no snapshot was taken).
pub fn in_process_env(key: &str) -> bool {
    match PROCESS_LOCK.get() {
        Some(lock) => lock.contains(key),
        None => nonempty_env(key).is_some(),
    }
}

/// Which layer supplied the effective value of `key` (never the value itself): the process
/// environment if it was set there at startup, else the first env file layer that sets it to
/// the effective value.
pub fn value_source(key: &str, env_file: Option<&Path>, dotenv: bool) -> ValueSource {
    let Some(effective) = nonempty_env(key) else {
        return ValueSource {
            layer: "unset",
            path: None,
        };
    };
    if PROCESS_LOCK.get().is_some_and(|lock| lock.contains(key)) {
        return ValueSource {
            layer: "process_env",
            path: None,
        };
    }
    for EnvLayer {
        kind: layer, path, ..
    } in env_file_layers(env_file, dotenv)
    {
        let matches = read_env_file(&path)
            .ok()
            .and_then(|m| m.get(key).map(|v| v.trim() == effective))
            .unwrap_or(false);
        if matches {
            return ValueSource {
                layer,
                path: Some(path),
            };
        }
    }
    ValueSource {
        layer: "unknown",
        path: None,
    }
}

fn nonempty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Which config layer supplied a value. `layer` uses the same names as `config where`
/// (`cli_env_file`, `plane_dotenv`, `dotenv_cwd`, `global`, `local`) plus `process_env`,
/// `unset` and `unknown`. Never carries the value itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueSource {
    pub layer: &'static str,
    pub path: Option<PathBuf>,
}

impl ValueSource {
    /// `process env` or `global file /home/x/.config/plane-cli/.plane-cli.env`.
    pub fn describe(&self) -> String {
        match (&self.layer, &self.path) {
            (&"process_env", _) => "process env".into(),
            (layer, Some(p)) => format!("{} {}", layer_label(layer), p.display()),
            (layer, None) => layer_label(layer).into(),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "layer": self.layer,
            "path": self.path.as_ref().map(|p| p.display().to_string()),
        })
    }
}

fn layer_label(layer: &str) -> &str {
    match layer {
        "cli_env_file" => "--env-file",
        "plane_dotenv" => "PLANE_DOTENV file",
        "dotenv_cwd" => "--dotenv file",
        "global" => "global file",
        "local" => "local file",
        "unset" => "nowhere (unset)",
        _ => "an unknown layer",
    }
}

/// Find the layer that supplied the effective `PLANE_API_BASE_URL`, by matching the value
/// against the process snapshot and each file layer in precedence order ([`env_file_layers`]),
/// so the first match is the layer that actually won.
pub fn base_url_source(env_file: Option<&Path>, dotenv: bool) -> ValueSource {
    const KEY: &str = "PLANE_API_BASE_URL";
    let Some(effective) = nonempty_env(KEY) else {
        return ValueSource {
            layer: "unset",
            path: None,
        };
    };
    if PROCESS_BASE_URL.get().cloned().flatten().as_deref() == Some(effective.as_str()) {
        return ValueSource {
            layer: "process_env",
            path: None,
        };
    }
    for candidate in env_file_layers(env_file, dotenv) {
        // A URL ignored from the current directory must not be credited for an equal value.
        if is_cwd_layer(&candidate) && !base_url_from_cwd() {
            continue;
        }
        let matches = read_env_file(&candidate.path)
            .ok()
            .and_then(|m| m.get(KEY).map(|v| v.trim() == effective))
            .unwrap_or(false);
        if matches {
            return ValueSource {
                layer: candidate.kind,
                path: Some(candidate.path),
            };
        }
    }
    ValueSource {
        layer: "unknown",
        path: None,
    }
}

/// Parsed key/value pairs from a single env file (for `config update` defaults).
pub fn read_env_file(path: &Path) -> Result<HashMap<String, String>, String> {
    if is_cwd_controlled(path) {
        // One bounded read. Parsing expands `$VAR`, so never pull process secrets through a file
        // a repository controls (`config update` would write them back to it).
        let name = path.display();
        let bytes = match read_cwd_file(path) {
            Ok(b) => b,
            Err(CwdReadError::Missing) => return Ok(HashMap::new()),
            Err(CwdReadError::NotRegular) => {
                return Err(format!("{name} is not a regular file"));
            }
            Err(CwdReadError::TooLarge) => {
                return Err(format!("{name} is larger than {MAX_CWD_ENV_FILE} bytes"));
            }
            Err(CwdReadError::Unreadable) => return Err(format!("could not read {name}")),
        };
        if bytes.contains(&b'$') {
            return Err(format!(
                "{name} contains `$`, which is not allowed in a file in the current directory"
            ));
        }
        return parse_bytes(bytes)
            .map(|items| items.into_iter().collect())
            .map_err(|()| format!("{name} is not a valid env file"));
    }
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let mut out = HashMap::new();
    let iter = dotenvy::from_path_iter(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    for item in iter {
        let (key, value) = item.map_err(|e| format!("could not parse {}: {e}", path.display()))?;
        out.insert(key, value);
    }
    Ok(out)
}

/// Serializes tests that mutate `PLANE_*` process env vars (shared across modules).
#[cfg(test)]
pub(crate) fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    static ENV_MUTEX: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
    ENV_MUTEX
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_source_finds_the_file_that_set_it() {
        let _g = lock_env();
        let dir = tempfile::tempdir().expect("tempdir");
        let f = dir.path().join("explicit.env");
        std::fs::write(&f, "PLANE_API_BASE_URL=http://from-file.example:8087\n").unwrap();
        std::env::set_var("PLANE_API_BASE_URL", "http://from-file.example:8087");
        let src = base_url_source(Some(&f), false);
        std::env::remove_var("PLANE_API_BASE_URL");
        assert_eq!(src.layer, "cli_env_file");
        assert_eq!(src.path.as_deref(), Some(f.as_path()));
        assert!(src.describe().contains("explicit.env"));
    }

    #[test]
    fn base_url_source_unset_when_missing() {
        let _g = lock_env();
        std::env::remove_var("PLANE_API_BASE_URL");
        assert_eq!(base_url_source(None, false).layer, "unset");
    }

    #[test]
    fn local_overrides_global_when_not_explicitly_locked() {
        let _g = lock_env();
        let dir = tempfile::tempdir().expect("tempdir");
        let global = dir.path().join("global.env");
        let local = dir.path().join("local.env");
        std::fs::write(
            &global,
            "PLANE_API_BASE_URL=https://global.example\nPLANE_API_KEY=global-key\nPLANE_WORKSPACE_SLUG=ws-global\n",
        )
        .unwrap();
        std::fs::write(&local, "PLANE_API_KEY=local-key\n").unwrap();

        let layer = |kind, path: &Path| EnvLayer {
            kind,
            path: path.to_path_buf(),
            explicit: false,
        };
        let lock = env_keys_nonempty();
        load_layers(&[layer("local", &local), layer("global", &global)], &lock).unwrap();

        assert_eq!(
            std::env::var("PLANE_API_BASE_URL").unwrap(),
            "https://global.example"
        );
        assert_eq!(std::env::var("PLANE_API_KEY").unwrap(), "local-key");
        assert_eq!(std::env::var("PLANE_WORKSPACE_SLUG").unwrap(), "ws-global");

        std::env::remove_var("PLANE_API_BASE_URL");
        std::env::remove_var("PLANE_API_KEY");
        std::env::remove_var("PLANE_WORKSPACE_SLUG");
    }

    #[test]
    fn nonempty_process_env_locks_key_before_layers() {
        let _g = lock_env();
        std::env::set_var("PLANE_API_KEY", "from-process");
        let dir = tempfile::tempdir().expect("tempdir");
        let f = dir.path().join("x.env");
        std::fs::write(&f, "PLANE_API_KEY=file-key\n").unwrap();

        let lock = env_keys_nonempty();
        let layers = [EnvLayer {
            kind: "cli_env_file",
            path: f.clone(),
            explicit: true,
        }];
        load_layers(&layers, &lock).unwrap();
        assert_eq!(std::env::var("PLANE_API_KEY").unwrap(), "from-process");

        std::env::remove_var("PLANE_API_KEY");
    }

    /// Restores env vars on drop.
    struct EnvRestore(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl EnvRestore {
        fn capture(keys: &[&'static str]) -> Self {
            EnvRestore(keys.iter().map(|k| (*k, std::env::var_os(k))).collect())
        }
    }
    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (k, v) in &self.0 {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
        }
    }

    /// `--env-file` must beat the global file, and attribution must agree.
    #[test]
    fn env_file_beats_global_file_and_attribution_agrees() {
        let _g = lock_env();
        let _restore = EnvRestore::capture(&[
            "XDG_CONFIG_HOME",
            "PLANE_DOTENV",
            "PLANE_API_BASE_URL",
            "PLANE_CLI_ORDER_TEST_ONLY_GLOBAL",
        ]);
        let dir = tempfile::tempdir().expect("tempdir");
        let global_dir = dir.path().join("plane-cli");
        std::fs::create_dir_all(&global_dir).unwrap();
        std::fs::write(
            global_dir.join(".plane-cli.env"),
            "PLANE_API_BASE_URL=http://global.example:8087\nPLANE_CLI_ORDER_TEST_ONLY_GLOBAL=g\n",
        )
        .unwrap();
        let explicit = dir.path().join("explicit.env");
        std::fs::write(&explicit, "PLANE_API_BASE_URL=https://explicit.example\n").unwrap();
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
        std::env::remove_var("PLANE_DOTENV");
        std::env::remove_var("PLANE_API_BASE_URL");
        std::env::remove_var("PLANE_CLI_ORDER_TEST_ONLY_GLOBAL");

        // Twice: main pre-scans argv before clap, then loads again with the parsed flags.
        load_env_layers(Some(&explicit), false).unwrap();
        load_env_layers(Some(&explicit), false).unwrap();

        assert_eq!(
            std::env::var("PLANE_API_BASE_URL").unwrap(),
            "https://explicit.example"
        );
        // Lower layers still fill keys the explicit file does not set.
        assert_eq!(
            std::env::var("PLANE_CLI_ORDER_TEST_ONLY_GLOBAL").unwrap(),
            "g"
        );
        let src = base_url_source(Some(&explicit), false);
        assert_eq!(src.layer, "cli_env_file");
        assert_eq!(src.path.as_deref(), Some(explicit.as_path()));
    }

    #[test]
    fn missing_explicit_env_file_is_an_error() {
        let _g = lock_env();
        let layers = [EnvLayer {
            kind: "cli_env_file",
            path: PathBuf::from("/nonexistent/plane-cli-test.env"),
            explicit: true,
        }];
        let err = load_layers(&layers, &HashSet::new()).unwrap_err();
        assert!(
            err.message
                .starts_with("--env-file /nonexistent/plane-cli-test.env"),
            "{err}"
        );
    }

    #[test]
    fn layer_order_matches_docs() {
        let _g = lock_env();
        let _restore = EnvRestore::capture(&["PLANE_DOTENV"]);
        std::env::set_var("PLANE_DOTENV", "/tmp/pd.env");
        let kinds: Vec<&str> = env_file_layers(Some(Path::new("/tmp/e.env")), true)
            .iter()
            .map(|l| l.kind)
            .collect();
        assert_eq!(
            kinds,
            vec![
                "cli_env_file",
                "plane_dotenv",
                "dotenv_cwd",
                "local",
                "global"
            ]
        );
    }

    fn cwd_layer(path: &Path) -> EnvLayer {
        EnvLayer {
            kind: "local",
            path: path.to_path_buf(),
            explicit: false,
        }
    }

    fn global_layer(path: &Path) -> EnvLayer {
        EnvLayer {
            kind: "global",
            path: path.to_path_buf(),
            explicit: false,
        }
    }

    const TRUST_KEYS: &[&str] = &[
        "PLANE_API_BASE_URL",
        "PLANE_API_KEY",
        "PLANE_CLI_AS",
        "PLANE_CLI_ANON_TELEMETRY",
        "PLANE_DOTENV",
        "HTTPS_PROXY",
    ];

    fn clear_trust_keys() -> EnvRestore {
        let restore = EnvRestore::capture(TRUST_KEYS);
        for k in TRUST_KEYS {
            std::env::remove_var(k);
        }
        restore
    }

    #[test]
    fn cwd_url_without_its_own_key_is_ignored() {
        let _g = lock_env();
        let _r = clear_trust_keys();
        let dir = tempfile::tempdir().unwrap();
        let (local, global) = (dir.path().join("l.env"), dir.path().join("g.env"));
        std::fs::write(&local, "PLANE_API_BASE_URL=https://evil.example\n").unwrap();
        std::fs::write(
            &global,
            "PLANE_API_BASE_URL=https://plane.example.com\nPLANE_API_KEY=user-key\n",
        )
        .unwrap();
        load_layers(&[cwd_layer(&local), global_layer(&global)], &HashSet::new()).unwrap();
        assert_eq!(
            std::env::var("PLANE_API_BASE_URL").unwrap(),
            "https://plane.example.com"
        );
        assert!(!base_url_from_cwd());
    }

    #[test]
    fn cwd_url_with_its_own_key_is_used_unless_another_key_wins() {
        let _g = lock_env();
        let _r = clear_trust_keys();
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("l.env");
        std::fs::write(
            &local,
            "PLANE_API_BASE_URL=https://own.example\nPLANE_API_KEY=own-key\n",
        )
        .unwrap();
        load_layers(&[cwd_layer(&local)], &HashSet::new()).unwrap();
        assert_eq!(
            std::env::var("PLANE_API_BASE_URL").unwrap(),
            "https://own.example"
        );
        assert!(base_url_from_cwd());
        std::env::remove_var("PLANE_API_BASE_URL");

        // A key from the process env would be the one sent, so the URL is refused.
        let lock = HashSet::from(["PLANE_API_KEY".to_string()]);
        load_layers(&[cwd_layer(&local)], &lock).unwrap();
        assert!(std::env::var("PLANE_API_BASE_URL").is_err());
        assert!(!base_url_from_cwd());
    }

    #[test]
    fn cwd_files_cannot_set_identity_telemetry_or_process_settings() {
        let _g = lock_env();
        let _r = clear_trust_keys();
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("l.env");
        std::fs::write(
            &local,
            "PLANE_CLI_AS=mallory\nPLANE_CLI_ANON_TELEMETRY=1\nPLANE_DOTENV=1\nHTTPS_PROXY=http://evil.example\n",
        )
        .unwrap();
        load_layers(&[cwd_layer(&local)], &HashSet::new()).unwrap();
        for k in TRUST_KEYS {
            assert!(std::env::var(k).is_err(), "{k} was accepted");
        }
    }

    #[test]
    fn cwd_file_with_dollar_fails_closed_whatever_the_quoting() {
        let _g = lock_env();
        let _r = clear_trust_keys();
        let _s = EnvRestore::capture(&["PLANE_TEST_SECRET"]);
        std::env::set_var("PLANE_TEST_SECRET", "secret-value");
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("l.env");
        for value in [
            "${PLANE_TEST_SECRET}",
            "$PLANE_TEST_SECRET",
            "\"${PLANE_TEST_SECRET}\"",
            "\"$PLANE_TEST_SECRET\"",
            "'${PLANE_TEST_SECRET}'",
            "'$PLANE_TEST_SECRET'",
        ] {
            std::fs::write(
                &local,
                format!("PLANE_API_BASE_URL=https://own.example\nPLANE_API_KEY={value}\n"),
            )
            .unwrap();
            let e = load_layers(&[cwd_layer(&local)], &HashSet::new()).unwrap_err();
            assert_eq!(e.code, Some("env_file_unsafe"), "{value}");
            assert!(e.message.contains("l.env"), "{e}");
            assert!(!e.message.contains("secret-value"), "{e}");
            assert!(std::env::var("PLANE_API_KEY").is_err(), "{value}");
            assert!(std::env::var("PLANE_API_BASE_URL").is_err(), "{value}");
            assert!(!base_url_from_cwd(), "{value}");
        }
    }

    #[test]
    fn dollar_outside_allowed_keys_is_ignored() {
        let _g = lock_env();
        let _r = clear_trust_keys();
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("l.env");
        std::fs::write(&local, "APP_SECRET=$HOME\n# costs $5\n").unwrap();
        load_layers(&[cwd_layer(&local)], &HashSet::new()).unwrap();
        assert!(std::env::var("APP_SECRET").is_err());
    }

    #[test]
    fn cwd_reader_is_bounded_and_regular_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("ok.env");
        std::fs::write(&ok, vec![b'#'; MAX_CWD_ENV_FILE as usize]).unwrap();
        assert!(read_cwd_file(&ok).is_ok());
        let big = dir.path().join("big.env");
        std::fs::write(&big, vec![b'#'; MAX_CWD_ENV_FILE as usize + 1]).unwrap();
        assert_eq!(read_cwd_file(&big), Err(CwdReadError::TooLarge));
        assert_eq!(read_cwd_file(dir.path()), Err(CwdReadError::NotRegular));
        assert_eq!(
            read_cwd_file(&dir.path().join("none")),
            Err(CwdReadError::Missing)
        );
        #[cfg(unix)]
        {
            let link = dir.path().join("link.env");
            std::os::unix::fs::symlink(&ok, &link).unwrap();
            assert_eq!(read_cwd_file(&link), Err(CwdReadError::NotRegular));
            let zero = dir.path().join("zero.env");
            std::os::unix::fs::symlink("/dev/zero", &zero).unwrap();
            assert_eq!(read_cwd_file(&zero), Err(CwdReadError::NotRegular));
            assert_eq!(
                read_cwd_file(Path::new("/dev/zero")),
                Err(CwdReadError::NotRegular)
            );
        }
    }

    #[test]
    fn parse_errors_never_echo_file_content() {
        let _g = lock_env();
        let _r = clear_trust_keys();
        let dir = tempfile::tempdir().unwrap();
        let local = dir.path().join("l.env");
        std::fs::write(&local, "private line with no equals sign\n").unwrap();
        let e = load_layers(&[cwd_layer(&local)], &HashSet::new()).unwrap_err();
        assert_eq!(e.code, Some("env_file_invalid"));
        assert!(!e.message.contains("private line"), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn plane_dotenv_cwd_check_resolves_symlinks_and_dotdot() {
        let _g = lock_env();
        let cwd = std::env::current_dir().unwrap().canonicalize().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("app.env");
        std::fs::write(&target, "X=1\n").unwrap();
        assert!(!path_under_cwd(&target));

        // Inside the current directory, spelled through `..` from outside it.
        let inside = tempfile::tempdir_in(&cwd).unwrap();
        let file = inside.path().join("in.env");
        std::fs::write(&file, "X=1\n").unwrap();
        let name = inside.path().file_name().unwrap();
        let dotted = cwd.join(name).join("..").join(name).join("in.env");
        assert!(path_under_cwd(&dotted));

        // A symlink in the current directory is controlled there, wherever it points.
        let link = inside.path().join("link.env");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(path_under_cwd(&link));

        // A symlink outside that resolves into the current directory.
        let back = outside.path().join("back.env");
        std::os::unix::fs::symlink(&file, &back).unwrap();
        assert!(path_under_cwd(&back));
    }

    #[test]
    fn plane_dotenv_values() {
        let _g = lock_env();
        let _r = EnvRestore::capture(&["PLANE_DOTENV"]);
        for off in ["0", "false", "no"] {
            std::env::set_var("PLANE_DOTENV", off);
            assert_eq!(plane_dotenv_layer(), None);
        }
        for on in ["1", "true", "YES"] {
            std::env::set_var("PLANE_DOTENV", on);
            let l = plane_dotenv_layer().unwrap();
            assert!(l.path.ends_with(".env") && !l.explicit);
            assert!(is_cwd_layer(&l));
        }
        std::env::set_var("PLANE_DOTENV", "/etc/plane/app.env");
        let l = plane_dotenv_layer().unwrap();
        assert_eq!(l.path, PathBuf::from("/etc/plane/app.env"));
        assert!(l.explicit && !is_cwd_layer(&l));
    }

    #[test]
    fn prescan_finds_env_file_and_dotenv() {
        let argv: Vec<String> = [
            "plane-cli",
            "--json",
            "--env-file",
            "/x.env",
            "auth",
            "whoami",
            "--dotenv",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            prescan_env_args(&argv),
            (Some(PathBuf::from("/x.env")), true)
        );
        let argv: Vec<String> = ["plane-cli", "--env-file=/y.env", "--", "--dotenv"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            prescan_env_args(&argv),
            (Some(PathBuf::from("/y.env")), false)
        );
    }

    #[test]
    fn read_env_file_returns_values() {
        let dir = tempfile::tempdir().expect("tempdir");
        let f = dir.path().join("test.env");
        std::fs::write(
            &f,
            "PLANE_API_BASE_URL=https://example.com\nPLANE_WORKSPACE_SLUG=acme\n",
        )
        .unwrap();
        let map = read_env_file(&f).unwrap();
        assert_eq!(
            map.get("PLANE_API_BASE_URL").map(String::as_str),
            Some("https://example.com")
        );
        assert_eq!(
            map.get("PLANE_WORKSPACE_SLUG").map(String::as_str),
            Some("acme")
        );
    }

    #[test]
    fn keys_in_env_file_lists_key_names_only() {
        let dir = tempfile::tempdir().expect("tempdir");
        let f = dir.path().join("test.env");
        std::fs::write(
            &f,
            "PLANE_API_BASE_URL=https://example.com\nPLANE_API_KEY=secret\n",
        )
        .unwrap();
        let keys = keys_in_env_file(&f).unwrap();
        assert_eq!(
            keys,
            vec![
                "PLANE_API_BASE_URL".to_string(),
                "PLANE_API_KEY".to_string()
            ]
        );
    }
}
