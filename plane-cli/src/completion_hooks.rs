//! Optional shell snippets appended by `plane-cli completions … --catalog-hooks`.

use clap_complete::Shell;
use std::io::{self, Write};

const BASH_HOOKS: &str = r#"

# === plane-cli catalog completion (from `plane-cli completions bash --catalog-hooks`) ===
# Requires PLANE_API_BASE_URL, PLANE_API_KEY, and PLANE_WORKSPACE_SLUG in login shells.
# Source this *after* the main `plane-cli completions bash` script so `_plane-cli` exists.
eval "$(declare -f _plane-cli | sed '1s/_plane-cli/_plane_cli__clap_core/')"
_plane_cli__find_flag_value() {
  local flag="$1" i
  for ((i=1; i<COMP_CWORD; i++)); do
    if [[ "${COMP_WORDS[i]}" == "$flag" && -n "${COMP_WORDS[i+1]:-}" ]]; then
      printf '%s' "${COMP_WORDS[i+1]}"
      return 0
    fi
  done
  return 1
}
_plane-cli() {
  local cur="${COMP_WORDS[COMP_CWORD]}"
  local prev="${COMP_WORDS[COMP_CWORD-1]}"
  case "$prev" in
    --project)
      mapfile -t COMPREPLY < <(compgen -W "$(plane-cli complete projects 2>/dev/null | tr '\n' ' ')" -- "$cur")
      return 0
      ;;
    --state)
      local p
      p=$(_plane_cli__find_flag_value --project) || { _plane_cli__clap_core "$@"; return; }
      mapfile -t COMPREPLY < <(compgen -W "$(plane-cli complete states --project "$p" 2>/dev/null | tr '\n' ' ')" -- "$cur")
      return 0
      ;;
    --module)
      local p
      p=$(_plane_cli__find_flag_value --project) || { _plane_cli__clap_core "$@"; return; }
      mapfile -t COMPREPLY < <(compgen -W "$(plane-cli complete modules --project "$p" 2>/dev/null | tr '\n' ' ')" -- "$cur")
      return 0
      ;;
  esac
  _plane_cli__clap_core "$@"
}
complete -r plane-cli 2>/dev/null || true
complete -F _plane-cli plane-cli
"#;

const ZSH_HOOKS: &str = r#"

# === plane-cli catalog completion (from `plane-cli completions zsh --catalog-hooks`) ===
# zsh compiles from `plane-cli complete …`. Helpers (source after the generated `_plane-cli`):
_plane_cli_complete_projects() {
  local -a items; items=(${(f)"$(plane-cli complete projects 2>/dev/null)"})
  _describe -t identifiers 'project identifiers' items
}
_plane_cli_complete_states() {
  local proj="${(Q)opt_args[--project]:-}"
  [[ -z "$proj" ]] && return 1
  local -a items; items=(${(f)"$(plane-cli complete states --project "$proj" 2>/dev/null)"})
  _describe -t states 'workflow states' items
}
_plane_cli_complete_modules() {
  local proj="${(Q)opt_args[--project]:-}"
  [[ -z "$proj" ]] && return 1
  local -a items; items=(${(f)"$(plane-cli complete modules --project "$proj" 2>/dev/null)"})
  _describe -t modules 'module UUIDs' items
}
# Wire these inside a custom `compdef` wrapper, or extend the generated `_plane-cli` with
# `_arguments` specs that call `_plane_cli_complete_projects` for `--project` values, etc.
"#;

const OTHER_NOTE: &str = r#"

# plane-cli: --catalog-hooks only installs automatic wiring for bash.
# On this shell, call `plane-cli complete …` from your own completion rules;
# see https://pseudovar.github.io/plane-cli/agent-contract.html
"#;

pub fn write_catalog_hooks(shell: Shell, w: &mut dyn Write) -> io::Result<()> {
    let body = catalog_hooks_body(shell);
    w.write_all(body.as_bytes())?;
    Ok(())
}

fn catalog_hooks_body(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => BASH_HOOKS,
        Shell::Zsh => ZSH_HOOKS,
        _ => OTHER_NOTE,
    }
}

pub fn print_catalog_hooks(shell: Shell) {
    print!("{}", catalog_hooks_body(shell));
}
