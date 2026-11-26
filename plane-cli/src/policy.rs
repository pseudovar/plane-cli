//! Per-project write policy: rules plane-cli enforces on work-item writes.
//!
//! A policy is a TOML file named after the project identifier. Lookup order: the first
//! `.plane-cli/policy/<PROJECT>.toml` in the current directory or a parent (stopping at the
//! repository root), else `<config dir>/plane-cli/policy/<PROJECT>.toml` (next to the global
//! env file). The repo-local file replaces the global one; they are not merged. Files are
//! written with `plane-cli config policy init`, not by hand.
//!
//! This module holds the file format, lookup and the pure rule checks; the API-backed
//! orchestration lives in `dispatch/policy_gate.rs`.

use crate::plane_env_files;
use plane_core::CoreError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const POLICY_VERSION: u32 = 1;

/// Rule names, as reported in `error.details.rule` and dry-run checks.
pub const RULE_SEARCH: &str = "search_before_create";
pub const RULE_DESCRIPTION: &str = "description_shape";
pub const RULE_REQUIRED: &str = "required_fields";
pub const RULE_LABELS: &str = "allowed_labels";
pub const RULE_COMMENT: &str = "comment_on_change";
pub const RULE_STATES: &str = "state_permissions";
pub const RULE_DELETES: &str = "no_deletes";

/// Fields `required_fields.on_create` may name.
pub const REQUIRABLE_FIELDS: &[&str] = &[
    "priority",
    "module",
    "labels",
    "assignees",
    "state",
    "target_date",
    "start_date",
];
/// Fields `comment_on_change.fields` may name.
pub const WATCHABLE_FIELDS: &[&str] = &["state", "assignees", "priority"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_before_create: Option<SearchRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<DescriptionRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_fields: Option<RequiredFields>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub labels: Option<LabelsRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment_on_change: Option<CommentOnChange>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_rules: Vec<StateRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deletes: Option<DeletesRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRule {
    /// Name similarity (0-1) at or above which an existing item counts as a close match.
    #[serde(default = "default_threshold")]
    pub threshold: f64,
}

fn default_threshold() -> f64 {
    0.85
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptionRule {
    /// Section headings that must appear; `"A|B"` accepts either.
    #[serde(default)]
    pub required_sections: Vec<String>,
    #[serde(default = "default_true")]
    pub forbid_single_paragraph: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredFields {
    #[serde(default)]
    pub on_create: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelsRule {
    /// Label names a work item may carry (case-insensitive). An empty list allows no labels.
    #[serde(default)]
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommentOnChange {
    #[serde(default)]
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRule {
    /// State names this rule covers (case-insensitive).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    /// State groups this rule covers (`backlog`, `unstarted`, `started`, `completed`,
    /// `cancelled`, `triage`), so a renamed or new state in the group is covered too.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    pub members: Vec<String>,
}

impl StateRule {
    fn covers(&self, name: &str, group: Option<&str>) -> bool {
        self.states.iter().any(|s| s.eq_ignore_ascii_case(name))
            || group.is_some_and(|g| self.groups.iter().any(|x| x.eq_ignore_ascii_case(g)))
    }

    fn describe(&self) -> String {
        let mut parts: Vec<String> = self.states.clone();
        parts.extend(self.groups.iter().map(|g| format!("any {g} state")));
        parts.join(", ")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeletesRule {
    /// Members allowed to delete; everyone else is refused.
    #[serde(default)]
    pub members: Vec<String>,
}

impl Policy {
    pub fn parse(text: &str, path: &Path) -> Result<Policy, CoreError> {
        let p: Policy = toml::from_str(text).map_err(|e| {
            CoreError::usage_code(
                "policy_invalid",
                format!("policy file {} is not valid: {e}", path.display()),
            )
        })?;
        p.validate(path)?;
        Ok(p)
    }

    fn validate(&self, path: &Path) -> Result<(), CoreError> {
        let bad = |msg: String| {
            Err(CoreError::usage_code(
                "policy_invalid",
                format!("policy file {}: {msg}", path.display()),
            ))
        };
        if self.version != POLICY_VERSION {
            return bad(format!(
                "version {} is not supported (expected {POLICY_VERSION})",
                self.version
            ));
        }
        if let Some(s) = &self.search_before_create {
            if !(0.0..=1.0).contains(&s.threshold) {
                return bad("search_before_create.threshold must be between 0 and 1".into());
            }
        }
        if let Some(r) = &self.required_fields {
            for f in &r.on_create {
                if !REQUIRABLE_FIELDS.contains(&f.as_str()) {
                    return bad(format!(
                        "required_fields.on_create: unknown field {f:?} (one of {})",
                        REQUIRABLE_FIELDS.join(", ")
                    ));
                }
            }
        }
        if let Some(c) = &self.comment_on_change {
            for f in &c.fields {
                if !WATCHABLE_FIELDS.contains(&f.as_str()) {
                    return bad(format!(
                        "comment_on_change.fields: unknown field {f:?} (one of {})",
                        WATCHABLE_FIELDS.join(", ")
                    ));
                }
            }
        }
        for r in &self.state_rules {
            if (r.states.is_empty() && r.groups.is_empty()) || r.members.is_empty() {
                return bad(
                    "each [[state_rules]] needs `states` or `groups`, and non-empty `members`"
                        .into(),
                );
            }
            for g in &r.groups {
                if !plane_core::STATE_GROUPS.contains(&g.to_lowercase().as_str()) {
                    return bad(format!(
                        "state_rules.groups: unknown group {g:?} (one of {})",
                        plane_core::STATE_GROUPS.join(", ")
                    ));
                }
            }
        }
        Ok(())
    }

    /// Merge a repo-local policy over this one so it can only tighten: sets of required things
    /// are unions, sets of allowed things are intersections, the duplicate threshold takes the
    /// stricter (lower) value, state rules accumulate (a state covered by several rules needs
    /// a member listed in all of them), and no rule is ever dropped.
    pub fn tighten_with(mut self, local: &Policy) -> Policy {
        fn union(a: &mut Vec<String>, b: &[String]) {
            for x in b {
                if !a.iter().any(|y| y.eq_ignore_ascii_case(x)) {
                    a.push(x.clone());
                }
            }
        }
        fn intersect(a: &[String], b: &[String]) -> Vec<String> {
            a.iter()
                .filter(|x| b.iter().any(|y| y.eq_ignore_ascii_case(x)))
                .cloned()
                .collect()
        }
        self.search_before_create = match (self.search_before_create, &local.search_before_create) {
            (Some(a), Some(b)) => Some(SearchRule {
                threshold: a.threshold.min(b.threshold),
            }),
            (a, b) => a.or_else(|| b.clone()),
        };
        self.description = match (self.description, &local.description) {
            (Some(mut a), Some(b)) => {
                union(&mut a.required_sections, &b.required_sections);
                a.forbid_single_paragraph |= b.forbid_single_paragraph;
                Some(a)
            }
            (a, b) => a.or_else(|| b.clone()),
        };
        self.required_fields = match (self.required_fields, &local.required_fields) {
            (Some(mut a), Some(b)) => {
                union(&mut a.on_create, &b.on_create);
                Some(a)
            }
            (a, b) => a.or_else(|| b.clone()),
        };
        self.labels = match (self.labels, &local.labels) {
            (Some(a), Some(b)) => Some(LabelsRule {
                allowed: intersect(&a.allowed, &b.allowed),
            }),
            (a, b) => a.or_else(|| b.clone()),
        };
        self.comment_on_change = match (self.comment_on_change, &local.comment_on_change) {
            (Some(mut a), Some(b)) => {
                union(&mut a.fields, &b.fields);
                Some(a)
            }
            (a, b) => a.or_else(|| b.clone()),
        };
        for r in &local.state_rules {
            if !self.state_rules.contains(r) {
                self.state_rules.push(r.clone());
            }
        }
        self.deletes = match (self.deletes, &local.deletes) {
            (Some(a), Some(b)) => Some(DeletesRule {
                members: intersect(&a.members, &b.members),
            }),
            (a, b) => a.or_else(|| b.clone()),
        };
        self
    }

    /// One plain sentence per active rule, for `projects context` and `config policy show`.
    pub fn summary(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(s) = &self.search_before_create {
            out.push(format!(
                "{RULE_SEARCH}: create refuses a name that matches an existing item (similarity >= {}) unless --allow-duplicate",
                s.threshold
            ));
        }
        if let Some(d) = &self.description {
            let mut parts = Vec::new();
            if !d.required_sections.is_empty() {
                parts.push(format!(
                    "sections {} (headings in description_html)",
                    d.required_sections
                        .iter()
                        .map(|s| format!("\"{s}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if d.forbid_single_paragraph {
                parts.push("not a single plain paragraph".into());
            }
            if !parts.is_empty() {
                out.push(format!(
                    "{RULE_DESCRIPTION}: a written description needs {}",
                    parts.join("; ")
                ));
            }
        }
        if let Some(r) = &self.required_fields {
            if !r.on_create.is_empty() {
                out.push(format!(
                    "{RULE_REQUIRED}: create needs {} (module through --module)",
                    r.on_create.join(", ")
                ));
            }
        }
        if let Some(l) = &self.labels {
            out.push(if l.allowed.is_empty() {
                format!("{RULE_LABELS}: no labels may be set")
            } else {
                format!("{RULE_LABELS}: labels only from {}", l.allowed.join(", "))
            });
        }
        if let Some(c) = &self.comment_on_change {
            if !c.fields.is_empty() {
                out.push(format!(
                    "{RULE_COMMENT}: changing {} needs --reason, posted as a comment",
                    c.fields.join(", ")
                ));
            }
        }
        for r in &self.state_rules {
            out.push(format!(
                "{RULE_STATES}: only {} may set {}",
                r.members.join(", "),
                r.describe()
            ));
        }
        if let Some(d) = &self.deletes {
            out.push(if d.members.is_empty() {
                format!("{RULE_DELETES}: nobody may delete through plane-cli")
            } else {
                format!("{RULE_DELETES}: only {} may delete", d.members.join(", "))
            });
        }
        out
    }
}

/// One policy file that was merged into the active policy.
#[derive(Debug, Clone)]
pub struct PolicyFile {
    pub path: PathBuf,
    pub source: &'static str,
}

/// The active policy for a project: the global file with any repo-local files merged over it.
#[derive(Debug, Clone)]
pub struct LoadedPolicy {
    pub project: String,
    /// Merged files, global first, then repo-local from the outermost directory inwards.
    pub files: Vec<PolicyFile>,
    pub policy: Policy,
}

impl LoadedPolicy {
    /// The most specific merged file (the last one).
    pub fn file(&self) -> &PolicyFile {
        self.files
            .last()
            .expect("a loaded policy has at least one file")
    }

    pub fn files_json(&self) -> Value {
        json!(self
            .files
            .iter()
            .map(|f| json!({"path": f.path.display().to_string(), "source": f.source}))
            .collect::<Vec<_>>())
    }

    pub fn to_json(&self) -> Value {
        json!({
            "project": self.project,
            "file": self.file().path.display().to_string(),
            "source": self.file().source,
            "files": self.files_json(),
            "rules": serde_json::to_value(&self.policy).unwrap_or(Value::Null),
            "summary": self.policy.summary(),
        })
    }
}

/// Upper-case project identifier from `--project`; policy files are keyed by identifier.
pub fn normalize_identifier(raw: &str) -> Result<String, CoreError> {
    let t = raw.trim();
    let ok = !t.is_empty() && t.len() <= 12 && t.chars().all(|c| c.is_ascii_alphanumeric());
    if !ok {
        return Err(CoreError::usage_code(
            "invalid_project_identifier",
            format!(
                "policy files are named after the project identifier (e.g. WEB), 1-12 letters or digits; got {t:?}"
            ),
        ));
    }
    Ok(t.to_ascii_uppercase())
}

/// `<config dir>/plane-cli/policy`.
pub fn global_policy_dir() -> Option<PathBuf> {
    plane_env_files::global_plane_cli_env_path().and_then(|p| p.parent().map(|d| d.join("policy")))
}

/// Directories searched for repo-local policies, innermost first: `start` and its parents up
/// to and including the first that holds `.git` or is `home`, whichever comes first. When
/// neither is above `start`, only `start` itself is searched (never up to `/`).
pub fn local_search_dirs(start: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for dir in start.ancestors() {
        dirs.push(dir.to_path_buf());
        if dir.join(".git").exists() || home.is_some_and(|h| dir == h) {
            return dirs;
        }
    }
    vec![start.to_path_buf()]
}

/// Why a repo-local policy path cannot be trusted: not owned by the current user, or
/// writable by group/other (checked on the file and on `.plane-cli` and `.plane-cli/policy`).
#[cfg(unix)]
pub fn untrusted_reason(file: &Path) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    // SAFETY: geteuid has no preconditions and cannot fail.
    let uid = unsafe { libc::geteuid() };
    let policy_dir = file.parent()?;
    let base = policy_dir.parent()?;
    for p in [file, policy_dir, base] {
        let m = match std::fs::symlink_metadata(p) {
            Ok(m) => m,
            Err(e) => return Some(format!("{}: {e}", p.display())),
        };
        if m.file_type().is_symlink() {
            return Some(format!("{} is a symlink", p.display()));
        }
        if m.uid() != uid {
            return Some(format!("{} is not owned by you", p.display()));
        }
        if m.mode() & 0o022 != 0 {
            return Some(format!("{} is writable by group or others", p.display()));
        }
    }
    None
}

#[cfg(not(unix))]
pub fn untrusted_reason(_file: &Path) -> Option<String> {
    None
}

/// One path checked for a project's policy and what happened to it.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub path: PathBuf,
    pub source: &'static str,
    /// `merged`, `missing` or `skipped`.
    pub status: &'static str,
    pub reason: Option<String>,
}

impl Candidate {
    pub fn to_json(&self) -> Value {
        json!({
            "path": self.path.display().to_string(),
            "source": self.source,
            "status": self.status,
            "reason": self.reason,
        })
    }
}

/// Every path checked for `project`, global first, then repo-local from the outermost
/// directory inwards (the merge order).
pub fn candidate_paths(project: &str) -> Vec<(PathBuf, &'static str)> {
    let file = format!("{project}.toml");
    let mut out = Vec::new();
    if let Some(d) = global_policy_dir() {
        out.push((d.join(&file), "global"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let mut local: Vec<_> = local_search_dirs(&cwd, home.as_deref())
            .into_iter()
            .map(|d| (d.join(".plane-cli").join("policy").join(&file), "repo"))
            .collect();
        local.reverse();
        // The global dir can sit under a searched directory (e.g. ~/.plane-cli is not it, but
        // keep the list free of repeats anyway).
        local.retain(|(p, _)| out.iter().all(|(q, _)| q != p));
        out.extend(local);
    }
    out
}

/// Resolve the active policy for `project`: what was checked, and the merged result.
pub fn resolve(project: &str) -> Result<(Vec<Candidate>, Option<LoadedPolicy>), CoreError> {
    let project = normalize_identifier(project)?;
    let mut candidates = Vec::new();
    let mut loaded: Option<LoadedPolicy> = None;
    for (path, source) in candidate_paths(&project) {
        if !path.is_file() {
            candidates.push(Candidate {
                path,
                source,
                status: "missing",
                reason: None,
            });
            continue;
        }
        if source == "repo" {
            if let Some(reason) = untrusted_reason(&path) {
                plane_core::push_warning(format!(
                    "ignored repo-local policy {}: {reason}",
                    path.display()
                ));
                candidates.push(Candidate {
                    path,
                    source,
                    status: "skipped",
                    reason: Some(reason),
                });
                continue;
            }
        }
        let text = std::fs::read_to_string(&path).map_err(|e| {
            CoreError::usage_code(
                "policy_invalid",
                format!("cannot read policy file {}: {e}", path.display()),
            )
        })?;
        let policy = Policy::parse(&text, &path)?;
        let file = PolicyFile {
            path: path.clone(),
            source,
        };
        loaded = Some(match loaded {
            None => LoadedPolicy {
                project: project.clone(),
                files: vec![file],
                policy,
            },
            Some(mut l) => {
                l.policy = l.policy.tighten_with(&policy);
                l.files.push(file);
                l
            }
        });
        candidates.push(Candidate {
            path,
            source,
            status: "merged",
            reason: None,
        });
    }
    Ok((candidates, loaded))
}

/// The active policy for `project` (identifier), or `None` when no file exists.
pub fn load(project: &str) -> Result<Option<LoadedPolicy>, CoreError> {
    Ok(resolve(project)?.1)
}

/// The commented starter file `config policy init` writes. Without owners, the member rules
/// are written commented out.
/// TOML array of quoted strings.
fn toml_list(items: &[String]) -> String {
    items
        .iter()
        .map(|i| toml::Value::String(i.clone()).to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Starter-file options for `config policy init`.
#[derive(Debug, Default, Clone)]
pub struct TemplateOptions {
    /// Members allowed to set restricted states and to delete; none = those rules commented out.
    pub owners: Vec<String>,
    /// `required_fields.on_create`; none = `["priority"]`.
    pub required: Vec<String>,
    /// `labels.allowed`; none = the section commented out.
    pub labels: Vec<String>,
}

pub fn template(project: &str, opts: &TemplateOptions) -> String {
    let (pre, members) = if opts.owners.is_empty() {
        ("# ", "\"<member>\"".to_string())
    } else {
        ("", toml_list(&opts.owners))
    };
    let required = if opts.required.is_empty() {
        toml_list(&["priority".to_string()])
    } else {
        toml_list(&opts.required)
    };
    let (lpre, labels) = if opts.labels.is_empty() {
        ("# ", "\"bug\", \"feature\", \"docs\"".to_string())
    } else {
        ("", toml_list(&opts.labels))
    };
    format!(
        r#"# plane-cli write policy for project {project}.
# Written by `plane-cli config policy init`; check it with `plane-cli config policy show --project {project}`.
# plane-cli refuses a write that breaks a rule (error.code "policy_violation", exit 2).
# A section that is absent or commented out is not enforced.
version = 1

# Before `work-items create`, search the project for the new name and refuse a close match
# unless --allow-duplicate is passed.
[search_before_create]
threshold = 0.85

# A written description needs these sections, as headings in description_html
# (<h3>What we want</h3>, or a paragraph that is only bold text). "A|B" accepts either.
# A single plain paragraph (what --description writes) is refused.
[description]
required_sections = ["What is wrong|What we want", "What to do", "Done when"]
forbid_single_paragraph = true

# Fields `work-items create` must set. One or more of: priority, module (--module),
# labels, assignees, state, start_date, target_date.
[required_fields]
on_create = [{required}]

# Labels a work item may carry (names, case-insensitive).
{lpre}[labels]
{lpre}allowed = [{labels}]

# Changing these fields needs --reason; plane-cli posts the reason as a comment in the same run.
[comment_on_change]
fields = ["state", "assignees", "priority"]

# Only these members may move a work item into these states (on create or update).
# Members are Plane display names (the email local part only when the display name is
# empty); without --as the caller is the API key's own member.
{pre}[[state_rules]]
{pre}states = ["Todo", "Done", "Cancelled"]
{pre}members = [{members}]

# Only these members may delete work items, comments, links or attachments in this project,
# or archive the project or its modules.
{pre}[deletes]
{pre}members = [{members}]
"#
    )
}

// ----- pure rule checks -----

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Pass,
    Fail,
}

/// One rule's verdict on one write.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    pub rule: &'static str,
    pub result: Outcome,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl Check {
    pub fn pass(rule: &'static str, message: impl Into<String>) -> Self {
        Check {
            rule,
            result: Outcome::Pass,
            message: message.into(),
            fix: None,
        }
    }
    pub fn fail(rule: &'static str, message: impl Into<String>, fix: impl Into<String>) -> Self {
        Check {
            rule,
            result: Outcome::Fail,
            message: message.into(),
            fix: Some(fix.into()),
        }
    }
    pub fn failed(&self) -> bool {
        self.result == Outcome::Fail
    }
}

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("static regex"))
}

fn strip_tags(s: &str) -> String {
    static TAGS: OnceLock<Regex> = OnceLock::new();
    let t = re(&TAGS, r"<[^>]*>").replace_all(s, "");
    t.replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn norm_heading(s: &str) -> String {
    s.trim()
        .trim_end_matches(|c: char| c == ':' || c.is_whitespace())
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Section headings in a description: `<h1>`-`<h6>` text and paragraphs that are only bold.
pub fn description_headings(html: &str) -> Vec<String> {
    static HEAD: OnceLock<Regex> = OnceLock::new();
    static BOLD_P: OnceLock<Regex> = OnceLock::new();
    let mut out = Vec::new();
    for c in re(&HEAD, r"(?is)<h[1-6][^>]*>(.*?)</h[1-6]>").captures_iter(html) {
        out.push(norm_heading(&strip_tags(&c[1])));
    }
    for c in re(
        &BOLD_P,
        r"(?is)<p[^>]*>\s*<(?:strong|b)>(.*?)</(?:strong|b)>\s*:?\s*</p>",
    )
    .captures_iter(html)
    {
        out.push(norm_heading(&strip_tags(&c[1])));
    }
    out
}

/// Block elements (paragraphs, headings, lists, code, quotes, tables) in a description.
pub fn block_count(html: &str) -> usize {
    static BLOCK: OnceLock<Regex> = OnceLock::new();
    re(&BLOCK, r"(?i)<(?:p|h[1-6]|ul|ol|pre|blockquote|table)\b")
        .find_iter(html)
        .count()
}

/// `description_shape` for a description being written (`None` on create = no description).
pub fn check_description(rule: &DescriptionRule, html: Option<&str>) -> Option<Check> {
    let fix_sections = || {
        let names: Vec<String> = rule
            .required_sections
            .iter()
            .map(|s| s.split('|').next().unwrap_or(s).trim().to_string())
            .collect();
        format!(
            "send the description as HTML through --body with description_html, one <h3> per section ({}), then a paragraph or list under each",
            names.join(", ")
        )
    };
    let html = html.map(str::trim).unwrap_or("");
    if html.is_empty() || strip_tags(html).trim().is_empty() {
        if rule.required_sections.is_empty() && !rule.forbid_single_paragraph {
            return None;
        }
        return Some(Check::fail(
            RULE_DESCRIPTION,
            "the work item has no description",
            fix_sections(),
        ));
    }
    let headings = description_headings(html);
    let missing: Vec<&str> = rule
        .required_sections
        .iter()
        .filter(|req| {
            !req.split('|').any(|alt| {
                let alt = norm_heading(alt);
                headings.iter().any(|h| h.starts_with(&alt))
            })
        })
        .map(String::as_str)
        .collect();
    if rule.forbid_single_paragraph && block_count(html) <= 1 && headings.is_empty() {
        return Some(Check::fail(
            RULE_DESCRIPTION,
            "the description is a single plain paragraph",
            fix_sections(),
        ));
    }
    if !missing.is_empty() {
        return Some(Check::fail(
            RULE_DESCRIPTION,
            format!(
                "the description is missing section(s): {}",
                missing.join(", ")
            ),
            fix_sections(),
        ));
    }
    Some(Check::pass(
        RULE_DESCRIPTION,
        "the description has the required sections",
    ))
}

/// Facts about a create body that `required_fields` needs.
#[derive(Debug, Default, Clone)]
pub struct CreateFacts {
    pub priority: Option<String>,
    pub module: bool,
    pub labels: usize,
    pub assignees: usize,
    pub state: bool,
    pub target_date: bool,
    pub start_date: bool,
}

impl CreateFacts {
    pub fn from_body(body: &Value, module: bool) -> Self {
        let s = |k: &str| {
            body.get(k)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(String::from)
        };
        let n = |k: &str| body.get(k).and_then(Value::as_array).map_or(0, Vec::len);
        CreateFacts {
            priority: s("priority"),
            module,
            labels: n("labels"),
            assignees: n("assignees"),
            state: s("state").is_some(),
            target_date: s("target_date").is_some(),
            start_date: s("start_date").is_some(),
        }
    }
}

pub fn check_required(rule: &RequiredFields, facts: &CreateFacts) -> Option<Check> {
    if rule.on_create.is_empty() {
        return None;
    }
    let missing: Vec<&str> = rule
        .on_create
        .iter()
        .map(String::as_str)
        .filter(|f| match *f {
            "priority" => facts.priority.as_deref().is_none_or(|p| p == "none"),
            "module" => !facts.module,
            "labels" => facts.labels == 0,
            "assignees" => facts.assignees == 0,
            "state" => !facts.state,
            "target_date" => !facts.target_date,
            "start_date" => !facts.start_date,
            _ => false,
        })
        .collect();
    if missing.is_empty() {
        return Some(Check::pass(
            RULE_REQUIRED,
            format!("sets {}", rule.on_create.join(", ")),
        ));
    }
    let hints: Vec<String> = missing
        .iter()
        .map(|f| match *f {
            "priority" => "--priority urgent|high|medium|low".to_string(),
            "module" => "--module <MODULE> (UUID or exact name)".to_string(),
            "labels" => "\"labels\": [...] in --body (with --strict-catalog for names)".to_string(),
            "assignees" => "--assignee <UUID>".to_string(),
            "state" => "--state <STATE>".to_string(),
            other => format!("\"{other}\": \"YYYY-MM-DD\" in --body"),
        })
        .collect();
    Some(Check::fail(
        RULE_REQUIRED,
        format!("create is missing {}", missing.join(", ")),
        format!("add {}", hints.join("; ")),
    ))
}

pub fn check_labels(rule: &LabelsRule, names: &[String]) -> Option<Check> {
    if names.is_empty() {
        return None;
    }
    let bad: Vec<&str> = names
        .iter()
        .filter(|n| {
            !rule
                .allowed
                .iter()
                .any(|a| a.eq_ignore_ascii_case(n.trim()))
        })
        .map(String::as_str)
        .collect();
    if bad.is_empty() {
        return Some(Check::pass(RULE_LABELS, "labels are from the allowed set"));
    }
    Some(Check::fail(
        RULE_LABELS,
        format!("label(s) not allowed: {}", bad.join(", ")),
        if rule.allowed.is_empty() {
            "remove the labels; this project's policy allows none".to_string()
        } else {
            format!("use only: {}", rule.allowed.join(", "))
        },
    ))
}

pub fn check_comment_on_change(
    rule: &CommentOnChange,
    changed: &[&str],
    reason: Option<&str>,
) -> Option<Check> {
    let watched: Vec<&str> = changed
        .iter()
        .copied()
        .filter(|f| rule.fields.iter().any(|w| w == f))
        .collect();
    if watched.is_empty() {
        return None;
    }
    if reason.is_some_and(|r| !r.trim().is_empty()) {
        return Some(Check::pass(
            RULE_COMMENT,
            format!(
                "changes {}; --reason will be posted as a comment",
                watched.join(", ")
            ),
        ));
    }
    Some(Check::fail(
        RULE_COMMENT,
        format!("this changes {} without --reason", watched.join(", ")),
        "add --reason \"<what changed and why>\"; plane-cli posts it as a comment in the same run",
    ))
}

/// `state_permissions` when the work item moves into `state_name` (in `state_group`). Every
/// rule that covers the state must list the caller. `caller` is the acting member's
/// `users/me` record.
pub fn check_state(
    rules: &[StateRule],
    state_name: &str,
    state_group: Option<&str>,
    caller: &Value,
    caller_label: &str,
) -> Option<Check> {
    let covering: Vec<&StateRule> = rules
        .iter()
        .filter(|r| r.covers(state_name, state_group))
        .collect();
    if covering.is_empty() {
        return None;
    }
    let allowed = |r: &&StateRule| {
        r.members
            .iter()
            .any(|m| crate::identity::user_matches_member(m, caller))
    };
    if covering.iter().all(allowed) {
        return Some(Check::pass(
            RULE_STATES,
            format!("{caller_label} may set {state_name}"),
        ));
    }
    // Members listed in every covering rule.
    let mut who: Vec<String> = covering[0].members.clone();
    for r in &covering[1..] {
        who.retain(|m| r.members.iter().any(|x| x.eq_ignore_ascii_case(m)));
    }
    let who_text = if who.is_empty() {
        "nobody".to_string()
    } else {
        who.join(", ")
    };
    Some(Check::fail(
        RULE_STATES,
        format!("{caller_label} may not set state {state_name}; only {who_text} may"),
        if who.is_empty() {
            "pick another state; no member may set this one through plane-cli".to_string()
        } else {
            format!(
                "pick another state, or leave a comment asking {} to move it",
                who.join(" or ")
            )
        },
    ))
}

pub fn check_delete(rule: &DeletesRule, what: &str, caller: &Value, caller_label: &str) -> Check {
    check_removal(rule, "delete", what, caller, caller_label)
}

/// `[deletes]` also covers archiving a project or module: it hides their work items from
/// the active lists, so it is treated like a delete.
pub fn check_archive(rule: &DeletesRule, what: &str, caller: &Value, caller_label: &str) -> Check {
    check_removal(rule, "archive", what, caller, caller_label)
}

fn check_removal(
    rule: &DeletesRule,
    verb: &str,
    what: &str,
    caller: &Value,
    caller_label: &str,
) -> Check {
    if rule
        .members
        .iter()
        .any(|m| crate::identity::user_matches_member(m, caller))
    {
        return Check::pass(
            RULE_DELETES,
            format!("{caller_label} may {verb} in this project"),
        );
    }
    Check::fail(
        RULE_DELETES,
        format!("{caller_label} may not {verb} this {what}"),
        if rule.members.is_empty() {
            format!("{verb}s are turned off for this project; leave a comment instead")
        } else {
            format!(
                "leave a comment asking {} to {verb} it",
                rule.members.join(" or ")
            )
        },
    )
}

/// Lower-case alphanumeric words joined by single spaces.
pub fn normalize_name(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Words that do not make two names different ("fix the login page" = "fix login page").
const FILLER_WORDS: &[&str] = &["a", "an", "the", "to", "of", "for", "in", "on", "and", "or"];

/// Name words in order, without filler words.
fn name_tokens(s: &str) -> Vec<String> {
    normalize_name(s)
        .split_whitespace()
        .filter(|w| !FILLER_WORDS.contains(w))
        .map(String::from)
        .collect()
}

/// Numbers in a name, in order, with their separators kept ("0.2.0", "2030-11-14", "v2").
fn number_groups(s: &str) -> Vec<String> {
    static NUM: OnceLock<Regex> = OnceLock::new();
    re(&NUM, r"\d+(?:[.:/-]\d+)*")
        .find_iter(s)
        .map(|m| m.as_str().to_string())
        .collect()
}

fn has_digit(w: &str) -> bool {
    w.chars().any(|c| c.is_ascii_digit())
}

/// Two words count as the same when equal, or (both 4+ letters, no digits) one typo apart:
/// Damerau-Levenshtein similarity of at least 0.8 ("logn" = "login", "enable" != "disable").
fn same_word(a: &str, b: &str) -> bool {
    a == b
        || (!has_digit(a)
            && !has_digit(b)
            && a.chars().count() >= 4
            && b.chars().count() >= 4
            && strsim::normalized_damerau_levenshtein(a, b) >= 0.8)
}

/// 0-1 similarity of two work-item names: the words both names share (as a multiset, so a
/// repeated word counts once per occurrence, and word order does not matter) over the longer
/// name's word count. Names whose numbers differ, in value or order ("0.2.0" / "0.2.1",
/// "0.2.0" / "2.0.0", "ADR 0001" / "ADR 0002", two dates), never match.
pub fn name_similarity(a: &str, b: &str) -> f64 {
    let (ta, tb) = (name_tokens(a), name_tokens(b));
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    if number_groups(a) != number_groups(b) {
        return 0.0;
    }
    let mut unused: Vec<&String> = tb.iter().collect();
    let mut shared = 0usize;
    for w in &ta {
        if let Some(i) = unused.iter().position(|u| same_word(w, u)) {
            unused.remove(i);
            shared += 1;
        }
    }
    shared as f64 / ta.len().max(tb.len()) as f64
}

/// Queries for the duplicate search. CE 1.4.2's work-item search is a substring match on the
/// name, so "Fix login - Safari" would miss "Fix login: Safari"; search the full name, then
/// the two longest distinct words (4+ letters) of the normalized name.
pub fn search_queries(name: &str) -> Vec<String> {
    let mut out = vec![name.trim().to_string()];
    let norm = normalize_name(name);
    let mut words: Vec<&str> = norm
        .split_whitespace()
        .filter(|w| w.chars().count() >= 4 && !FILLER_WORDS.contains(w))
        .collect();
    words.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
    words.dedup();
    for w in words.into_iter().take(2) {
        if !out.iter().any(|q| q.eq_ignore_ascii_case(w)) {
            out.push(w.to_string());
        }
    }
    out
}

/// `search_before_create` verdict from the open close matches (`{work_item, name,
/// similarity}`; Done/Cancelled matches are reported separately and never refuse).
pub fn duplicate_check(open: &[Value], allow_duplicate: bool) -> Check {
    if open.is_empty() {
        return Check::pass(RULE_SEARCH, "no open work item has a close name");
    }
    let ids: Vec<&str> = open
        .iter()
        .filter_map(|m| m["work_item"].as_str())
        .collect();
    if allow_duplicate {
        return Check::pass(
            RULE_SEARCH,
            format!(
                "close match(es) {} allowed by --allow-duplicate",
                ids.join(", ")
            ),
        );
    }
    Check::fail(
        RULE_SEARCH,
        format!("close match(es) already exist: {}", ids.join(", ")),
        format!(
            "update {} instead, or pass --allow-duplicate if this is different work",
            ids.first().copied().unwrap_or("the existing item")
        ),
    )
}

/// Close matches among `(composite id, name)` candidates at `rule.threshold`.
pub fn close_matches(rule: &SearchRule, name: &str, candidates: &[(String, String)]) -> Vec<Value> {
    candidates
        .iter()
        .filter_map(|(id, n)| {
            let score = name_similarity(name, n);
            (score >= rule.threshold).then(|| {
                json!({"work_item": id, "name": n, "similarity": (score * 100.0).round() / 100.0})
            })
        })
        .collect()
}

/// `policy_violation` error for the failed checks (exit 2).
pub fn violation_error(loaded: &LoadedPolicy, action: &str, checks: &[Check]) -> CoreError {
    let failed: Vec<&Check> = checks.iter().filter(|c| c.failed()).collect();
    let first = failed[0];
    let message = format!(
        "policy for {} refused {action}: {}: {}",
        loaded.project, first.rule, first.message
    );
    CoreError::usage_code("policy_violation", message).with_details(
        "",
        json!({
            "rule": first.rule,
            "fix": first.fix,
            "violations": failed,
            "project": loaded.project,
            "policy_file": loaded.file().path.display().to_string(),
            "policy_files": loaded.files_json(),
            "hint": format!("Rules: `plane-cli --json config policy show --project {}`; preview a write with --dry-run.", loaded.project),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tp_rule() -> DescriptionRule {
        DescriptionRule {
            required_sections: vec![
                "What is wrong|What we want".into(),
                "What to do".into(),
                "Done when".into(),
            ],
            forbid_single_paragraph: true,
        }
    }

    #[test]
    fn template_parses_with_and_without_owners() {
        let path = Path::new("WEB.toml");
        let p = Policy::parse(&template("WEB", &TemplateOptions::default()), path).unwrap();
        assert!(p.state_rules.is_empty() && p.deletes.is_none());
        assert!(p.search_before_create.is_some());
        let opts = TemplateOptions {
            owners: vec!["alice".into()],
            required: vec!["priority".into(), "module".into(), "labels".into()],
            labels: vec!["bug".into(), "say \"hi\"".into()],
        };
        let p = Policy::parse(&template("WEB", &opts), path).unwrap();
        assert_eq!(p.state_rules[0].members, vec!["alice"]);
        assert_eq!(p.required_fields.unwrap().on_create.len(), 3);
        assert_eq!(p.labels.unwrap().allowed[1], "say \"hi\"");
        assert_eq!(p.deletes.unwrap().members, vec!["alice"]);
    }

    #[test]
    fn invalid_policies_are_refused() {
        let path = Path::new("X.toml");
        let e = Policy::parse("version = 1\n[nope]\n", path).unwrap_err();
        assert_eq!(e.error_code(), "policy_invalid");
        let e = Policy::parse("version = 2\n", path).unwrap_err();
        assert_eq!(e.error_code(), "policy_invalid");
        let e = Policy::parse(
            "version = 1\n[required_fields]\non_create=[\"colour\"]\n",
            path,
        )
        .unwrap_err();
        assert!(e.to_string().contains("colour"));
        let e = Policy::parse(
            "version = 1\n[[state_rules]]\nstates=[]\nmembers=[\"j\"]\n",
            path,
        )
        .unwrap_err();
        assert_eq!(e.error_code(), "policy_invalid");
    }

    #[test]
    fn description_shape_refuses_paragraph_and_missing_sections() {
        let r = tp_rule();
        let c = check_description(&r, Some("<p>Fix the login page.</p>")).unwrap();
        assert!(c.failed() && c.message.contains("single plain paragraph"));
        let c = check_description(&r, Some("Just text")).unwrap();
        assert!(c.failed());
        let c = check_description(&r, None).unwrap();
        assert!(c.failed() && c.message.contains("no description"));
        let c = check_description(
            &r,
            Some("<h3>What we want</h3><p>a</p><h3>What to do</h3><ul><li>b</li></ul>"),
        )
        .unwrap();
        assert!(c.failed() && c.message.contains("Done when"));
        let good = "<h3>What is wrong</h3><p>a</p><p><strong>What to do:</strong></p><ul><li>b</li></ul><h2>Done when</h2><p>c</p>";
        assert!(!check_description(&r, Some(good)).unwrap().failed());
    }

    #[test]
    fn required_fields_and_labels() {
        let rule = RequiredFields {
            on_create: vec!["priority".into(), "module".into(), "labels".into()],
        };
        let body = json!({"name": "x", "priority": "none", "labels": []});
        let c = check_required(&rule, &CreateFacts::from_body(&body, false)).unwrap();
        assert!(c.failed());
        assert!(c.message.contains("priority, module, labels"));
        let body = json!({"name": "x", "priority": "high", "labels": ["bug"]});
        assert!(!check_required(&rule, &CreateFacts::from_body(&body, true))
            .unwrap()
            .failed());
        let lr = LabelsRule {
            allowed: vec!["bug".into(), "feature".into()],
        };
        assert!(check_labels(&lr, &["Bug".into()]).is_some_and(|c| !c.failed()));
        assert!(check_labels(&lr, &["bug".into(), "random".into()]).is_some_and(|c| c.failed()));
        assert!(check_labels(&lr, &[]).is_none());
        // A present [labels] section with an empty list allows no labels.
        let none = LabelsRule { allowed: vec![] };
        assert!(check_labels(&none, &["bug".into()]).is_some_and(|c| c.failed()));
    }

    #[test]
    fn comment_on_change_needs_reason() {
        let rule = CommentOnChange {
            fields: vec!["state".into(), "priority".into()],
        };
        assert!(check_comment_on_change(&rule, &["name"], None).is_none());
        assert!(check_comment_on_change(&rule, &["state"], None).is_some_and(|c| c.failed()));
        assert!(check_comment_on_change(&rule, &["state"], Some("  ")).is_some_and(|c| c.failed()));
        assert!(
            check_comment_on_change(&rule, &["priority"], Some("raised: blocks release"))
                .is_some_and(|c| !c.failed())
        );
    }

    #[test]
    fn state_and_delete_rules_follow_identity() {
        let rules = vec![StateRule {
            states: vec!["Todo".into(), "Done".into()],
            groups: vec![],
            members: vec!["alice".into()],
        }];
        let alice = json!({"display_name": "alice", "email": "alice@example.com"});
        let bob = json!({"display_name": "bob", "email": "bob@agents.local"});
        let st = |name, group, who: &Value, label| check_state(&rules, name, group, who, label);
        assert!(st("In Progress", Some("started"), &bob, "bob").is_none());
        assert!(st("done", Some("completed"), &bob, "bob").is_some_and(|c| c.failed()));
        assert!(st("Done", Some("completed"), &alice, "alice").is_some_and(|c| !c.failed()));
        // Match rule: the email local part counts only when display_name is empty.
        let disguised = json!({"display_name": "bob", "email": "alice@example.com"});
        assert!(st("Done", None, &disguised, "bob").is_some_and(|c| c.failed()));
        let no_display = json!({"display_name": "", "email": "alice@example.com"});
        assert!(st("Done", None, &no_display, "alice").is_some_and(|c| !c.failed()));
        let d = DeletesRule {
            members: vec!["alice".into()],
        };
        assert!(check_delete(&d, "work item", &bob, "bob").failed());
        assert!(!check_delete(&d, "work item", &alice, "alice").failed());
        assert!(check_delete(&DeletesRule::default(), "comment", &alice, "alice").failed());
    }

    #[test]
    fn state_rules_by_group_and_overlapping_rules_intersect() {
        let rules = vec![
            StateRule {
                states: vec![],
                groups: vec!["completed".into()],
                members: vec!["alice".into(), "ci-bot".into()],
            },
            StateRule {
                states: vec!["Shipped".into()],
                groups: vec![],
                members: vec!["ci-bot".into()],
            },
        ];
        let alice = json!({"display_name": "alice"});
        let ci_bot = json!({"display_name": "ci-bot"});
        // A renamed completed state is covered through its group.
        assert!(
            check_state(&rules, "Finished", Some("completed"), &alice, "alice")
                .is_some_and(|c| !c.failed())
        );
        // Covered by both rules: only members listed in both may set it.
        let c = check_state(&rules, "Shipped", Some("completed"), &alice, "alice").unwrap();
        assert!(c.failed() && c.message.contains("only ci-bot"), "{c:?}");
        assert!(
            check_state(&rules, "Shipped", Some("completed"), &ci_bot, "ci-bot")
                .is_some_and(|c| !c.failed())
        );
        let path = Path::new("X.toml");
        let e = Policy::parse(
            "version = 1\n[[state_rules]]\ngroups=[\"done\"]\nmembers=[\"j\"]\n",
            path,
        )
        .unwrap_err();
        assert!(e.to_string().contains("unknown group"));
        assert!(Policy::parse(
            "version = 1\n[[state_rules]]\ngroups=[\"completed\"]\nmembers=[\"j\"]\n",
            path
        )
        .is_ok());
    }

    #[test]
    fn search_queries_add_the_longest_words() {
        assert_eq!(
            search_queries("Policy live check: sandbox item"),
            vec!["Policy live check: sandbox item", "sandbox", "policy"]
        );
        assert_eq!(search_queries("Fix it"), vec!["Fix it"]);
    }

    #[test]
    fn duplicates_by_similarity() {
        let rule = SearchRule { threshold: 0.85 };
        let cands = vec![
            ("WEB-1".to_string(), "Fix login page".to_string()),
            ("WEB-2".to_string(), "Add billing export".to_string()),
        ];
        let m = close_matches(&rule, "fix the login page", &cands);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0]["work_item"], "WEB-1");
        assert!(duplicate_check(&m, false).failed());
        assert!(!duplicate_check(&m, true).failed());
        let m = close_matches(&rule, "Write release notes", &cands);
        assert!(m.is_empty() && !duplicate_check(&m, false).failed());
    }

    #[test]
    fn similarity_pairs() {
        // (a, b, counts as a duplicate at 0.85)
        let pairs = [
            ("Bump version to 0.2.0", "Bump version to 0.2.1", false),
            ("Bump version to 0.2.0", "Bump version to 2.0.0", false),
            ("Write ADR 0001", "Write ADR 0002", false),
            ("Enable cycles in ENG", "Disable cycles in ENG", false),
            ("Add login page", "Remove login page", false),
            (
                "Weekly report 2030-11-14",
                "Weekly report 2030-11-21",
                false,
            ),
            ("Fix login on Safari", "Fix login on Firefox", false),
            ("Migrate DB to new host", "Migrate VM to new host", false),
            ("Login broken on Safari", "Safari: login broken", true),
            ("Fix login page", "Fix the login page", true),
            ("Fix logn page", "Fix login page", true),
        ];
        for (a, b, dup) in pairs {
            let score = name_similarity(a, b);
            eprintln!("{score:.3}  {a:?} vs {b:?}");
            assert_eq!(score >= 0.85, dup, "{a:?} vs {b:?}: {score}");
        }
        // Repeated words count per occurrence (multiset, not set).
        assert!(name_similarity("test test login", "test login") < 0.85);
    }

    fn write_policy(dir: &Path, text: &str) -> PathBuf {
        let d = dir.join(".plane-cli").join("policy");
        std::fs::create_dir_all(&d).unwrap();
        let f = d.join("WEB.toml");
        std::fs::write(&f, text).unwrap();
        f
    }

    #[test]
    fn local_search_stops_at_repo_root_or_home() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let sub = repo.join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let dirs = local_search_dirs(&sub, None);
        assert_eq!(dirs.last().unwrap(), &repo);
        assert!(!dirs.contains(&tmp.path().to_path_buf()));
        // No repo: stop at home.
        let home = tmp.path().join("home");
        let deep = home.join("x").join("y");
        std::fs::create_dir_all(&deep).unwrap();
        let dirs = local_search_dirs(&deep, Some(&home));
        assert_eq!(dirs.last().unwrap(), &home);
        // Neither above: only the start directory (never up to `/`).
        let loose = tmp.path().join("loose").join("dir");
        std::fs::create_dir_all(&loose).unwrap();
        assert_eq!(local_search_dirs(&loose, Some(&home)), vec![loose.clone()]);
        assert!(normalize_identifier("web").is_ok_and(|p| p == "WEB"));
        assert!(normalize_identifier("211751d8-8fc6").is_err());
    }

    #[test]
    fn local_policy_only_tightens() {
        let global = Policy::parse(
            &template(
                "WEB",
                &TemplateOptions {
                    owners: vec!["alice".into()],
                    required: vec!["priority".into()],
                    labels: vec!["bug".into(), "feature".into()],
                },
            ),
            Path::new("g.toml"),
        )
        .unwrap();
        // A local file with only `version = 1` must not replace the
        // global policy (every rule gone). Merged, nothing is dropped.
        let empty = Policy::parse("version = 1\n", Path::new("l.toml")).unwrap();
        assert_eq!(global.clone().tighten_with(&empty), global);
        let local = Policy::parse(
            r#"version = 1
[search_before_create]
threshold = 0.99
[description]
required_sections = ["References"]
forbid_single_paragraph = false
[required_fields]
on_create = ["module"]
[labels]
allowed = ["feature", "docs"]
[comment_on_change]
fields = ["state"]
[[state_rules]]
states = ["Done"]
members = ["bob"]
[deletes]
members = ["alice", "bob"]
"#,
            Path::new("l.toml"),
        )
        .unwrap();
        let m = global.clone().tighten_with(&local);
        assert_eq!(
            m.search_before_create.unwrap().threshold,
            0.85,
            "stricter = lower"
        );
        let d = m.description.unwrap();
        assert!(d.forbid_single_paragraph);
        assert_eq!(d.required_sections.len(), 4);
        assert_eq!(
            m.required_fields.unwrap().on_create,
            vec!["priority", "module"]
        );
        assert_eq!(m.labels.unwrap().allowed, vec!["feature"]);
        assert_eq!(m.comment_on_change.unwrap().fields.len(), 3);
        assert_eq!(m.state_rules.len(), 2);
        assert_eq!(m.deletes.unwrap().members, vec!["alice"]);
        // Done is now covered by alice's and bob's rules: nobody is in both.
        let alice = json!({"display_name": "alice"});
        let merged = global.tighten_with(&local);
        assert!(
            check_state(&merged.state_rules, "Done", None, &alice, "alice")
                .is_some_and(|c| c.failed())
        );
    }

    #[cfg(unix)]
    #[test]
    fn untrusted_local_files_are_reported() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let f = write_policy(tmp.path(), "version = 1\n");
        for p in [
            &f,
            f.parent().unwrap(),
            f.parent().unwrap().parent().unwrap(),
        ] {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(untrusted_reason(&f), None);
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o666)).unwrap();
        assert!(untrusted_reason(&f).unwrap().contains("writable"));
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::set_permissions(f.parent().unwrap(), std::fs::Permissions::from_mode(0o777))
            .unwrap();
        assert!(untrusted_reason(&f).unwrap().contains("writable"));
    }
}
