use crate::config_cmd;
use crate::skill_cmd;
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use plane_core::CoreError;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{stdout, Stdout};

pub fn run_skill_install_wizard(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), CoreError> {
    terminal
        .clear()
        .map_err(|e| CoreError::Usage(format!("terminal clear: {e}")))?;
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
    eprintln!("Launching skill installer wizard...");
    eprintln!(
        "OpenClaw (no npx): {} self skills install --openclaw",
        env!("CARGO_PKG_NAME")
    );
    eprintln!(
        "Command: npx {}",
        skill_cmd::skills_install_command().join(" ")
    );
    let install_result = skill_cmd::run_install_interactive();
    execute!(stdout(), EnterAlternateScreen)
        .map_err(|e| CoreError::Usage(format!("alternate screen: {e}")))?;
    enable_raw_mode().map_err(|e| CoreError::Usage(format!("raw mode: {e}")))?;
    terminal
        .clear()
        .map_err(|e| CoreError::Usage(format!("terminal clear: {e}")))?;
    install_result
}

pub fn run_config_init_wizard(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<std::path::PathBuf, CoreError> {
    terminal
        .clear()
        .map_err(|e| CoreError::Usage(format!("terminal clear: {e}")))?;
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen);
    let init_result = config_cmd::run_init_interactive(None, false, false);
    execute!(stdout(), EnterAlternateScreen)
        .map_err(|e| CoreError::Usage(format!("alternate screen: {e}")))?;
    enable_raw_mode().map_err(|e| CoreError::Usage(format!("raw mode: {e}")))?;
    terminal
        .clear()
        .map_err(|e| CoreError::Usage(format!("terminal clear: {e}")))?;
    init_result
}
