//! GitHub release checks and installer entrypoints (`plane-cli self …`).
//! Uses the public GitHub API and release URLs; no GitHub CLI or login needed.

use crate::dispatch::{Cli, SelfManageCmd, SelfSkillsCmd};
use crate::skill_cmd;
use plane_core::{print_json, CoreError, ResponseEnvelope};
use semver::Version;
use serde::Deserialize;
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

// These URLs are constants on purpose: `self update --apply` runs what it downloads, so no
// environment variable or env file may redirect it.
const RELEASES_API: &str = "https://api.github.com/repos/pseudovar/plane-cli/releases?per_page=40";
const RAW_BASE: &str = "https://raw.githubusercontent.com/pseudovar/plane-cli";
const INSTALL_ONE_LINER: &str =
    "curl -fsSL https://raw.githubusercontent.com/pseudovar/plane-cli/main/scripts/install.sh | bash";

const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
/// The TUI's background hint must never hold anything up.
const HINT_TIMEOUT: Duration = Duration::from_secs(3);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct ReleaseRow {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

fn http_client(timeout: Duration) -> Result<reqwest::blocking::Client, CoreError> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("plane-cli/", env!("CARGO_PKG_VERSION")))
        .timeout(timeout)
        // Tests talk to a local mock server over plain HTTP.
        .https_only(cfg!(not(test)))
        .build()
        .map_err(|e| CoreError::Usage(format!("could not start HTTP client: {e}")))
}

fn pick_stable(rows: Vec<ReleaseRow>) -> Option<String> {
    rows.into_iter()
        .find(|r| !r.prerelease && !r.draft)
        .map(|r| r.tag_name)
}

fn latest_stable_tag_from(url: &str, timeout: Duration) -> Result<String, CoreError> {
    let resp = http_client(timeout)?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| CoreError::Usage(format!("could not reach GitHub releases: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        let hint = if matches!(status.as_u16(), 403 | 429) {
            " (GitHub rate limit for unauthenticated requests; try again later)"
        } else {
            ""
        };
        return Err(CoreError::Usage(format!(
            "GitHub releases returned HTTP {}{hint}",
            status.as_u16()
        )));
    }
    let rows: Vec<ReleaseRow> = resp
        .json()
        .map_err(|e| CoreError::Usage(format!("could not parse GitHub releases JSON: {e}")))?;
    pick_stable(rows).ok_or_else(|| CoreError::Usage("no stable GitHub release found".into()))
}

/// Download to a private temp file. Any HTTP or write failure is an error, so a failed
/// download can never reach the shell.
fn download_to_temp(url: &str) -> Result<tempfile::NamedTempFile, CoreError> {
    let resp = http_client(DOWNLOAD_TIMEOUT)?
        .get(url)
        .send()
        .map_err(|e| CoreError::Usage(format!("could not download install script: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(CoreError::Usage(format!(
            "could not download install script: HTTP {}",
            status.as_u16()
        )));
    }
    let body = resp
        .bytes()
        .map_err(|e| CoreError::Usage(format!("could not download install script: {e}")))?;
    if body.is_empty() {
        return Err(CoreError::Usage("install script download was empty".into()));
    }
    let mut file = tempfile::Builder::new()
        .prefix("plane-cli-install-")
        .suffix(".sh")
        .tempfile()?;
    file.write_all(&body)?;
    file.flush()?;
    Ok(file)
}

/// Install prefix that puts the new binary at `exe`: `<prefix>/bin/plane-cli`. Any other layout
/// (a `cargo build` output, a renamed binary) cannot be updated in place.
fn install_prefix_for(exe: &Path) -> Result<PathBuf, CoreError> {
    let prefix = (exe.file_name().is_some_and(|n| n == "plane-cli"))
        .then(|| exe.parent())
        .flatten()
        .filter(|bin| bin.file_name().is_some_and(|n| n == "bin"))
        .and_then(Path::parent);
    prefix.map(Path::to_path_buf).ok_or_else(|| {
        CoreError::Usage(format!(
            "cannot update {} in place: the installer writes <prefix>/bin/plane-cli. Nothing was changed. Install with: {INSTALL_ONE_LINER}",
            exe.display()
        ))
    })
}

fn ensure_writable(dir: &Path) -> Result<(), CoreError> {
    tempfile::Builder::new()
        .prefix(".plane-cli-write-check")
        .tempfile_in(dir)
        .map(drop)
        .map_err(|e| {
            CoreError::Usage(format!(
                "cannot update the binary in {}: {e}. Nothing was changed. Re-run with permission to write there",
                dir.display()
            ))
        })
}

/// The version string from `plane-cli --version` output.
fn parse_version_output(output: &str) -> Option<&str> {
    output.split_whitespace().nth(1)
}

/// Success means the binary at `exe` now reports `expected`.
fn verify_installed(exe: &Path, expected: &Version) -> Result<(), CoreError> {
    let out = Command::new(exe).arg("--version").output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    match parse_version_output(&text) {
        Some(v) if out.status.success() && v == expected.to_string() => Ok(()),
        found => Err(CoreError::Usage(format!(
            "update did not take effect: {} reports {}, expected {expected}",
            exe.display(),
            found.unwrap_or("no version")
        ))),
    }
}

/// Update the running binary's own location to `tag`, and confirm it changed.
fn apply_update(tag: &str, latest: &Version) -> Result<(), CoreError> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let prefix = install_prefix_for(&exe)?;
    if let Some(dir) = exe.parent() {
        ensure_writable(dir)?;
    }
    run_installer(tag, &prefix)?;
    verify_installed(&exe, latest)?;
    eprintln!("Updated {} to {latest}", exe.display());
    Ok(())
}

/// Run the install script from the release being installed, pinned to that tag. `PLANE_*`
/// variables (API keys, and anything an env file set) are not passed to it.
fn run_installer(tag: &str, prefix: &Path) -> Result<(), CoreError> {
    tag_to_version(tag)?;
    let script = download_to_temp(&format!("{RAW_BASE}/{tag}/scripts/install.sh"))?;
    let mut cmd = Command::new("bash");
    cmd.arg(script.path());
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("PLANE_") {
            cmd.env_remove(key);
        }
    }
    let st = cmd
        .env("PLANE_CLI_VERSION", tag)
        .env("PREFIX", prefix)
        .status()?;
    if !st.success() {
        return Err(CoreError::Usage(format!(
            "install script exited with status {}",
            st.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

fn tag_to_version(tag: &str) -> Result<Version, CoreError> {
    let s = tag.strip_prefix('v').unwrap_or(tag);
    Version::parse(s)
        .map_err(|_| CoreError::Usage(format!("release tag {tag:?} is not a valid semver version")))
}

/// When a newer stable GitHub release exists than this binary, returns display info.
/// Any failure (network, rate limit, parse) returns `None` so callers can ignore quietly.
/// `PLANE_CLI_NO_UPDATE_CHECK=1` turns the check off.
pub struct UpdateHint {
    pub current: String,
    pub latest_tag: String,
    pub short_tip: &'static str,
}

pub fn try_update_hint() -> Option<UpdateHint> {
    let off = std::env::var("PLANE_CLI_NO_UPDATE_CHECK")
        .is_ok_and(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes"));
    if off {
        return None;
    }
    let tag = latest_stable_tag_from(RELEASES_API, HINT_TIMEOUT).ok()?;
    let latest = tag_to_version(&tag).ok()?;
    let current = Version::parse(env!("CARGO_PKG_VERSION")).expect("crate version is valid semver");
    if latest <= current {
        return None;
    }
    Some(UpdateHint {
        current: current.to_string(),
        latest_tag: tag,
        short_tip: "plane-cli self update --apply",
    })
}

pub fn run(cli: &Cli, cmd: &SelfManageCmd) -> Result<(), CoreError> {
    match cmd {
        SelfManageCmd::Check => {
            let tag = latest_stable_tag_from(RELEASES_API, CHECK_TIMEOUT)?;
            let latest = tag_to_version(&tag)?;
            let current =
                Version::parse(env!("CARGO_PKG_VERSION")).expect("crate version is valid semver");
            let update_available = latest > current;

            if cli.json_mode() {
                let env = ResponseEnvelope::success(json!({
                    "current": current.to_string(),
                    "latest_tag": tag,
                    "latest": latest.to_string(),
                    "update_available": update_available,
                }));
                print_json(&env)?;
            } else {
                eprintln!("This binary: {current}");
                eprintln!("Latest release: {latest} ({tag})");
                if update_available {
                    eprintln!("Update available: yes");
                    eprintln!("Install: plane-cli self update --apply");
                    eprintln!("Or: {INSTALL_ONE_LINER}");
                } else {
                    eprintln!("Update available: no");
                }
            }
            Ok(())
        }
        SelfManageCmd::Update { apply } => {
            if *apply && cli.json_mode() {
                return Err(CoreError::Usage(
                    "machine-readable JSON mode cannot be combined with `self update --apply` (installer prints text); unset PLANE_CLI_JSON or omit `--json`"
                        .into(),
                ));
            }

            if !apply {
                if cli.json_mode() {
                    let env = ResponseEnvelope::success(json!({
                        "command": INSTALL_ONE_LINER,
                    }));
                    print_json(&env)?;
                } else {
                    eprintln!("Run:");
                    println!("{INSTALL_ONE_LINER}");
                }
                return Ok(());
            }

            let tag = latest_stable_tag_from(RELEASES_API, CHECK_TIMEOUT)?;
            let latest = tag_to_version(&tag)?;
            let current =
                Version::parse(env!("CARGO_PKG_VERSION")).expect("crate version is valid semver");
            if latest <= current {
                eprintln!("plane-cli {current} is up to date (latest release: {latest})");
                return Ok(());
            }
            apply_update(&tag, &latest)
        }
        SelfManageCmd::Skills { cmd } => match cmd {
            SelfSkillsCmd::Check => {
                let payload = skill_cmd::check_json_payload();
                if cli.json_mode() {
                    let env = ResponseEnvelope::success(payload);
                    print_json(&env)?;
                } else {
                    let expected = payload
                        .get("expected_files")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default();
                    let any_complete = payload
                        .get("installed")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if any_complete {
                        eprintln!("plane-cli skill: complete bundle found");
                    } else if payload
                        .get("bundles")
                        .and_then(|v| v.as_array())
                        .is_some_and(|a| !a.is_empty())
                    {
                        eprintln!("plane-cli skill: partial (missing bundled files)");
                    } else {
                        eprintln!("plane-cli skill: not found");
                    }
                    eprintln!("Expected files: {expected}");
                    if let Some(arr) = payload.get("bundles").and_then(|v| v.as_array()) {
                        for b in arr {
                            let dir = b.get("dir").and_then(|v| v.as_str()).unwrap_or("?");
                            let complete =
                                b.get("complete").and_then(|v| v.as_bool()).unwrap_or(false);
                            let tag = if complete { "complete" } else { "incomplete" };
                            eprintln!("- {dir} ({tag})");
                            if let Some(missing) = b.get("missing").and_then(|v| v.as_array()) {
                                for m in missing.iter().filter_map(|v| v.as_str()) {
                                    eprintln!("    missing: {m}");
                                }
                            }
                        }
                    }
                    if !any_complete {
                        let cmd = payload
                            .get("install_local_command")
                            .and_then(|v| v.as_str())
                            .unwrap_or("plane-cli self skills install --local");
                        eprintln!("Install full bundle: {cmd}");
                        if let Some(p) = payload
                            .get("install_openclaw_command")
                            .and_then(|v| v.as_str())
                        {
                            eprintln!("OpenClaw: {p}");
                        }
                        if let Some(p) = payload.get("install_command").and_then(|v| v.as_str()) {
                            eprintln!("Or via npx: {p}");
                        }
                    }
                }
                Ok(())
            }
            SelfSkillsCmd::Install { openclaw, local } => {
                if *openclaw {
                    let dir = skill_cmd::install_openclaw()?;
                    let files: Vec<_> = skill_cmd::SKILL_FILES
                        .iter()
                        .map(|(name, _)| name.to_string())
                        .collect();
                    if cli.json_mode() {
                        let env = ResponseEnvelope::success(json!({
                            "skill_name": skill_cmd::SKILL_NAME,
                            "target": "openclaw",
                            "path": dir.display().to_string(),
                            "files": files,
                        }));
                        print_json(&env)?;
                    } else {
                        eprintln!(
                            "Wrote OpenClaw skill bundle to {} ({})",
                            dir.display(),
                            files.join(", ")
                        );
                    }
                    Ok(())
                } else if *local {
                    let dirs = skill_cmd::install_local()?;
                    let files: Vec<_> = skill_cmd::SKILL_FILES
                        .iter()
                        .map(|(name, _)| name.to_string())
                        .collect();
                    if cli.json_mode() {
                        let paths: Vec<_> = dirs.iter().map(|d| d.display().to_string()).collect();
                        let env = ResponseEnvelope::success(json!({
                            "skill_name": skill_cmd::SKILL_NAME,
                            "target": "local",
                            "paths": paths,
                            "files": files,
                        }));
                        print_json(&env)?;
                    } else {
                        eprintln!("Wrote skill bundle ({}) to:", files.join(", "));
                        for dir in dirs {
                            eprintln!("- {}", dir.display());
                        }
                    }
                    Ok(())
                } else if cli.json_mode() {
                    Err(CoreError::Usage(
                        "machine-readable JSON mode cannot be combined with `self skills install` (npx wizard is interactive); use `self skills install --local`, `--openclaw`, or unset PLANE_CLI_JSON / omit `--json`"
                            .into(),
                    ))
                } else {
                    skill_cmd::run_install_interactive()
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELEASES: &str = r#"[
        {"tag_name":"v0.5.0","prerelease":false,"draft":true},
        {"tag_name":"v0.4.0-rc.1","prerelease":true,"draft":false},
        {"tag_name":"v0.3.0","prerelease":false,"draft":false},
        {"tag_name":"v0.2.0","prerelease":false,"draft":false}
    ]"#;

    #[test]
    fn latest_stable_skips_drafts_and_prereleases() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("GET", "/releases")
            .match_header("user-agent", mockito::Matcher::Regex("^plane-cli/".into()))
            .with_body(RELEASES)
            .create();
        let url = format!("{}/releases", server.url());
        assert_eq!(
            latest_stable_tag_from(&url, CHECK_TIMEOUT).unwrap(),
            "v0.3.0"
        );
        m.assert();
    }

    #[test]
    fn prefix_comes_from_the_running_binary() {
        assert_eq!(
            install_prefix_for(Path::new("/opt/tools/bin/plane-cli")).unwrap(),
            PathBuf::from("/opt/tools")
        );
        for bad in [
            "/work/plane-cli/target/release/plane-cli",
            "/opt/tools/bin/plane-cli-old",
            "plane-cli",
        ] {
            let e = install_prefix_for(Path::new(bad)).unwrap_err();
            assert!(e.to_string().contains("Nothing was changed"), "{e}");
        }
    }

    #[test]
    fn version_output_is_parsed() {
        assert_eq!(parse_version_output("plane-cli 0.3.0\n"), Some("0.3.0"));
        assert_eq!(parse_version_output(""), None);
    }

    #[cfg(unix)]
    #[test]
    fn stale_binary_is_not_reported_as_updated() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("plane-cli");
        std::fs::write(&exe, "#!/bin/sh\necho 'plane-cli 0.3.0'\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(verify_installed(&exe, &Version::new(0, 3, 0)).is_ok());
        let e = verify_installed(&exe, &Version::new(0, 4, 0)).unwrap_err();
        assert!(e.to_string().contains("did not take effect"), "{e}");
    }

    #[test]
    fn rate_limit_is_reported() {
        let mut server = mockito::Server::new();
        server.mock("GET", "/releases").with_status(403).create();
        let url = format!("{}/releases", server.url());
        let e = latest_stable_tag_from(&url, CHECK_TIMEOUT).unwrap_err();
        assert!(e.to_string().contains("rate limit"), "{e}");
    }

    #[test]
    fn failed_or_empty_script_download_is_an_error() {
        let mut server = mockito::Server::new();
        server.mock("GET", "/missing").with_status(404).create();
        server.mock("GET", "/empty").with_body("").create();
        server.mock("GET", "/ok").with_body("echo hi\n").create();
        assert!(download_to_temp(&format!("{}/missing", server.url())).is_err());
        assert!(download_to_temp(&format!("{}/empty", server.url())).is_err());
        let f = download_to_temp(&format!("{}/ok", server.url())).unwrap();
        assert_eq!(std::fs::read_to_string(f.path()).unwrap(), "echo hi\n");
    }
}
