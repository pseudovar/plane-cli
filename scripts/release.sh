#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Bump crate versions, commit, tag, and push to trigger GitHub Releases (.github/workflows/release.yml).

Usage:
  ./scripts/release.sh <new-version>
  ./scripts/release.sh patch|minor|major

Examples:
  ./scripts/release.sh 0.1.2
  ./scripts/release.sh patch

Options:
  --dry-run    Print what would be done; do not modify files, git, or remotes
  --no-push    Commit and tag locally only (does not run git push)
  --no-verify  Skip `cargo check -p plane-cli` after bumping

Requires a clean git working tree (commit or stash first).
EOF
}

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

die() {
  echo "release.sh: $*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing '$1' in PATH"
}

PLANE_CLI_TOML="$ROOT/plane-cli/Cargo.toml"
PLANE_CORE_TOML="$ROOT/plane-core/Cargo.toml"

read_package_version() {
  local file="$1"
  grep -m1 '^version' "$file" | sed -n 's/^version = "\([^"]*\)".*/\1/p'
}

set_package_version_line() {
  local file="$1"
  local ver="$2"
  # Only the first top-level `version = "..."` line ([package] section comes first in these crates).
  perl -i -pe 'BEGIN { $n = 0 } if (/^version = "/ && $n++ == 0) { s/^version = "[^"]*"/version = "'"$ver"'"/ }' "$file"
}

validate_semver() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]] || die "invalid version '$1' (expected semver like 0.2.0 or 1.0.0-rc.1)"
}

bump_semver() {
  local cur="$1"
  local kind="$2"
  local base="${cur%%-*}"
  IFS=. read -r X Y Z <<< "$base" || true
  [[ -n "${X:-}" && -n "${Y:-}" && -n "${Z:-}" ]] || die "cannot parse current version '$cur'"
  case "$kind" in
    patch) Z=$((Z + 1)) ;;
    minor) Y=$((Y + 1)); Z=0 ;;
    major) X=$((X + 1)); Y=0; Z=0 ;;
    *) die "internal: bad kind '$kind'" ;;
  esac
  if [[ "$cur" == *-* ]]; then
    die "current version has a pre-release suffix ($cur); bump with an explicit new version instead of $kind"
  fi
  echo "${X}.${Y}.${Z}"
}

DRY_RUN=0
NO_PUSH=0
NO_VERIFY=0
VERSION_ARG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --no-push)
      NO_PUSH=1
      shift
      ;;
    --no-verify)
      NO_VERIFY=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    -*)
      die "unknown option: $1"
      ;;
    *)
      if [[ -n "$VERSION_ARG" ]]; then
        die "unexpected extra argument: $1"
      fi
      VERSION_ARG="$1"
      shift
      ;;
  esac
done

[[ -n "$VERSION_ARG" ]] || die "expected exactly one argument (version or patch|minor|major). Try --help."

ARG="$VERSION_ARG"
CURRENT_CLI="$(read_package_version "$PLANE_CLI_TOML")"
CURRENT_CORE="$(read_package_version "$PLANE_CORE_TOML")"

[[ -n "$CURRENT_CLI" && -n "$CURRENT_CORE" ]] || die "could not read version from Cargo.toml files"
[[ "$CURRENT_CLI" == "$CURRENT_CORE" ]] || die "version mismatch: plane-cli=$CURRENT_CLI plane-core=$CURRENT_CORE (fix manually)"

NEW_VER=""
case "$ARG" in
  patch | minor | major)
    NEW_VER="$(bump_semver "$CURRENT_CLI" "$ARG")"
    ;;
  *)
    NEW_VER="$ARG"
    validate_semver "$NEW_VER"
    ;;
esac

[[ "$NEW_VER" != "$CURRENT_CLI" ]] || die "new version ($NEW_VER) equals current ($CURRENT_CLI); nothing to do"

if [[ "$DRY_RUN" -eq 1 ]]; then
  echo "Would bump: $CURRENT_CLI -> $NEW_VER (both crates)"
  echo "Would run: cargo check -p plane-cli"
  echo "Would commit: Bump version to $NEW_VER"
  echo "Would tag: v$NEW_VER"
  if [[ "$NO_PUSH" -eq 1 ]]; then
    echo "Would not push (--no-push)"
  else
    echo "Would push: origin HEAD and origin v$NEW_VER"
  fi
  exit 0
fi

if [[ -n "$(git status --porcelain)" ]]; then
  die "working tree is not clean. Commit or stash changes before releasing."
fi

need_cmd git
need_cmd perl

set_package_version_line "$PLANE_CLI_TOML" "$NEW_VER"
set_package_version_line "$PLANE_CORE_TOML" "$NEW_VER"

if [[ "$NO_VERIFY" -eq 0 ]]; then
  need_cmd cargo
  cargo check -p plane-cli
fi

git add plane-cli/Cargo.toml plane-core/Cargo.toml
if [[ -f Cargo.lock ]] && git status --porcelain Cargo.lock | grep -q .; then
  git add Cargo.lock
fi

git commit -m "Bump version to $NEW_VER"

git tag -a "v$NEW_VER" -m "Release v$NEW_VER"

if [[ "$NO_PUSH" -eq 1 ]]; then
  echo "Tagged v$NEW_VER locally. Push when ready:"
  echo "  git push origin HEAD"
  echo "  git push origin v$NEW_VER"
  exit 0
fi

need_cmd gh
if ! gh auth status >/dev/null 2>&1; then
  die "GitHub CLI is not logged in. Run: gh auth login"
fi

CURRENT_BRANCH="$(git branch --show-current)"
git push origin "$CURRENT_BRANCH"
git push origin "v$NEW_VER"

echo "Pushed v$NEW_VER — GitHub Actions should build and publish the release."
