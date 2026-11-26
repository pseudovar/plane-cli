//! `--as <member>`: act as a Plane member with that member's own API key.
//!
//! Key lookup order for a member (first hit wins):
//! 1. `PLANE_API_KEY_<MEMBER>` in the process environment (never from an env file);
//! 2. macOS: Keychain item `plane-cli-<member>` (via the `security` CLI);
//! 3. other Unix: `<config dir>/plane-cli/keys/<member>`, refused unless it is a regular
//!    file owned by the current user with no group or other permission bits.
//!
//! The key is then checked against `GET /api/v1/users/me/` (cached per key fingerprint), so a
//! key that belongs to someone else is refused. Keys never appear in output, errors or
//! telemetry: error messages name only the source (variable name, Keychain item, file path).

use crate::plane_env_files;
use plane_core::{CoreError, PlaneApi};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Environment variable that sets the acting member when `--as` is not given.
pub const AS_ENV: &str = "PLANE_CLI_AS";

/// How long a successful `users/me` check is trusted for the same key.
const IDENTITY_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

const MAX_MEMBER_LEN: usize = 32;

/// Member this process acts as, once resolved (for error hints).
static CURRENT_MEMBER: OnceLock<String> = OnceLock::new();

pub fn current_member() -> Option<&'static str> {
    CURRENT_MEMBER.get().map(String::as_str)
}

/// Where the acting member name came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsOrigin {
    Flag,
    ProcessEnv,
    EnvFile,
}

impl AsOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            AsOrigin::Flag => "flag",
            AsOrigin::ProcessEnv => "process_env",
            AsOrigin::EnvFile => "env_file",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActingAs {
    pub member: String,
    pub origin: AsOrigin,
}

/// Normalise and check a member name: 1–32 characters of `a-z`, `0-9` and `-`, starting with
/// a letter or digit. `_` is refused so `ci-bot-2` and `ci_bot_2` cannot share
/// `PLANE_API_KEY_CI_BOT_2`. `source` names where the value came from (`--as`,
/// `PLANE_CLI_AS (process env)`, …). Invalid input is never echoed back (it could be a key).
pub fn validate_member(raw: &str, source: &str) -> Result<String, CoreError> {
    let m = raw.trim().to_ascii_lowercase();
    if m.starts_with("plane_api") {
        return Err(CoreError::usage_code(
            "invalid_member_name",
            format!(
                "{source} takes a Plane member name (e.g. `bob`), and this looks like an API key; \
                 it was not used. Store keys with `plane-cli config set-key --member <name>`."
            ),
        ));
    }
    let valid = !m.is_empty()
        && m.len() <= MAX_MEMBER_LEN
        && m.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && m.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !valid {
        return Err(CoreError::usage_code(
            "invalid_member_name",
            format!(
                "{source}: member names are 1-{MAX_MEMBER_LEN} characters of a-z, 0-9 and `-` (no `_`), starting with a letter or digit"
            ),
        ));
    }
    Ok(m)
}

/// `PLANE_API_KEY_<MEMBER>`: upper case, `-` becomes `_`.
pub fn member_env_var(member: &str) -> String {
    format!(
        "PLANE_API_KEY_{}",
        member.to_ascii_uppercase().replace('-', "_")
    )
}

/// Keychain service (and label) for a member: `plane-cli-<member>`.
pub fn keychain_service(member: &str) -> String {
    format!("plane-cli-{member}")
}

/// `<config dir>/plane-cli/keys` (next to the global env file).
pub fn keys_dir() -> Option<PathBuf> {
    plane_env_files::global_plane_cli_env_path().and_then(|p| p.parent().map(|d| d.join("keys")))
}

/// What [`resolve_acting_from`] needs to know about `PLANE_CLI_AS` (injectable for tests).
#[derive(Debug, Clone)]
pub struct AsEnv {
    /// Effective non-empty `PLANE_CLI_AS`, if any.
    pub value: Option<String>,
    /// Layer that supplied it: `process_env`, `local`, `global`, `cli_env_file`, … (see
    /// `config where`).
    pub layer: &'static str,
    /// Human label for that layer, e.g. `local file ./.plane-cli.env`.
    pub layer_label: String,
    /// `PLANE_API_KEY` was set in the process environment.
    pub api_key_in_process_env: bool,
}

/// Pick the acting member: `--as` flag, else `PLANE_CLI_AS` from the real environment and env
/// files (see [`resolve_acting_from`] for the rules).
pub fn resolve_acting(
    flag: Option<&str>,
    env_file: Option<&Path>,
    dotenv: bool,
) -> Result<Option<ActingAs>, CoreError> {
    let value = std::env::var(AS_ENV).ok().filter(|v| !v.trim().is_empty());
    let src = plane_env_files::value_source(AS_ENV, env_file, dotenv);
    let layer = if plane_env_files::in_process_env(AS_ENV) {
        "process_env"
    } else {
        src.layer
    };
    resolve_acting_from(
        flag,
        &AsEnv {
            value,
            layer,
            layer_label: src.describe(),
            api_key_in_process_env: plane_env_files::in_process_env("PLANE_API_KEY"),
        },
    )
}

/// A member key is never sent to a URL that came from a file in the current directory: a
/// cloned repository controls that file. Call before reading the key from any source.
pub fn refuse_cwd_url_for_member(url_from_cwd: bool, member: &str) -> Result<(), CoreError> {
    if !url_from_cwd {
        return Ok(());
    }
    Err(CoreError::usage_code(
        "member_key_untrusted_url",
        format!(
            "refusing to send the key for member `{member}`: PLANE_API_BASE_URL comes from a file in the current directory. Set the URL in the global env file, the process environment or --env-file"
        ),
    ))
}

/// Rules, in order:
/// 1. `--as` always wins.
/// 2. `PLANE_CLI_AS` in the process environment is used.
/// 3. `PLANE_CLI_AS` from the per-directory `./.plane-cli.env` is ignored with a warning (a
///    checked-out repo must not pick an identity).
/// 4. `PLANE_CLI_AS` from another env file gives way to `PLANE_API_KEY` in the process
///    environment (CI override), with a warning. This is checked before the name is
///    validated, so a bad value in a file cannot break a CI run.
pub fn resolve_acting_from(flag: Option<&str>, env: &AsEnv) -> Result<Option<ActingAs>, CoreError> {
    if let Some(f) = flag {
        return Ok(Some(ActingAs {
            member: validate_member(f, "--as")?,
            origin: AsOrigin::Flag,
        }));
    }
    let Some(v) = env.value.as_deref() else {
        return Ok(None);
    };
    if env.layer == "process_env" {
        return Ok(Some(ActingAs {
            member: validate_member(v, &format!("{AS_ENV} (process env)"))?,
            origin: AsOrigin::ProcessEnv,
        }));
    }
    if env.layer == "local" {
        plane_core::push_warning(format!(
            "{AS_ENV} in {} was ignored: the acting member is never taken from a per-directory env file; pass --as or set it in the process environment or the global file",
            env.layer_label
        ));
        return Ok(None);
    }
    if env.api_key_in_process_env {
        plane_core::push_warning(format!(
            "{AS_ENV} from {} was ignored: PLANE_API_KEY is set in the process environment and takes precedence (pass --as to override)",
            env.layer_label
        ));
        return Ok(None);
    }
    Ok(Some(ActingAs {
        member: validate_member(v, &format!("{AS_ENV} (from {})", env.layer_label))?,
        origin: AsOrigin::EnvFile,
    }))
}

/// Where a member key was found (no key material).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySource {
    ProcessEnv { var: String },
    Keychain { service: String },
    KeyFile { path: PathBuf },
}

impl KeySource {
    pub fn kind(&self) -> &'static str {
        match self {
            KeySource::ProcessEnv { .. } => "process_env",
            KeySource::Keychain { .. } => "keychain",
            KeySource::KeyFile { .. } => "key_file",
        }
    }

    pub fn describe(&self) -> String {
        match self {
            KeySource::ProcessEnv { var } => format!("process env {var}"),
            KeySource::Keychain { service } => format!("Keychain item {service}"),
            KeySource::KeyFile { path } => format!("key file {}", path.display()),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            KeySource::ProcessEnv { var } => json!({"kind": self.kind(), "var": var}),
            KeySource::Keychain { service } => json!({"kind": self.kind(), "service": service}),
            KeySource::KeyFile { path } => {
                json!({"kind": self.kind(), "path": path.display().to_string()})
            }
        }
    }
}

/// A member's API key and where it came from. `Debug` never prints the key.
pub struct MemberKey {
    pub member: String,
    pub key: String,
    pub source: KeySource,
}

impl std::fmt::Debug for MemberKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemberKey")
            .field("member", &self.member)
            .field("key", &"<redacted>")
            .field("source", &self.source)
            .finish()
    }
}

type EnvLookup = Box<dyn Fn(&str) -> Option<String>>;
type KeychainLookup = Box<dyn Fn(&str) -> Result<Option<String>, CoreError>>;

/// Key sources, injectable so tests never touch the real environment, Keychain or home.
pub struct KeyLookup {
    /// Value of a variable if it was set in the process environment.
    pub process_env: EnvLookup,
    /// Keychain lookup by service name (macOS only in production).
    pub keychain: Option<KeychainLookup>,
    /// Directory of mode-600 key files (non-macOS Unix in production).
    pub keys_dir: Option<PathBuf>,
}

/// Production sources for this platform.
pub fn system_lookup() -> KeyLookup {
    let keychain: Option<KeychainLookup> = if cfg!(target_os = "macos") {
        Some(Box::new(keychain_find))
    } else {
        None
    };
    let keys_dir = if cfg!(all(unix, not(target_os = "macos"))) {
        keys_dir()
    } else {
        None
    };
    KeyLookup {
        process_env: env_lookup(plane_env_files::in_process_env),
        keychain,
        keys_dir,
    }
}

/// Reads a variable only if `in_process_env` says it was set in the process environment; a
/// value that came from an env file is ignored with a warning.
pub fn env_lookup(in_process_env: fn(&str) -> bool) -> EnvLookup {
    Box::new(move |name| {
        let v = std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        if in_process_env(name) {
            v
        } else {
            if v.is_some() {
                plane_core::push_warning(format!(
                    "{name} is set in an env file and was ignored: member keys are read only from the process environment, the Keychain (macOS) or the key file"
                ));
            }
            None
        }
    })
}

/// Find `member`'s key in `lookup`'s sources, in order.
pub fn load_member_key(member: &str, lookup: &KeyLookup) -> Result<MemberKey, CoreError> {
    let var = member_env_var(member);
    if let Some(v) = (lookup.process_env)(&var) {
        return Ok(MemberKey {
            member: member.into(),
            key: v.trim().to_string(),
            source: KeySource::ProcessEnv { var },
        });
    }
    let mut tried = vec![format!("process env {var}")];
    if let Some(kc) = &lookup.keychain {
        let service = keychain_service(member);
        if let Some(v) = kc(&service)? {
            return Ok(MemberKey {
                member: member.into(),
                key: v,
                source: KeySource::Keychain { service },
            });
        }
        tried.push(format!("Keychain item {service}"));
    }
    if let Some(dir) = &lookup.keys_dir {
        let path = dir.join(member);
        let found = if check_keys_dir(dir)? {
            read_key_file(&path)?
        } else {
            None
        };
        if let Some(v) = found {
            return Ok(MemberKey {
                member: member.into(),
                key: v,
                source: KeySource::KeyFile { path },
            });
        }
        tried.push(format!("key file {}", path.display()));
    }
    Err(CoreError::usage_code(
        "member_key_not_found",
        format!(
            "no API key for member `{member}`; looked in: {}. Store one with `plane-cli config set-key --member {member}` (see https://pseudovar.github.io/plane-cli/agent-identity.html)",
            tried.join(", ")
        ),
    ))
}

fn insecure(path: &Path, why: &str) -> CoreError {
    CoreError::usage_code(
        "member_key_file_insecure",
        format!(
            "refusing key file {}: {why}. It must be a regular file owned by you with mode 600 (`chmod 600 {}`)",
            path.display(),
            path.display()
        ),
    )
}

fn insecure_dir(dir: &Path, why: &str) -> CoreError {
    CoreError::usage_code(
        "member_key_dir_insecure",
        format!(
            "refusing key directory {}: {why}. It must be a real directory owned by you with mode 700 (`chmod 700 {}`)",
            dir.display(),
            dir.display()
        ),
    )
}

#[cfg(unix)]
fn euid() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

/// The keys directory must not be a symlink, must be owned by the current user and must not be
/// writable by group or others (someone else could swap a key in). `Ok(false)` if it does not
/// exist.
#[cfg(unix)]
pub fn check_keys_dir(dir: &Path) -> Result<bool, CoreError> {
    use std::os::unix::fs::MetadataExt;
    let meta = match std::fs::symlink_metadata(dir) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(insecure_dir(dir, &format!("cannot inspect it ({e})"))),
    };
    if meta.file_type().is_symlink() {
        return Err(insecure_dir(dir, "it is a symlink"));
    }
    if !meta.is_dir() {
        return Err(insecure_dir(dir, "it is not a directory"));
    }
    if meta.uid() != euid() {
        return Err(insecure_dir(
            dir,
            &format!(
                "it is owned by uid {}, not by you (uid {})",
                meta.uid(),
                euid()
            ),
        ));
    }
    let mode = meta.mode() & 0o777;
    if mode & 0o022 != 0 {
        return Err(insecure_dir(
            dir,
            &format!("its mode is {mode:03o} (writable by group or others)"),
        ));
    }
    Ok(true)
}

#[cfg(not(unix))]
pub fn check_keys_dir(dir: &Path) -> Result<bool, CoreError> {
    Ok(dir.is_dir())
}

/// Read a key file: `Ok(None)` if it does not exist; refused if it is a symlink, not a regular
/// file (a FIFO cannot block: it is opened non-blocking), not owned by the current user, or
/// readable/writable by group or others.
#[cfg(unix)]
pub fn read_key_file(path: &Path) -> Result<Option<String>, CoreError> {
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

    let mut file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => {
            return Err(insecure(path, "it is a symlink"))
        }
        Err(e) => {
            return Err(CoreError::usage_code(
                "member_key_file_unreadable",
                format!("could not open key file {}: {e}", path.display()),
            ))
        }
    };
    // Checks use the opened file, so a swap between check and read is not possible.
    let meta = file.metadata()?;
    if !meta.file_type().is_file() {
        return Err(insecure(path, "it is not a regular file"));
    }
    let uid = euid();
    if meta.uid() != uid {
        return Err(insecure(
            path,
            &format!("it is owned by uid {}, not by you (uid {uid})", meta.uid()),
        ));
    }
    let mode = meta.mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(insecure(path, &format!("its mode is {mode:03o}")));
    }
    let mut raw = String::new();
    file.read_to_string(&mut raw).map_err(|e| {
        CoreError::usage_code(
            "member_key_file_unreadable",
            format!("could not read key file {}: {e}", path.display()),
        )
    })?;
    let key = raw.trim().to_string();
    if key.is_empty() {
        return Err(CoreError::usage_code(
            "member_key_not_found",
            format!("key file {} is empty", path.display()),
        ));
    }
    Ok(Some(key))
}

#[cfg(not(unix))]
pub fn read_key_file(_path: &Path) -> Result<Option<String>, CoreError> {
    Ok(None)
}

/// Write `key` to `<dir>/<member>` with mode 600 (directory 700), atomically via a temp file.
#[cfg(unix)]
pub fn write_key_file(dir: &Path, member: &str, key: &str) -> Result<PathBuf, CoreError> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};

    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    // Never follow or chmod a directory someone else controls; then make our own one 700.
    let meta = std::fs::symlink_metadata(dir)?;
    if meta.file_type().is_symlink() {
        return Err(insecure_dir(dir, "it is a symlink"));
    }
    if meta.uid() != euid() {
        return Err(insecure_dir(dir, "it is not owned by you"));
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    check_keys_dir(dir)?;
    let path = dir.join(member);
    let tmp = dir.join(format!(".{member}.tmp-{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)?;
        f.write_all(key.as_bytes())?;
        f.write_all(b"\n")?;
        f.sync_all()?;
    }
    // `create_new` + `mode` respects the umask; make sure it is exactly 600.
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(path)
}

#[cfg(not(unix))]
pub fn write_key_file(_dir: &Path, _member: &str, _key: &str) -> Result<PathBuf, CoreError> {
    Err(CoreError::usage_code(
        "member_key_store_unsupported",
        "storing member keys is supported on macOS (Keychain) and Unix (key file) only; set PLANE_API_KEY_<MEMBER> in the process environment instead",
    ))
}

/// Check a pasted key before storing it. Never echoes the key.
pub fn validate_key(key: &str) -> Result<(), CoreError> {
    let ok = (8..=512).contains(&key.len())
        && key
            .chars()
            .all(|c| c.is_ascii_graphic() && !matches!(c, '"' | '\'' | '\\'));
    if !ok {
        return Err(CoreError::usage_code(
            "invalid_api_key",
            "the API key must be 8-512 printable ASCII characters with no spaces or quotes (Plane personal access tokens look like `plane_api_…`); nothing was stored",
        ));
    }
    Ok(())
}

// ---- Keychain (macOS `security` CLI). The command builders and output parsing compile and are
// tested everywhere; only the call sites run on macOS (`cfg!(target_os = "macos")`).

/// Arguments for `security find-generic-password -s <service> -w`.
pub fn keychain_find_args(service: &str) -> Vec<String> {
    vec![
        "find-generic-password".into(),
        "-s".into(),
        service.into(),
        "-w".into(),
    ]
}

/// `security` exits 44 when the item is not found.
const SECURITY_ITEM_NOT_FOUND: i32 = 44;

/// Interpret `security find-generic-password -w`: the password on stdout (exit 0), not found
/// (exit 44), or an error that carries only stderr (never stdout, which may hold the key).
pub fn parse_keychain_find(
    service: &str,
    code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<Option<String>, CoreError> {
    match code {
        Some(0) => {
            let key = String::from_utf8_lossy(stdout).trim().to_string();
            Ok((!key.is_empty()).then_some(key))
        }
        Some(SECURITY_ITEM_NOT_FOUND) => Ok(None),
        _ => {
            let err = String::from_utf8_lossy(stderr);
            let first = err.lines().next().unwrap_or("").trim();
            Err(CoreError::usage_code(
                "keychain_error",
                format!(
                    "could not read Keychain item {service} (security exit {}): {first}",
                    code.map(|c| c.to_string())
                        .unwrap_or_else(|| "signal".into())
                ),
            ))
        }
    }
}

/// Script for `security -i` (read from stdin) that adds or updates the item, so the key is
/// never on a command line (`ps` would show it). The key is checked by [`validate_key`], so it
/// has no quotes or spaces to escape.
pub fn keychain_add_script(member: &str, key: &str) -> Result<String, CoreError> {
    validate_key(key)?;
    let service = keychain_service(member);
    Ok(format!(
        "add-generic-password -U -a \"{member}\" -s \"{service}\" -l \"{service}\" -w \"{key}\"\n"
    ))
}

fn keychain_find(service: &str) -> Result<Option<String>, CoreError> {
    let out = std::process::Command::new("security")
        .args(keychain_find_args(service))
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| {
            CoreError::usage_code(
                "keychain_error",
                format!("could not run `security` to read Keychain item {service}: {e}"),
            )
        })?;
    parse_keychain_find(service, out.status.code(), &out.stdout, &out.stderr)
}

/// Add or update the member's Keychain item, then read it back to confirm.
pub fn keychain_store(member: &str, key: &str) -> Result<String, CoreError> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let script = keychain_add_script(member, key)?;
    let service = keychain_service(member);
    let fail = |why: String| {
        CoreError::usage_code(
            "keychain_error",
            format!("could not store Keychain item {service}: {why}"),
        )
    };
    let mut child = Command::new("security")
        .arg("-i")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| fail(format!("could not run `security`: {e}")))?;
    {
        let mut stdin = child.stdin.take().ok_or_else(|| fail("no stdin".into()))?;
        stdin
            .write_all(script.as_bytes())
            .map_err(|e| fail(e.to_string()))?;
    }
    // stdout is discarded unread: interactive mode prints prompts only, but never risk it.
    let out = child.wait_with_output().map_err(|e| fail(e.to_string()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(fail(err.lines().next().unwrap_or("").trim().to_string()));
    }
    match keychain_find(&service)? {
        Some(stored) if stored == key => Ok(service),
        _ => Err(fail(
            "the item could not be read back after writing it".into(),
        )),
    }
}

// ---- users/me check

/// Short fingerprint of a key for the identity cache (the key itself is never stored).
pub fn key_fingerprint(key: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"plane-cli identity v1\0");
    h.update(key.as_bytes());
    h.finalize()
        .iter()
        .take(12)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Does this `users/me` body belong to `member`? The `display_name` must equal the member
/// (ignoring case; member names are lower case). Only when `display_name` is empty or missing
/// does the local part of `email` count.
pub fn user_matches_member(member: &str, me: &Value) -> bool {
    let display = me
        .get("display_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|d| !d.is_empty());
    match display {
        Some(d) => d.eq_ignore_ascii_case(member),
        None => me
            .get("email")
            .and_then(Value::as_str)
            .and_then(|e| e.split('@').next())
            .is_some_and(|local| local.trim().eq_ignore_ascii_case(member)),
    }
}

fn cache_kind(member: &str) -> String {
    format!("identity_{member}")
}

/// Trust-on-first-use pin: the Plane user id first seen for a member. Kept apart from the
/// 24-hour check so a new key does not wipe it; `config set-key` resets it.
fn pin_kind(member: &str) -> String {
    format!("identity_pin_{member}")
}

/// Pins are effectively permanent (the cache needs a TTL).
const PIN_TTL_SECS: u64 = 10 * 365 * 24 * 60 * 60;

/// Forget the cached check and the user-id pin for `member` in `cache` (used by
/// `config set-key`, the documented way to accept a member's new Plane user).
pub fn reset_identity(cache: &plane_core::Cache, member: &str) {
    let _ = cache.remove(&cache_kind(member));
    let _ = cache.remove(&pin_kind(member));
}

/// Check that `mk.key` belongs to `mk.member` (one `users/me` call, cached for 24 hours per key
/// fingerprint; `refresh` skips the cache). Refuses a mismatch with `member_identity_mismatch`
/// and a rejected key with `member_key_rejected`. Returns a summary with no key material.
pub fn verify_identity(api: &PlaneApi, mk: &MemberKey, refresh: bool) -> Result<Value, CoreError> {
    let _ = CURRENT_MEMBER.set(mk.member.clone());
    let fp = key_fingerprint(&mk.key);
    let kind = cache_kind(&mk.member);
    if let Ok(Some(cached)) = api.cache().get(&kind, refresh) {
        if cached.get("key_fingerprint").and_then(Value::as_str) == Some(fp.as_str()) {
            return Ok(json!({
                "member": mk.member,
                "user_id": cached.get("user_id"),
                "display_name": cached.get("display_name"),
                "key_source": mk.source.to_json(),
                "cached": true,
            }));
        }
    }
    let me = match api.whoami() {
        Ok(v) => v,
        Err(CoreError::Http { status, .. }) if status == 401 || status == 403 => {
            let _ = api.cache().remove(&kind);
            return Err(CoreError::usage_code(
                "member_key_rejected",
                format!(
                    "Plane rejected the API key for member `{}` (from {}) with HTTP {status}; create a new personal access token as that member and store it with `plane-cli config set-key --member {}`",
                    mk.member,
                    mk.source.describe(),
                    mk.member
                ),
            ));
        }
        Err(e) => return Err(e),
    };
    if !user_matches_member(&mk.member, &me) {
        let _ = api.cache().remove(&kind);
        let actual = me
            .get("display_name")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        return Err(CoreError::usage_code(
            "member_identity_mismatch",
            format!(
                "the API key for member `{}` (from {}) belongs to Plane user `{actual}`; refusing to act under another member's name. Store `{}`'s own key with `plane-cli config set-key --member {}`",
                mk.member,
                mk.source.describe(),
                mk.member,
                mk.member
            ),
        ));
    }
    let user_id = me.get("id").cloned().unwrap_or(Value::Null);
    let display = me.get("display_name").cloned().unwrap_or(Value::Null);
    let pin = pin_kind(&mk.member);
    // The pin is read even with --refresh: it is not a cache of Plane's answer.
    match api.cache().get(&pin, false).ok().flatten() {
        Some(p) if p.get("user_id") != Some(&user_id) => {
            let _ = api.cache().remove(&kind);
            return Err(CoreError::usage_code(
                "member_identity_changed",
                format!(
                    "the API key for member `{}` (from {}) belongs to a different Plane user than the one first seen for `{}`; refusing. If the member's account was replaced on purpose, store its key again with `plane-cli config set-key --member {}` (that resets the pin)",
                    mk.member,
                    mk.source.describe(),
                    mk.member,
                    mk.member
                ),
            ));
        }
        Some(_) => {}
        None => {
            let _ = api
                .cache()
                .put_with_ttl(&pin, json!({"user_id": user_id}), PIN_TTL_SECS);
        }
    }
    let _ = api.cache().put_with_ttl(
        &kind,
        json!({"key_fingerprint": fp, "user_id": user_id, "display_name": display}),
        IDENTITY_CACHE_TTL_SECS,
    );
    Ok(json!({
        "member": mk.member,
        "user_id": user_id,
        "display_name": display,
        "key_source": mk.source.to_json(),
        "cached": false,
    }))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::HashMap;

    pub(crate) fn lookup_with(
        env: &[(&str, &str)],
        keychain: Option<HashMap<String, String>>,
        keys_dir: Option<PathBuf>,
    ) -> KeyLookup {
        let env: HashMap<String, String> = env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        KeyLookup {
            process_env: Box::new(move |n| env.get(n).cloned()),
            keychain: keychain
                .map(|m| -> KeychainLookup { Box::new(move |s: &str| Ok(m.get(s).cloned())) }),
            keys_dir,
        }
    }

    #[cfg(unix)]
    fn write_mode(path: &Path, body: &str, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn member_names_are_normalised_and_checked() {
        assert_eq!(validate_member(" Bob ", "--as").unwrap(), "bob");
        assert_eq!(validate_member("ci-bot-2", "--as").unwrap(), "ci-bot-2");
        for bad in [
            "",
            "-x",
            "a b",
            "../etc",
            "x/y",
            "ci_bot_2",
            &"a".repeat(33),
        ] {
            let e = validate_member(bad, "--as").unwrap_err();
            assert_eq!(e.error_code(), "invalid_member_name", "{bad:?}");
            assert!(e.to_string().contains("--as"), "names the source: {e}");
        }
    }

    fn as_env(value: Option<&str>, layer: &'static str, key_in_process: bool) -> AsEnv {
        AsEnv {
            value: value.map(String::from),
            layer,
            layer_label: format!("{layer} file /x/.plane-cli.env"),
            api_key_in_process_env: key_in_process,
        }
    }

    #[test]
    fn resolve_acting_precedence() {
        let _g = crate::plane_env_files::lock_env();
        let _ = plane_core::take_warnings();
        // --as beats everything, including PLANE_API_KEY in the process env.
        let a = resolve_acting_from(Some("Bob"), &as_env(Some("ci-bot"), "process_env", true))
            .unwrap()
            .unwrap();
        assert_eq!((a.member.as_str(), a.origin), ("bob", AsOrigin::Flag));
        // Process-env PLANE_CLI_AS beats a process-env PLANE_API_KEY.
        let a = resolve_acting_from(None, &as_env(Some("ci-bot"), "process_env", true))
            .unwrap()
            .unwrap();
        assert_eq!(a.origin, AsOrigin::ProcessEnv);
        // Global-file PLANE_CLI_AS is used when PLANE_API_KEY is not in the process env.
        let a = resolve_acting_from(None, &as_env(Some("ci-bot"), "global", false))
            .unwrap()
            .unwrap();
        assert_eq!(a.origin, AsOrigin::EnvFile);
        assert!(plane_core::take_warnings().is_empty());
        // ...and gives way to it when it is, before validation (a bad value cannot break CI).
        let r = resolve_acting_from(None, &as_env(Some("not valid!"), "global", true)).unwrap();
        assert!(r.is_none());
        let w = plane_core::take_warnings();
        assert!(
            w[0].contains("PLANE_API_KEY is set in the process environment"),
            "{w:?}"
        );
        assert!(!w[0].contains("not valid!"));
        // The per-directory file never picks the identity.
        let r = resolve_acting_from(None, &as_env(Some("ci-bot"), "local", false)).unwrap();
        assert!(r.is_none());
        assert!(plane_core::take_warnings()[0].contains("per-directory"));
        // A bad name names its source.
        let e = resolve_acting_from(None, &as_env(Some("a_b"), "global", false)).unwrap_err();
        assert!(
            e.to_string().contains("PLANE_CLI_AS (from global file"),
            "{e}"
        );
        assert!(resolve_acting_from(None, &as_env(None, "unset", true))
            .unwrap()
            .is_none());
    }

    #[test]
    fn member_key_from_an_env_file_is_rejected_with_a_warning() {
        let _g = crate::plane_env_files::lock_env();
        let _ = plane_core::take_warnings();
        std::env::set_var("PLANE_API_KEY_ENVFILEBOT", "plane_api_from_env_file");
        fn never(_: &str) -> bool {
            false
        }
        fn always(_: &str) -> bool {
            true
        }
        let file_lookup = KeyLookup {
            process_env: env_lookup(never),
            keychain: None,
            keys_dir: None,
        };
        let e = load_member_key("envfilebot", &file_lookup).unwrap_err();
        assert_eq!(e.error_code(), "member_key_not_found");
        let w = plane_core::take_warnings();
        assert!(
            w.iter()
                .any(|w| w.contains("PLANE_API_KEY_ENVFILEBOT is set in an env file")),
            "{w:?}"
        );
        assert!(!w.iter().any(|w| w.contains("plane_api_from_env_file")));
        // The same variable set in the process env is used.
        let proc_lookup = KeyLookup {
            process_env: env_lookup(always),
            keychain: None,
            keys_dir: None,
        };
        assert_eq!(
            load_member_key("envfilebot", &proc_lookup).unwrap().key,
            "plane_api_from_env_file"
        );
        std::env::remove_var("PLANE_API_KEY_ENVFILEBOT");
    }

    #[test]
    fn a_pasted_key_is_not_echoed_as_a_member_name() {
        let key = "plane_api_0123456789abcdef0123456789abcdef";
        let e = validate_member(key, "--as").unwrap_err();
        assert_eq!(e.error_code(), "invalid_member_name");
        assert!(!e.to_string().contains(key));
        assert!(!e.to_string().contains("0123456789abcdef"));
    }

    #[test]
    fn env_var_and_keychain_names() {
        assert_eq!(member_env_var("bob"), "PLANE_API_KEY_BOB");
        assert_eq!(member_env_var("ci-bot-2"), "PLANE_API_KEY_CI_BOT_2");
        assert_eq!(keychain_service("bob"), "plane-cli-bob");
        assert_eq!(
            keychain_find_args("plane-cli-bob"),
            vec!["find-generic-password", "-s", "plane-cli-bob", "-w"]
        );
    }

    #[test]
    fn process_env_wins_over_keychain_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let kc = HashMap::from([("plane-cli-bob".to_string(), "kc-key".to_string())]);
        let lookup = lookup_with(
            &[("PLANE_API_KEY_BOB", "env-key")],
            Some(kc),
            Some(dir.path().to_path_buf()),
        );
        let mk = load_member_key("bob", &lookup).unwrap();
        assert_eq!(mk.key, "env-key");
        assert_eq!(mk.source.kind(), "process_env");
        // Another member's variable is never used.
        let mk = load_member_key(
            "bob",
            &lookup_with(&[("PLANE_API_KEY_CI_BOT", "x")], None, None),
        );
        assert_eq!(mk.unwrap_err().error_code(), "member_key_not_found");
    }

    #[test]
    fn keychain_is_used_before_the_key_file() {
        let kc = HashMap::from([("plane-cli-bob".to_string(), "kc-key".to_string())]);
        let mk = load_member_key("bob", &lookup_with(&[], Some(kc), None)).unwrap();
        assert_eq!(mk.key, "kc-key");
        assert_eq!(
            mk.source,
            KeySource::Keychain {
                service: "plane-cli-bob".into()
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn key_file_with_mode_600_is_read_and_trimmed() {
        let dir = tempfile::tempdir().unwrap();
        write_mode(&dir.path().join("bob"), "file-key\n", 0o600);
        let mk = load_member_key("bob", &lookup_with(&[], None, Some(dir.path().into()))).unwrap();
        assert_eq!(mk.key, "file-key");
        assert_eq!(mk.source.kind(), "key_file");
    }

    #[cfg(unix)]
    #[test]
    fn world_or_group_readable_key_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        for mode in [0o644, 0o640, 0o604, 0o660, 0o666] {
            let p = dir.path().join("bob");
            write_mode(&p, "secret-file-key", mode);
            let e = read_key_file(&p).unwrap_err();
            assert_eq!(e.error_code(), "member_key_file_insecure", "{mode:o}");
            assert!(e.to_string().contains(&format!("{mode:03o}")), "{e}");
            assert!(!e.to_string().contains("secret-file-key"));
        }
        write_mode(&dir.path().join("bob"), "k", 0o400);
        assert_eq!(
            read_key_file(&dir.path().join("bob")).unwrap().as_deref(),
            Some("k")
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_key_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("elsewhere");
        write_mode(&target, "k", 0o600);
        let link = dir.path().join("bob");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(
            read_key_file(&link).unwrap_err().error_code(),
            "member_key_file_insecure"
        );
    }

    #[cfg(unix)]
    #[test]
    fn missing_and_empty_key_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_key_file(&dir.path().join("nobody")).unwrap().is_none());
        write_mode(&dir.path().join("empty"), "\n", 0o600);
        assert_eq!(
            read_key_file(&dir.path().join("empty"))
                .unwrap_err()
                .error_code(),
            "member_key_not_found"
        );
        let e =
            load_member_key("bob", &lookup_with(&[], None, Some(dir.path().into()))).unwrap_err();
        assert_eq!(e.error_code(), "member_key_not_found");
        assert!(e.to_string().contains("PLANE_API_KEY_BOB"));
        assert!(e.to_string().contains("config set-key --member bob"));
    }

    #[cfg(unix)]
    #[test]
    fn write_key_file_sets_modes_and_round_trips() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let keys = dir.path().join("plane-cli").join("keys");
        let p = write_key_file(&keys, "bob", "plane_api_test_round_trip").unwrap();
        assert_eq!(p, keys.join("bob"));
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&p), 0o600);
        assert_eq!(mode(&keys), 0o700);
        assert_eq!(
            read_key_file(&p).unwrap().as_deref(),
            Some("plane_api_test_round_trip")
        );
        // Overwrite fixes a loosened mode and replaces the value.
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_key_file(&keys, "bob", "plane_api_second").unwrap();
        assert_eq!(mode(&p), 0o600);
        assert_eq!(
            read_key_file(&p).unwrap().as_deref(),
            Some("plane_api_second")
        );
        // No temp files left behind.
        assert_eq!(std::fs::read_dir(&keys).unwrap().count(), 1);
    }

    #[test]
    fn keychain_output_parsing() {
        let s = "plane-cli-bob";
        assert_eq!(
            parse_keychain_find(s, Some(0), b"plane_api_x\n", b"").unwrap(),
            Some("plane_api_x".into())
        );
        assert_eq!(
            parse_keychain_find(s, Some(44), b"", b"not found").unwrap(),
            None
        );
        let e = parse_keychain_find(s, Some(51), b"plane_api_leak", b"security: denied\nmore")
            .unwrap_err();
        assert_eq!(e.error_code(), "keychain_error");
        assert!(e.to_string().contains("security: denied"));
        assert!(!e.to_string().contains("plane_api_leak"));
    }

    #[test]
    fn keychain_script_keeps_the_key_off_argv_and_rejects_quotes() {
        let script = keychain_add_script("bob", "plane_api_abc123").unwrap();
        assert_eq!(
            script,
            "add-generic-password -U -a \"bob\" -s \"plane-cli-bob\" -l \"plane-cli-bob\" -w \"plane_api_abc123\"\n"
        );
        for bad in ["has space key", "quote\"key1", "back\\slash1", "short"] {
            let e = keychain_add_script("bob", bad).unwrap_err();
            assert_eq!(e.error_code(), "invalid_api_key");
            assert!(!e.to_string().contains(bad));
        }
    }

    #[test]
    fn users_me_matching() {
        assert!(user_matches_member("bob", &json!({"display_name": "Bob"})));
        // A non-empty display_name decides; the email is not a fallback then.
        assert!(!user_matches_member(
            "bob",
            &json!({"display_name": "Bob Bot", "email": "bob@example.com"})
        ));
        assert!(!user_matches_member(
            "bob",
            &json!({"display_name": "alice", "email": "bob@example.com"})
        ));
        // Empty or missing display_name: the email local part counts.
        assert!(user_matches_member(
            "bob",
            &json!({"display_name": " ", "email": "bob@example.com"})
        ));
        assert!(user_matches_member(
            "bob",
            &json!({"email": "Bob@example.com"})
        ));
        assert!(!user_matches_member("bob", &json!({})));
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_key_file_does_not_hang() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bob");
        let c = std::ffi::CString::new(p.as_os_str().to_str().unwrap()).unwrap();
        // SAFETY: valid NUL-terminated path; mkfifo has no other preconditions.
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let e = read_key_file(&p).unwrap_err();
        assert_eq!(e.error_code(), "member_key_file_insecure");
        assert!(e.to_string().contains("not a regular file"), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn insecure_keys_dirs_are_refused() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("real");
        std::fs::create_dir(&real).unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o700)).unwrap();
        write_mode(&real.join("bob"), "k-in-dir", 0o600);
        assert!(check_keys_dir(&real).unwrap());
        assert!(!check_keys_dir(&root.path().join("missing")).unwrap());

        // A symlinked keys dir: refused on read and on write.
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let e = load_member_key("bob", &lookup_with(&[], None, Some(link.clone()))).unwrap_err();
        assert_eq!(e.error_code(), "member_key_dir_insecure");
        assert_eq!(
            write_key_file(&link, "bob", "plane_api_x123")
                .unwrap_err()
                .error_code(),
            "member_key_dir_insecure"
        );

        // Group- or world-writable: refused on read.
        for mode in [0o770, 0o702, 0o777] {
            std::fs::set_permissions(&real, std::fs::Permissions::from_mode(mode)).unwrap();
            let e =
                load_member_key("bob", &lookup_with(&[], None, Some(real.clone()))).unwrap_err();
            assert_eq!(e.error_code(), "member_key_dir_insecure", "{mode:o}");
        }
        // Writing fixes our own directory back to 700.
        write_key_file(&real, "bob", "plane_api_fixed1").unwrap();
        assert_eq!(
            std::fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }

    #[test]
    fn fingerprint_is_stable_and_not_the_key() {
        let key = "plane_api_fingerprint_test";
        assert_eq!(key_fingerprint(key), key_fingerprint(key));
        assert_ne!(key_fingerprint(key), key_fingerprint("plane_api_other"));
        assert_eq!(key_fingerprint(key).len(), 24);
        assert!(!key_fingerprint(key).contains("plane_api"));
    }

    /// Points the cache at a temp dir (restored on drop) and builds a `PlaneApi` for `base`
    /// acting as `member`. Hold `plane_env_files::lock_env()` while it lives.
    pub(crate) struct MockApi {
        pub api: PlaneApi,
        _cache: tempfile::TempDir,
        old_cache: Option<std::ffi::OsString>,
    }

    impl MockApi {
        pub(crate) fn new(base: &str, member: &str, key: &str) -> Self {
            let cache = tempfile::tempdir().unwrap();
            let old_cache = std::env::var_os("XDG_CACHE_HOME");
            std::env::set_var("XDG_CACHE_HOME", cache.path());
            let config = plane_core::Config {
                api_base: url::Url::parse(base).unwrap(),
                api_key: key.into(),
                workspace_slug: "ws".into(),
                cache_enabled: true,
                json_output: true,
                acting_as: Some(member.into()),
            };
            MockApi {
                api: PlaneApi::new(config).unwrap(),
                _cache: cache,
                old_cache,
            }
        }
    }

    impl MockApi {
        /// Same instance, member and cache directory, but sending `key`.
        pub(crate) fn with_key(&self, key: &str) -> PlaneApi {
            let mut config = self.api.config.clone();
            config.api_key = key.into();
            PlaneApi::new(config).unwrap()
        }
    }

    impl Drop for MockApi {
        fn drop(&mut self) {
            match &self.old_cache {
                Some(v) => std::env::set_var("XDG_CACHE_HOME", v),
                None => std::env::remove_var("XDG_CACHE_HOME"),
            }
        }
    }

    pub(crate) fn env_key(member: &str, key: &str) -> MemberKey {
        MemberKey {
            member: member.into(),
            key: key.into(),
            source: KeySource::ProcessEnv {
                var: member_env_var(member),
            },
        }
    }

    #[test]
    fn verify_sends_the_member_key_and_caches_the_check() {
        let _g = crate::plane_env_files::lock_env();
        let key = "plane_api_bob_mock_key";
        let mut server = mockito::Server::new();
        let me = server
            .mock("GET", "/api/v1/users/me/")
            .match_header("x-api-key", key)
            .with_status(200)
            .with_body(r#"{"id":"u-bob","display_name":"bob","email":"bob@example.com"}"#)
            .expect(1)
            .create();
        let m = MockApi::new(&server.url(), "bob", key);
        let mk = env_key("bob", key);
        let first = verify_identity(&m.api, &mk, false).unwrap();
        assert_eq!(first["cached"], json!(false));
        assert_eq!(first["user_id"], json!("u-bob"));
        let second = verify_identity(&m.api, &mk, false).unwrap();
        assert_eq!(second["cached"], json!(true));
        me.assert(); // exactly one users/me call
        let cached = m.api.cache().get("identity_bob", false).unwrap().unwrap();
        assert!(!cached.to_string().contains(key), "key stored in cache");
    }

    #[test]
    fn a_changed_key_or_refresh_rechecks() {
        let _g = crate::plane_env_files::lock_env();
        let mut server = mockito::Server::new();
        let me = server
            .mock("GET", "/api/v1/users/me/")
            .with_status(200)
            .with_body(r#"{"id":"u-bob","display_name":"bob"}"#)
            .expect(3)
            .create();
        let m = MockApi::new(&server.url(), "bob", "plane_api_one");
        verify_identity(&m.api, &env_key("bob", "plane_api_one"), false).unwrap();
        verify_identity(&m.api, &env_key("bob", "plane_api_two"), false).unwrap();
        verify_identity(&m.api, &env_key("bob", "plane_api_two"), true).unwrap();
        me.assert();
    }

    #[test]
    fn a_key_for_another_user_is_refused() {
        let _g = crate::plane_env_files::lock_env();
        let key = "plane_api_actually_alices_key";
        let mut server = mockito::Server::new();
        let _me = server
            .mock("GET", "/api/v1/users/me/")
            .with_status(200)
            .with_body(r#"{"id":"u-alice","display_name":"alice","email":"alice@example.com"}"#)
            .create();
        let m = MockApi::new(&server.url(), "bob", key);
        let e = verify_identity(&m.api, &env_key("bob", key), false).unwrap_err();
        assert_eq!(e.error_code(), "member_identity_mismatch");
        assert_eq!(e.exit_kind().code(), 2);
        let msg = e.to_string();
        assert!(
            msg.contains("`alice`") && msg.contains("PLANE_API_KEY_BOB"),
            "{msg}"
        );
        assert!(!msg.contains(key));
        assert!(m.api.cache().get("identity_bob", false).unwrap().is_none());
    }

    #[test]
    fn a_different_user_for_the_same_member_is_refused_until_reset() {
        let _g = crate::plane_env_files::lock_env();
        let mut server = mockito::Server::new();
        let _first = server
            .mock("GET", "/api/v1/users/me/")
            .match_header("x-api-key", "plane_api_first")
            .with_status(200)
            .with_body(r#"{"id":"u-bob-1","display_name":"bob"}"#)
            .create();
        let _second = server
            .mock("GET", "/api/v1/users/me/")
            .match_header("x-api-key", "plane_api_second")
            .with_status(200)
            .with_body(r#"{"id":"u-bob-2","display_name":"bob"}"#)
            .create();
        let m = MockApi::new(&server.url(), "bob", "plane_api_first");
        let second = m.with_key("plane_api_second");
        verify_identity(&m.api, &env_key("bob", "plane_api_first"), false).unwrap();
        // A new key whose user is also called bob, but is a different account.
        let e = verify_identity(&second, &env_key("bob", "plane_api_second"), false).unwrap_err();
        assert_eq!(e.error_code(), "member_identity_changed");
        assert!(e.to_string().contains("config set-key --member bob"), "{e}");
        // --refresh does not bypass the pin.
        let e = verify_identity(&second, &env_key("bob", "plane_api_second"), true).unwrap_err();
        assert_eq!(e.error_code(), "member_identity_changed");
        // Resetting (what `config set-key` does) accepts the new account and pins it.
        reset_identity(m.api.cache(), "bob");
        let ok = verify_identity(&second, &env_key("bob", "plane_api_second"), false).unwrap();
        assert_eq!(ok["user_id"], json!("u-bob-2"));
        assert_eq!(
            verify_identity(&m.api, &env_key("bob", "plane_api_first"), true)
                .unwrap_err()
                .error_code(),
            "member_identity_changed"
        );
    }

    #[test]
    fn a_rejected_key_gets_its_own_code() {
        let _g = crate::plane_env_files::lock_env();
        let key = "plane_api_revoked_key";
        let mut server = mockito::Server::new();
        let _me = server
            .mock("GET", "/api/v1/users/me/")
            .with_status(401)
            .with_body(format!(r#"{{"detail":"Invalid token {key}"}}"#))
            .create();
        let m = MockApi::new(&server.url(), "bob", key);
        let e = verify_identity(&m.api, &env_key("bob", key), false).unwrap_err();
        assert_eq!(e.error_code(), "member_key_rejected");
        assert!(!e.to_string().contains(key));
        // The server body (which echoed the key) is not carried into the envelope.
        let env =
            serde_json::to_string(&plane_core::ResponseEnvelope::from_core_error(&e)).unwrap();
        assert!(!env.contains(key), "{env}");
    }

    #[test]
    fn member_key_debug_redacts() {
        let mk = MemberKey {
            member: "bob".into(),
            key: "plane_api_debug_secret".into(),
            source: KeySource::ProcessEnv {
                var: "PLANE_API_KEY_BOB".into(),
            },
        };
        assert!(!format!("{mk:?}").contains("plane_api_debug_secret"));
    }
}
