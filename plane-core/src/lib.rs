//! Shared Plane REST client, cache, and helpers for `plane-cli`.

// `CoreError::Http` carries API bodies; boxing would complicate callers for little gain.
#![allow(clippy::result_large_err)]

mod api;
mod cache;
mod catalog;
mod client;
mod config;
mod cycles;
mod envelope;
mod error;
mod merge;
mod named;
mod pagination;
mod project_context;
mod resolve;
mod settings;
mod validation;
mod warnings;

pub use api::PlaneApi;
pub use cache::Cache;
pub use catalog::{
    labels_from_value, projects_from_value, states_from_value, LabelEntry, ProjectEntry, StateEntry,
};
pub use client::HttpClient;
pub use config::{effective_json_output, Config};
pub use cycles::{
    annotate_cycle_statuses, cycle_date_flag, cycle_names, cycle_status, cycle_summaries,
    find_cycle_by_name, now_utc_timestamp, CYCLE_VIEWS,
};
pub use envelope::{print_json, ResponseEnvelope};
pub use error::{CoreError, ExitKind};
pub use merge::{load_json_source, merge_objects};
pub use named::{match_named, resolve_named, unknown_name_error, NameScope};
pub use pagination::{fetch_page, normalize_list, PageParams, MAX_ALL_PAGES, MAX_ALL_ROWS};
pub use project_context::{
    agent_hints, archived_project_hint, build_project_context, label_record, module_summaries,
    project_capabilities, project_summary_section, state_record, states_by_group, states_sorted,
};
pub use resolve::{parse_composite_work_item, resolve_project_id, resolve_work_item_uuid};
pub use settings::{
    annotate_intake_statuses, annotate_role_names, default_state_color, for_each_row, guess_mime,
    intake_status_name, normalize_color, parse_intake_status, parse_on_off, parse_role,
    project_features, resolve_label_ref, resolve_member_ref, resolve_state_ref, role_name,
    INTAKE_STATUSES, PROJECT_FEATURES, STATE_GROUPS,
};
pub use validation::normalize_work_item_body;
pub use warnings::{push_warning, take_warnings};

#[cfg(test)]
mod lib_tests;
