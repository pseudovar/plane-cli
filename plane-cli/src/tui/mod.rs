//! Terminal UI: modular layout, serialized Plane API worker, master–detail lists.
//!
//! Architecture and keybindings are documented in the book (`docs/src/tui.md`).

mod app;
mod charts;
mod format;
mod help_text;
mod modal;
mod msg;
mod project_open;
mod run;
mod theme;
mod view;
mod wizards;
mod worker;

pub use run::run_tui;
