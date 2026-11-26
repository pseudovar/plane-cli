//! Name lookup for cycles and modules, which live in an active list and an archived list.
//!
//! One set of matching rules for every `--cycle` / `--module` name:
//! - exact matches (after trimming) beat case-insensitive ones, across both lists;
//! - two or more matches at the winning level are ambiguous, even when one is active and
//!   one archived, and the error names every id;
//! - commands pick which lists to search with [`NameScope`] (`archive` searches active
//!   only, `unarchive` archived only).

use crate::error::CoreError;
use serde_json::Value;
use uuid::Uuid;

/// Which lists a command searches by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameScope {
    /// Active rows only (e.g. `archive`, adding work items to a module).
    Active,
    /// Archived rows only (`unarchive`).
    Archived,
    /// Both lists (reads, updates and deletes that work on archived rows too).
    Either,
}

impl NameScope {
    pub fn active(self) -> bool {
        matches!(self, NameScope::Active | NameScope::Either)
    }
    pub fn archived(self) -> bool {
        matches!(self, NameScope::Archived | NameScope::Either)
    }
}

struct Row {
    id: Uuid,
    name: String,
    archived: bool,
}

fn rows(list: Option<&Value>, archived: bool) -> Vec<Row> {
    let Some(list) = list else {
        return Vec::new();
    };
    let arr = list
        .get("results")
        .and_then(Value::as_array)
        .or_else(|| list.as_array());
    arr.map(|a| {
        a.iter()
            .filter_map(|r| {
                Some(Row {
                    id: Uuid::parse_str(r.get("id")?.as_str()?).ok()?,
                    name: r.get("name")?.as_str()?.to_string(),
                    archived,
                })
            })
            .collect()
    })
    .unwrap_or_default()
}

/// Finds the id named `raw` in the given lists (`None` for a list that was not searched).
/// `Ok(None)` when nothing matches; ambiguous names fail with every matching id.
pub fn match_named(
    active: Option<&Value>,
    archived: Option<&Value>,
    raw: &str,
    kind: &str,
) -> Result<Option<Uuid>, CoreError> {
    let wanted = raw.trim();
    let mut all = rows(active, false);
    all.extend(rows(archived, true));
    let mut hits: Vec<&Row> = all.iter().filter(|r| r.name.trim() == wanted).collect();
    if hits.is_empty() {
        hits = all
            .iter()
            .filter(|r| r.name.trim().eq_ignore_ascii_case(wanted))
            .collect();
    }
    match hits.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one.id)),
        many => Err(CoreError::Resolve(format!(
            "ambiguous {kind} name {wanted:?}: {} matches ({}); pass the UUID",
            many.len(),
            many.iter()
                .map(|r| format!(
                    "{} {:?}{}",
                    r.id,
                    r.name,
                    if r.archived { " archived" } else { " active" }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// "unknown …" error listing the names in the lists that were searched. `list_cmd` is the
/// command that lists them, e.g. `modules list --project <PROJECT>`.
pub fn unknown_name_error(
    active: Option<&Value>,
    archived: Option<&Value>,
    raw: &str,
    kind: &str,
    list_cmd: &str,
) -> CoreError {
    let names = |list: Option<&Value>, arch: bool| -> Vec<String> {
        rows(list, arch).into_iter().map(|r| r.name).collect()
    };
    let mut parts = Vec::new();
    if active.is_some() {
        let n = names(active, false);
        parts.push(if n.is_empty() {
            format!("no active {kind}s")
        } else {
            format!("active {kind}s: {}", n.join(", "))
        });
    }
    if archived.is_some() {
        let n = names(archived, true);
        parts.push(if n.is_empty() {
            format!("no archived {kind}s")
        } else {
            format!("archived {kind}s: {}", n.join(", "))
        });
    }
    CoreError::Resolve(format!(
        "unknown {kind} {raw:?}; {}. Use the {kind} UUID or exact name (`{list_cmd}`)",
        parts.join("; ")
    ))
}

/// Resolves `raw` (UUID, or name in the lists `scope` names). The fetchers are only called
/// for the lists searched, and not at all for a UUID.
pub fn resolve_named(
    raw: &str,
    kind: &str,
    scope: NameScope,
    list_cmd: &str,
    fetch_active: impl FnOnce() -> Result<Value, CoreError>,
    fetch_archived: impl FnOnce() -> Result<Value, CoreError>,
) -> Result<Uuid, CoreError> {
    if let Ok(u) = Uuid::parse_str(raw.trim()) {
        return Ok(u);
    }
    let active = if scope.active() {
        Some(fetch_active()?)
    } else {
        None
    };
    let archived = if scope.archived() {
        Some(fetch_archived()?)
    } else {
        None
    };
    match match_named(active.as_ref(), archived.as_ref(), raw, kind)? {
        Some(id) => Ok(id),
        None => Err(unknown_name_error(
            active.as_ref(),
            archived.as_ref(),
            raw,
            kind,
            list_cmd,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const A: &str = "11111111-1111-4111-8111-111111111111";
    const B: &str = "22222222-2222-4222-8222-222222222222";
    const C: &str = "33333333-3333-4333-8333-333333333333";

    fn list(rows: &[(&str, &str)]) -> Value {
        json!({"results": rows.iter().map(|(id, n)| json!({"id": id, "name": n})).collect::<Vec<_>>()})
    }

    fn never() -> Result<Value, CoreError> {
        panic!("list must not be fetched")
    }

    #[test]
    fn same_name_in_both_lists_is_ambiguous_with_both_ids() {
        let active = list(&[(A, "Q4")]);
        let archived = list(&[(B, "Q4")]);
        let e = match_named(Some(&active), Some(&archived), "Q4", "module").unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains(A) && msg.contains(B), "{msg}");
        assert!(msg.contains("active") && msg.contains("archived"), "{msg}");
        assert_eq!(e.exit_kind().code(), 2);
    }

    #[test]
    fn scope_limits_the_lists_searched() {
        let active = list(&[(A, "Q4")]);
        let archived = list(&[(B, "Q4")]);
        let id = resolve_named("Q4", "module", NameScope::Archived, "x", never, || {
            Ok(archived.clone())
        })
        .unwrap();
        assert_eq!(id.to_string(), B);
        let id = resolve_named(
            "Q4",
            "module",
            NameScope::Active,
            "x",
            || Ok(active.clone()),
            never,
        )
        .unwrap();
        assert_eq!(id.to_string(), A);
    }

    #[test]
    fn exact_match_beats_case_only_match_across_lists() {
        let active = list(&[(A, "sprint 1")]);
        let archived = list(&[(B, "Sprint 1")]);
        assert_eq!(
            match_named(Some(&active), Some(&archived), "Sprint 1", "cycle")
                .unwrap()
                .unwrap()
                .to_string(),
            B
        );
        assert_eq!(
            match_named(Some(&active), Some(&archived), "sprint 1", "cycle")
                .unwrap()
                .unwrap()
                .to_string(),
            A
        );
        // Only case-insensitive matches, one in each list: ambiguous.
        assert!(match_named(Some(&active), Some(&archived), "SPRINT 1", "cycle").is_err());
        // A single case-insensitive match resolves.
        let only = list(&[(C, "Launch")]);
        assert_eq!(
            match_named(Some(&only), None, "launch", "cycle")
                .unwrap()
                .unwrap()
                .to_string(),
            C
        );
    }

    #[test]
    fn unknown_name_lists_active_and_archived_names() {
        let active = list(&[(A, "Live")]);
        let archived = list(&[(B, "Old")]);
        let e = resolve_named(
            "nope",
            "module",
            NameScope::Either,
            "modules list --project <PROJECT> [--archived]",
            || Ok(active.clone()),
            || Ok(archived.clone()),
        )
        .unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("active modules: Live"), "{msg}");
        assert!(msg.contains("archived modules: Old"), "{msg}");
        let e = resolve_named(
            "nope",
            "module",
            NameScope::Active,
            "x",
            || Ok(list(&[])),
            never,
        )
        .unwrap_err();
        assert!(e.to_string().contains("no active modules"), "{e}");
        assert!(!e.to_string().contains("archived"), "{e}");
    }

    #[test]
    fn uuid_skips_every_fetch() {
        let id = resolve_named(A, "cycle", NameScope::Either, "x", never, never).unwrap();
        assert_eq!(id.to_string(), A);
    }
}
