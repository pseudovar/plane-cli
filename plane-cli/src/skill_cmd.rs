use plane_core::CoreError;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const SKILL_NAME: &str = "plane-cli";
pub const SKILL_REPO: &str = "pseudovar/plane-cli";

/// Bundled skill files at build time (OpenClaw-compatible `metadata.openclaw` gates).
pub(crate) const SKILL_FILES: &[(&str, &str)] = &[
    ("SKILL.md", include_str!("../../skills/plane-cli/SKILL.md")),
    (
        "CRUD-REFERENCE.md",
        include_str!("../../skills/plane-cli/CRUD-REFERENCE.md"),
    ),
    (
        "WORKFLOWS.md",
        include_str!("../../skills/plane-cli/WORKFLOWS.md"),
    ),
];

#[derive(Debug, Clone)]
pub struct SkillBundleStatus {
    pub dir: PathBuf,
    pub complete: bool,
    pub present: Vec<String>,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SkillCheck {
    pub any_complete: bool,
    pub bundles: Vec<SkillBundleStatus>,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn codex_home() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CODEX_HOME") {
        return Some(PathBuf::from(p));
    }
    home_dir().map(|h| h.join(".codex"))
}

fn bundled_file_names() -> Vec<&'static str> {
    SKILL_FILES.iter().map(|(name, _)| *name).collect()
}

fn candidate_skill_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = home_dir() {
        out.push(home.join(".agents/skills").join(SKILL_NAME));
        out.push(home.join(".cursor/skills").join(SKILL_NAME));
        out.push(home.join(".claude/skills").join(SKILL_NAME));
        out.push(home.join(".openclaw/skills").join(SKILL_NAME));
    }
    if let Some(codex) = codex_home() {
        out.push(codex.join("skills").join(SKILL_NAME));
    }
    out.push(
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("skills")
            .join(SKILL_NAME),
    );
    out.push(
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".agents")
            .join("skills")
            .join(SKILL_NAME),
    );
    out
}

fn local_agent_skill_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = home_dir() {
        out.push(home.join(".agents/skills").join(SKILL_NAME));
        out.push(home.join(".cursor/skills").join(SKILL_NAME));
        out.push(home.join(".claude/skills").join(SKILL_NAME));
    }
    if let Some(codex) = codex_home() {
        out.push(codex.join("skills").join(SKILL_NAME));
    }
    out
}

pub fn bundle_status(dir: &Path) -> SkillBundleStatus {
    let expected = bundled_file_names();
    let mut present = Vec::new();
    let mut missing = Vec::new();
    for name in &expected {
        if dir.join(name).is_file() {
            present.push((*name).to_string());
        } else {
            missing.push((*name).to_string());
        }
    }
    let complete = missing.is_empty() && !present.is_empty();
    SkillBundleStatus {
        dir: dir.to_path_buf(),
        complete,
        present,
        missing,
    }
}

pub fn check_installed() -> SkillCheck {
    let mut bundles = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in candidate_skill_dirs() {
        let key = dir.display().to_string();
        if !seen.insert(key.clone()) {
            continue;
        }
        if dir.join("SKILL.md").is_file() || dir.exists() {
            bundles.push(bundle_status(&dir));
        }
    }
    let any_complete = bundles.iter().any(|b| b.complete);
    SkillCheck {
        any_complete,
        bundles,
    }
}

/// Write all bundled skill files into `dest` (creates the directory if needed).
pub fn install_bundle_to(dest: &Path) -> Result<(), CoreError> {
    fs::create_dir_all(dest)
        .map_err(|e| CoreError::Usage(format!("create skill directory {}: {e}", dest.display())))?;
    for (name, contents) in SKILL_FILES {
        let path = dest.join(name);
        fs::write(&path, contents)
            .map_err(|e| CoreError::Usage(format!("write skill file {}: {e}", path.display())))?;
    }
    Ok(())
}

fn openclaw_skill_dir() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".openclaw/skills").join(SKILL_NAME))
}

/// Copy bundled skill files into `~/.openclaw/skills/plane-cli/`.
pub fn install_openclaw() -> Result<PathBuf, CoreError> {
    let dir = openclaw_skill_dir()
        .ok_or_else(|| CoreError::Usage("HOME is not set; cannot install OpenClaw skill".into()))?;
    install_bundle_to(&dir)?;
    Ok(dir)
}

/// Copy bundled skill files into standard local agent skill dirs (Cursor, Codex, Claude, …).
pub fn install_local() -> Result<Vec<PathBuf>, CoreError> {
    let dirs = local_agent_skill_dirs();
    if dirs.is_empty() {
        return Err(CoreError::Usage(
            "HOME (or CODEX_HOME) is not set; cannot install local skill".into(),
        ));
    }
    for dir in &dirs {
        install_bundle_to(dir)?;
    }
    Ok(dirs)
}

pub fn skills_install_command() -> Vec<String> {
    vec![
        "skills@latest".into(),
        "add".into(),
        SKILL_REPO.into(),
        "--skill".into(),
        SKILL_NAME.into(),
    ]
}

pub fn ensure_npx() -> Result<(), CoreError> {
    Command::new("npx")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| {
            CoreError::Usage(
                "could not run `npx`; install Node.js/npm, or use `plane-cli self skills install --local` (no npx needed)"
                    .into(),
            )
        })?
        .success()
        .then_some(())
        .ok_or_else(|| CoreError::Usage("`npx --version` failed".into()))
}

pub fn run_install_interactive() -> Result<(), CoreError> {
    ensure_npx()?;
    let parts = skills_install_command();
    let st = Command::new("npx").args(parts).status()?;
    if !st.success() {
        return Err(CoreError::Usage(format!(
            "skill installer exited with status {}",
            st.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

pub fn check_json_payload() -> serde_json::Value {
    let check = check_installed();
    let expected: Vec<_> = bundled_file_names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let bundles: Vec<_> = check
        .bundles
        .iter()
        .map(|b| {
            json!({
                "dir": b.dir.display().to_string(),
                "complete": b.complete,
                "present": b.present,
                "missing": b.missing,
            })
        })
        .collect();
    json!({
        "skill_name": SKILL_NAME,
        "repo": SKILL_REPO,
        "expected_files": expected,
        "installed": check.any_complete,
        "bundles": bundles,
        "install_command": format!("npx {}", skills_install_command().join(" ")),
        "install_local_command": format!("{} self skills install --local", env!("CARGO_PKG_NAME")),
        "install_openclaw_command": format!("{} self skills install --openclaw", env!("CARGO_PKG_NAME")),
    })
}

#[cfg(test)]
mod tests {
    use super::{bundle_status, install_bundle_to, SKILL_FILES};
    use std::fs;

    #[test]
    fn embedded_skill_has_openclaw_bin_requirement() {
        let skill_md = SKILL_FILES
            .iter()
            .find(|(name, _)| *name == "SKILL.md")
            .map(|(_, contents)| *contents)
            .expect("SKILL.md bundled");
        assert!(
            skill_md.contains("\"openclaw\"")
                && skill_md.contains("\"bins\"")
                && skill_md.contains("plane-cli"),
            "SKILL.md frontmatter should include metadata.openclaw.requires.bins for plane-cli"
        );
    }

    #[test]
    fn embedded_skill_includes_crud_and_workflows() {
        let names: Vec<_> = SKILL_FILES.iter().map(|(name, _)| *name).collect();
        assert!(names.contains(&"CRUD-REFERENCE.md"));
        assert!(names.contains(&"WORKFLOWS.md"));
    }

    #[test]
    fn install_bundle_writes_all_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        install_bundle_to(tmp.path()).expect("install");
        let status = bundle_status(tmp.path());
        assert!(status.complete, "missing: {:?}", status.missing);
        assert_eq!(status.present.len(), SKILL_FILES.len());
        for (name, expected) in SKILL_FILES {
            let got = fs::read_to_string(tmp.path().join(name)).expect("read");
            assert_eq!(got, *expected);
        }
    }
}
