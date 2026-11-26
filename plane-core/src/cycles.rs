//! Cycle helpers: status, summaries, date flags and name resolution.
//!
//! Behaviour checked against Plane Community Edition 1.4.2:
//! - `POST cycles/` needs `start_date` and `end_date` both set or both empty.
//! - The server keeps only the date part of each value and stores it in the project's
//!   time zone: start at 00:00:01 local, end at 23:59 local. The `timezone` field on
//!   the cycle does not change this.
//! - `PATCH` with only one of the two dates is accepted but stores the raw value with no
//!   time-zone conversion, so the CLI sends both or neither.

use crate::error::CoreError;
use crate::named::match_named;
use crate::settings::for_each_row;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Values the CE 1.4.2 `cycle_view` list parameter accepts (from `/api/schema/`).
pub const CYCLE_VIEWS: &[&str] = &[
    "all",
    "current",
    "upcoming",
    "completed",
    "draft",
    "incomplete",
];

/// Current UTC time as `YYYY-MM-DDTHH:MM:SS` (no suffix), comparable with the first
/// 19 characters of Plane's UTC timestamps.
pub fn now_utc_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_utc(secs)
}

fn format_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 to (year, month, day), proleptic Gregorian (H. Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn ts_prefix(v: &Value, key: &str) -> Option<String> {
    let s = v.get(key)?.as_str()?;
    if s.len() >= 19 {
        Some(s[..19].to_string())
    } else if s.len() == 10 {
        Some(format!("{s}T00:00:00"))
    } else {
        None
    }
}

/// `draft` (no dates), `upcoming`, `current` or `completed`, judged against `now`
/// (`YYYY-MM-DDTHH:MM:SS` UTC). Plane's v1 API does not return a status field.
pub fn cycle_status(cycle: &Value, now: &str) -> &'static str {
    let start = ts_prefix(cycle, "start_date");
    let end = ts_prefix(cycle, "end_date");
    match (start, end) {
        (Some(s), Some(e)) => {
            if now < s.as_str() {
                "upcoming"
            } else if now > e.as_str() {
                "completed"
            } else {
                "current"
            }
        }
        _ => "draft",
    }
}

/// `{ id, name, start_date, end_date, status, archived }` rows from a cycles list response.
pub fn cycle_summaries(cycles_list: &Value, now: &str) -> Vec<Value> {
    let Some(arr) = cycles_list.get("results").and_then(|x| x.as_array()) else {
        return Vec::new();
    };
    let mut rows: Vec<Value> = arr
        .iter()
        .filter_map(|c| {
            let id = c.get("id")?.as_str()?;
            Some(json!({
                "id": id,
                "name": c.get("name").and_then(|x| x.as_str()).unwrap_or(""),
                "start_date": c.get("start_date").cloned().unwrap_or(Value::Null),
                "end_date": c.get("end_date").cloned().unwrap_or(Value::Null),
                "status": cycle_status(c, now),
                "archived": c.get("archived_at").map(|x| !x.is_null()).unwrap_or(false),
            }))
        })
        .collect();
    rows.sort_by(|a, b| {
        let ka = a.get("start_date").and_then(|x| x.as_str()).unwrap_or("~");
        let kb = b.get("start_date").and_then(|x| x.as_str()).unwrap_or("~");
        ka.cmp(kb).then_with(|| {
            let na = a.get("name").and_then(|x| x.as_str()).unwrap_or("");
            let nb = b.get("name").and_then(|x| x.as_str()).unwrap_or("");
            na.cmp(nb)
        })
    });
    rows
}

/// Adds a computed `status` to each row of a cycles list response (in place). Handles the
/// paginated object and a bare array.
pub fn annotate_cycle_statuses(cycles_list: &mut Value, now: &str) {
    for_each_row(cycles_list, |c| {
        let status = cycle_status(c, now);
        if let Some(obj) = c.as_object_mut() {
            obj.entry("status").or_insert_with(|| json!(status));
        }
    });
}

/// Validates a `--start` / `--end` flag (`YYYY-MM-DD`) and returns the date-time string
/// sent to Plane. Rejects timestamps: Plane keeps only the date part, so a UTC timestamp
/// copied from output (`2030-11-30T14:00:01Z`) would land a day early in a UTC+ zone.
pub fn cycle_date_flag(flag: &str, raw: &str) -> Result<String, CoreError> {
    let s = raw.trim();
    let bad = || {
        CoreError::Usage(format!(
            "{flag} {raw:?} must be a date in YYYY-MM-DD form; Plane applies the project's time zone itself (start 00:00, end 23:59)"
        ))
    };
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return Err(bad());
    }
    let num = |r: std::ops::Range<usize>| -> Option<u32> {
        let part = &s[r];
        if part.bytes().all(|c| c.is_ascii_digit()) {
            part.parse().ok()
        } else {
            None
        }
    };
    let (Some(y), Some(m), Some(d)) = (num(0..4), num(5..7), num(8..10)) else {
        return Err(bad());
    };
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let max_day = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(bad()),
    };
    if d == 0 || d > max_day {
        return Err(bad());
    }
    Ok(format!("{s}T00:00:00"))
}

/// Finds a cycle id in a list response by exact name, falling back to a unique
/// case-insensitive match. `None` when nothing matches.
pub fn find_cycle_by_name(cycles_list: &Value, name: &str) -> Result<Option<Uuid>, CoreError> {
    match_named(Some(cycles_list), None, name, "cycle")
}

/// Cycle names from a list response, for "unknown cycle" messages.
pub fn cycle_names(cycles_list: &Value) -> Vec<String> {
    cycles_list
        .get("results")
        .and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|c| c.get("name").and_then(|x| x.as_str()).map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_epoch_and_leap_day() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00");
        // 2024-02-29T12:34:56Z
        assert_eq!(format_utc(1_709_210_096), "2024-02-29T12:34:56");
        // 2030-12-24T13:59:00Z
        assert_eq!(format_utc(1_924_351_140), "2030-12-24T13:59:00");
    }

    #[test]
    fn status_from_dates() {
        let now = "2030-11-21T03:00:00";
        let c = |s: Value, e: Value| json!({"start_date": s, "end_date": e});
        assert_eq!(cycle_status(&c(Value::Null, Value::Null), now), "draft");
        assert_eq!(
            cycle_status(
                &c(
                    json!("2030-11-20T06:03:30.936255Z"),
                    json!("2030-12-24T13:59:00Z")
                ),
                now
            ),
            "current"
        );
        assert_eq!(
            cycle_status(
                &c(json!("2030-11-30T14:00:01Z"), json!("2030-12-05T13:59:00Z")),
                now
            ),
            "upcoming"
        );
        assert_eq!(
            cycle_status(
                &c(json!("2030-11-01T14:00:01Z"), json!("2030-11-09T13:59:00Z")),
                now
            ),
            "completed"
        );
    }

    #[test]
    fn date_flag_accepts_dates_only() {
        assert_eq!(
            cycle_date_flag("--start", "2030-12-01").unwrap(),
            "2030-12-01T00:00:00"
        );
        assert!(cycle_date_flag("--end", "2028-02-29").is_ok());
        assert!(cycle_date_flag("--end", "2030-02-29").is_err());
        assert!(cycle_date_flag("--end", "2030-12-24T13:59:00Z").is_err());
        assert!(cycle_date_flag("--end", "24/12/2026").is_err());
        assert!(cycle_date_flag("--end", "2030-13-01").is_err());
    }

    #[test]
    fn name_lookup_exact_then_case_insensitive() {
        let list = json!({"results": [
            {"id": "04e9c942-56b1-44b9-8362-9aa9a99707ea", "name": "Sprint 12"},
            {"id": "23954da7-e4b2-463f-a421-b877f5bf132d", "name": "Sprint 1"},
            {"id": "a1bbf2a7-3096-42e3-817a-077338e70d6d", "name": "sprint 1"},
        ]});
        assert_eq!(
            find_cycle_by_name(&list, "sprint 12")
                .unwrap()
                .unwrap()
                .to_string(),
            "04e9c942-56b1-44b9-8362-9aa9a99707ea"
        );
        assert_eq!(
            find_cycle_by_name(&list, "Sprint 1")
                .unwrap()
                .unwrap()
                .to_string(),
            "23954da7-e4b2-463f-a421-b877f5bf132d"
        );
        assert!(find_cycle_by_name(&list, "SPRINT 1").is_err());
        assert!(find_cycle_by_name(&list, "Nope").unwrap().is_none());
    }

    #[test]
    fn summaries_sort_and_annotate() {
        let mut list = json!({"results": [
            {"id": "b", "name": "Draft", "start_date": null, "end_date": null, "archived_at": null},
            {"id": "a", "name": "Now", "start_date": "2030-11-20T06:03:30Z", "end_date": "2030-12-24T13:59:00Z", "archived_at": null},
        ]});
        let rows = cycle_summaries(&list, "2030-11-21T03:00:00");
        assert_eq!(rows[0]["name"], "Now");
        assert_eq!(rows[0]["status"], "current");
        assert_eq!(rows[1]["status"], "draft");
        annotate_cycle_statuses(&mut list, "2030-11-21T03:00:00");
        assert_eq!(list["results"][1]["status"], "current");
    }
}
