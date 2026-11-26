#!/usr/bin/env bash
# shellcheck disable=SC2016  # Backticks in the printf formats are literal Markdown.
# Generate docs/src/commands.md from `plane-cli --help`.
#
# Usage:
#   ./scripts/gen-command-reference.sh [path/to/plane-cli]         # rewrite the page
#   ./scripts/gen-command-reference.sh --check [path/to/plane-cli] # fail if it is out of date
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PAGE="$ROOT/docs/src/commands.md"

CHECK=0
if [ "${1:-}" = "--check" ]; then
  CHECK=1
  shift
fi
BIN="${1:-$ROOT/target/debug/plane-cli}"
[ -x "$BIN" ] || {
  echo "gen-command-reference: no binary at $BIN (run: cargo build -p plane-cli)" >&2
  exit 1
}

# Subcommand names from the "Commands:" block of a help text, without `help`.
subcommands() {
  awk '
    /^Commands:/ { on = 1; next }
    on && /^$/ { on = 0 }
    on && /^  [a-z]/ { if ($1 != "help") print $1 }'
}

emit() {
  local help depth heading sub
  # Word splitting of "$*" is not used: each path segment is its own argument.
  help="$(env -u PLANE_CLI_JSON "$BIN" "$@" --help)"
  depth=$(($# + 1))
  [ "$depth" -le 4 ] || depth=4
  heading="$(printf '%*s' "$depth" '' | tr ' ' '#')"
  printf '\n%s `plane-cli%s`\n\n```text\n%s\n```\n' "$heading" "$(printf ' %s' "$@")" "$help"
  while IFS= read -r sub; do
    emit "$@" "$sub"
  done < <(printf '%s\n' "$help" | subcommands)
}

OUT="$(mktemp)"
trap 'rm -f "$OUT"' EXIT
{
  printf '# Command reference\n\n'
  printf 'Generated from `plane-cli --help` by `scripts/gen-command-reference.sh`. Do not edit by hand.\n'
  help="$(env -u PLANE_CLI_JSON "$BIN" --help)"
  printf '\n## `plane-cli`\n\n```text\n%s\n```\n' "$help"
  while IFS= read -r top; do
    emit "$top"
  done < <(printf '%s\n' "$help" | subcommands)
} >"$OUT"

if [ "$CHECK" -eq 1 ]; then
  if ! diff -u "$PAGE" "$OUT" >/dev/null; then
    echo "docs/src/commands.md is out of date. Run: ./scripts/gen-command-reference.sh" >&2
    exit 1
  fi
  echo "docs/src/commands.md is up to date"
else
  cp "$OUT" "$PAGE"
  echo "Wrote $PAGE"
fi
