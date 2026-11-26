//! Process-wide queue of agent-facing warnings raised below the dispatch layer
//! (for example inline JSON passed to `--body`, or a hidden flag alias).
//!
//! [`crate::print_json`] drains the queue into the envelope's `warnings[]`; the CLI's
//! human output prints drained warnings to stderr.

use std::sync::Mutex;

static PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Queue a warning for the next emitted envelope.
pub fn push_warning(warning: impl Into<String>) {
    if let Ok(mut q) = PENDING.lock() {
        q.push(warning.into());
    }
}

/// Remove and return every queued warning (oldest first).
pub fn take_warnings() -> Vec<String> {
    PENDING
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default()
}
