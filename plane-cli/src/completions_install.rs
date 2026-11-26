//! Install shell completions into conventional paths (`plane-cli completions --install`).

use crate::completion_hooks;
use crate::dispatch::Cli;
use clap::CommandFactory;
use clap_complete::Shell;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const MARK_START: &str = "# >>> plane-cli completions";
const MARK_END: &str = "# <<< plane-cli completions <<<";

pub fn run(shell: Shell, catalog_hooks: bool, dry_run: bool) -> Result<(), String> {
    let home = home_dir()?;
    let data_dir = xdg_data_home(&home)?.join("plane-cli/completions");

    match shell {
        Shell::Bash => install_bash(&home, &data_dir, catalog_hooks, dry_run),
        Shell::Zsh => install_zsh(&home, &data_dir, catalog_hooks, dry_run),
        Shell::Fish => install_fish(&home, catalog_hooks, dry_run),
        Shell::Elvish | Shell::PowerShell => {
            install_script_only(shell, &data_dir, catalog_hooks, dry_run)
        }
        _ => install_script_only(shell, &data_dir, catalog_hooks, dry_run),
    }
}

fn home_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| "HOME not set; cannot install completions".to_string())
}

fn xdg_data_home(home: &Path) -> Result<PathBuf, String> {
    if let Ok(d) = std::env::var("XDG_DATA_HOME") {
        let t = d.trim();
        if !t.is_empty() {
            return Ok(PathBuf::from(t));
        }
    }
    Ok(home.join(".local/share"))
}

fn xdg_config_home(home: &Path) -> Result<PathBuf, String> {
    if let Ok(d) = std::env::var("XDG_CONFIG_HOME") {
        let t = d.trim();
        if !t.is_empty() {
            return Ok(PathBuf::from(t));
        }
    }
    Ok(home.join(".config"))
}

fn write_full_script(shell: Shell, catalog_hooks: bool, w: &mut dyn Write) -> std::io::Result<()> {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "plane-cli", w);
    if catalog_hooks {
        completion_hooks::write_catalog_hooks(shell, w)?;
    }
    Ok(())
}

fn script_bytes(shell: Shell, catalog_hooks: bool) -> Result<Vec<u8>, String> {
    let mut v = Vec::new();
    write_full_script(shell, catalog_hooks, &mut v).map_err(|e| e.to_string())?;
    Ok(v)
}

fn bash_rc_path(home: &Path) -> PathBuf {
    let bashrc = home.join(".bashrc");
    if bashrc.exists() {
        return bashrc;
    }
    let profile = home.join(".bash_profile");
    if profile.exists() {
        return profile;
    }
    bashrc
}

fn rc_contains_marker(rc: &Path) -> Result<bool, String> {
    if !rc.exists() {
        return Ok(false);
    }
    let s = fs::read_to_string(rc).map_err(|e| format!("read {}: {e}", rc.display()))?;
    Ok(s.contains(MARK_START))
}

fn append_bash_block(rc: &Path, source_line: &str, dry_run: bool) -> Result<(), String> {
    let block = format!("\n{MARK_START}\n{source_line}\n{MARK_END}\n");
    if dry_run {
        eprintln!("Would append to {}:\n{}", rc.display(), block.trim_end());
        return Ok(());
    }
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(rc)
        .map_err(|e| format!("open {}: {e}", rc.display()))?;
    f.write_all(block.as_bytes())
        .map_err(|e| format!("write {}: {e}", rc.display()))?;
    Ok(())
}

fn prepend_zsh_block(rc: &Path, fpath_line: &str, dry_run: bool) -> Result<(), String> {
    let block = format!("{MARK_START}\n{fpath_line}\n{MARK_END}\n\n");
    if dry_run {
        eprintln!("Would prepend to {}:\n{}", rc.display(), block.trim_end());
        return Ok(());
    }
    let existing = if rc.exists() {
        fs::read_to_string(rc).map_err(|e| format!("read {}: {e}", rc.display()))?
    } else {
        String::new()
    };
    let new_body = format!("{block}{existing}");
    fs::write(rc, new_body.as_bytes()).map_err(|e| format!("write {}: {e}", rc.display()))?;
    Ok(())
}

fn zsh_single_quote_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

fn install_bash(
    home: &Path,
    data_dir: &Path,
    catalog_hooks: bool,
    dry_run: bool,
) -> Result<(), String> {
    let script_path = data_dir.join("plane-cli.bash");
    let bytes = script_bytes(Shell::Bash, catalog_hooks)?;

    if dry_run {
        eprintln!(
            "Would create directory {} and write {}",
            data_dir.display(),
            script_path.display()
        );
    } else {
        fs::create_dir_all(data_dir).map_err(|e| format!("create {}: {e}", data_dir.display()))?;
        fs::write(&script_path, &bytes)
            .map_err(|e| format!("write {}: {e}", script_path.display()))?;
        eprintln!("Wrote {}", script_path.display());
    }

    let rc = bash_rc_path(home);
    let source_line = format!("source {}", zsh_single_quote_path(&script_path));

    if rc_contains_marker(&rc)? {
        eprintln!(
            "Shell rc already contains plane-cli block — skipping edit ({}).",
            rc.display()
        );
        return summarize(shell_hint(Shell::Bash), dry_run);
    }

    append_bash_block(&rc, &source_line, dry_run)?;
    if !dry_run {
        eprintln!(
            "Appended a short block to {}. Restart your shell or `source` it.",
            rc.display()
        );
    }
    summarize(shell_hint(Shell::Bash), dry_run)
}

fn install_zsh(
    home: &Path,
    data_dir: &Path,
    catalog_hooks: bool,
    dry_run: bool,
) -> Result<(), String> {
    let script_path = data_dir.join("_plane-cli");
    let bytes = script_bytes(Shell::Zsh, catalog_hooks)?;

    if dry_run {
        eprintln!(
            "Would create directory {} and write {}",
            data_dir.display(),
            script_path.display()
        );
    } else {
        fs::create_dir_all(data_dir).map_err(|e| format!("create {}: {e}", data_dir.display()))?;
        fs::write(&script_path, &bytes)
            .map_err(|e| format!("write {}: {e}", script_path.display()))?;
        eprintln!("Wrote {}", script_path.display());
    }

    let zshrc = home.join(".zshrc");
    let fpath_line = format!("fpath=({} $fpath)", zsh_single_quote_path(data_dir));

    if rc_contains_marker(&zshrc)? {
        eprintln!(
            "Shell rc already contains plane-cli block — skipping edit ({}).",
            zshrc.display()
        );
        return summarize(shell_hint(Shell::Zsh), dry_run);
    }

    prepend_zsh_block(&zshrc, &fpath_line, dry_run)?;
    if !dry_run {
        eprintln!(
            "Prepended fpath to {} so `_plane-cli` is picked up before `compinit`. Open a new terminal or `exec zsh`.",
            zshrc.display()
        );
    }
    summarize(shell_hint(Shell::Zsh), dry_run)
}

fn install_fish(home: &Path, catalog_hooks: bool, dry_run: bool) -> Result<(), String> {
    let completions_dir = xdg_config_home(home)?.join("fish/completions");
    let script_path = completions_dir.join("plane-cli.fish");
    let bytes = script_bytes(Shell::Fish, catalog_hooks)?;

    if dry_run {
        eprintln!(
            "Would create directory {} and write {}",
            completions_dir.display(),
            script_path.display()
        );
    } else {
        fs::create_dir_all(&completions_dir)
            .map_err(|e| format!("create {}: {e}", completions_dir.display()))?;
        fs::write(&script_path, &bytes)
            .map_err(|e| format!("write {}: {e}", script_path.display()))?;
        eprintln!("Wrote {}", script_path.display());
        eprintln!(
            "Fish loads completions from this directory automatically; open a new terminal or run `exec fish`."
        );
    }
    summarize(shell_hint(Shell::Fish), dry_run)
}

fn install_script_only(
    shell: Shell,
    data_dir: &Path,
    catalog_hooks: bool,
    dry_run: bool,
) -> Result<(), String> {
    let ext = match shell {
        Shell::Elvish => "elv",
        Shell::PowerShell => "ps1",
        _ => "txt",
    };
    let script_path = data_dir.join(format!("plane-cli.{ext}"));
    let bytes = script_bytes(shell, catalog_hooks)?;

    if dry_run {
        eprintln!(
            "Would create directory {} and write {}",
            data_dir.display(),
            script_path.display()
        );
    } else {
        fs::create_dir_all(data_dir).map_err(|e| format!("create {}: {e}", data_dir.display()))?;
        fs::write(&script_path, &bytes)
            .map_err(|e| format!("write {}: {e}", script_path.display()))?;
        eprintln!("Wrote {}", script_path.display());
    }

    eprintln!(
        "Automatic rc/profile wiring is not provided for {:?}; source {} from your shell profile when ready.",
        shell,
        script_path.display()
    );
    summarize(shell_hint(shell), dry_run)
}

fn shell_hint(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => "bash",
        Shell::Zsh => "zsh",
        Shell::Fish => "fish",
        Shell::Elvish => "elvish",
        Shell::PowerShell => "powershell",
        _ => "your shell",
    }
}

fn summarize(shell_name: &str, dry_run: bool) -> Result<(), String> {
    if dry_run {
        eprintln!("Dry run only — no files were modified.");
    } else {
        eprintln!("Done. Tab-complete `plane-cli` after restarting {shell_name} (or re-source your rc file).");
    }
    Ok(())
}
