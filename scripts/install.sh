#!/usr/bin/env bash
# Install plane-cli from GitHub Releases. Needs curl or wget, tar, and a SHA-256 tool.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/pseudovar/plane-cli/main/scripts/install.sh | bash
#   curl -fsSL https://raw.githubusercontent.com/pseudovar/plane-cli/main/scripts/install.sh | bash -s -- v0.3.0
#   ./scripts/install.sh [v0.3.0]
#
# Env:
#   PREFIX                 (optional) install directory; default: $HOME/.local  (binary -> $PREFIX/bin/plane-cli)
#   PLANE_CLI_VERSION      (optional) exact tag e.g. v0.3.0; overrides the first argument
#   PLANE_CLI_PRERELEASE=1 (optional) when resolving "latest", include prereleases
#   PLANE_CLI_FORCE=1      (optional) reinstall even if `plane-cli --version` already matches
#
set -euo pipefail

REPO="pseudovar/plane-cli"
API_URL="https://api.github.com/repos/${REPO}/releases?per_page=40"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download"

die() {
  echo "install.sh: $*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "missing '$1' in PATH"
}

# fetch URL DEST: download to DEST, failing on any HTTP error.
fetch() {
  case "$1" in
  https://*) ;;
  *) die "refusing non-https URL: $1" ;;
  esac
  if command -v curl >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -fsSL --retry 2 -o "$2" "$1"
  else
    # wget has no flag that limits redirects to https, so read its response headers and
    # discard the download if any redirect left https.
    local log="$2.headers"
    wget -nv -S -O "$2" "$1" 2>"$log" || {
      rm -f "$2" "$log"
      return 1
    }
    if awk 'tolower($1) == "location:" && tolower($2) ~ /^[a-z][a-z0-9+.-]*:/ && tolower($2) !~ /^https:/ { bad = 1 } END { exit !bad }' "$log"; then
      rm -f "$2" "$log"
      die "refusing a download that was redirected away from https: $1"
    fi
    rm -f "$log"
  fi
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

# Print the newest matching tag from the GitHub releases JSON (newest first). Drafts are
# always skipped; prereleases are skipped unless $2 is 1. Works on pretty or minified JSON.
pick_tag() {
  # One JSON key per line. Quotes inside strings are escaped, so they never match here.
  # The second awk reads to the end: exiting early could fail the pipe under pipefail.
  awk '{ gsub(/,[ \t]*"/, ",\n\""); gsub(/\{[ \t]*"/, "{\n\""); print }' "$1" |
    awk -v pre="$2" '
    { line = $0; sub(/^[ \t]+/, "", line) }
    line ~ /^"tag_name":/ {
      t = line
      sub(/^"tag_name":[ \t]*"/, "", t)
      sub(/".*/, "", t)
      tag = t; draft = ""; next
    }
    line ~ /^"draft":/ { draft = (line ~ /^"draft":[ \t]*true/) ? "1" : "0"; next }
    line ~ /^"prerelease":/ {
      if (found == "" && tag != "" && draft == "0" && (pre == "1" || line !~ /^"prerelease":[ \t]*true/)) found = tag
      tag = ""
    }
    END { if (found != "") print found }'
}

main() {
  command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || die "missing 'curl' or 'wget' in PATH"
  need_cmd tar
  need_cmd awk
  need_cmd sed
  command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 ||
    die "missing 'sha256sum' or 'shasum' in PATH"

  local prefix="${PREFIX:-$HOME/.local}"
  local bin_dir="${prefix}/bin"
  local version_arg="${PLANE_CLI_VERSION:-${1:-}}"

  local platform triple
  platform="$(uname -s)-$(uname -m)"
  case "$platform" in
  Linux-x86_64) triple="x86_64-unknown-linux-gnu" ;;
  Darwin-arm64) triple="aarch64-apple-darwin" ;;
  Darwin-x86_64) triple="x86_64-apple-darwin" ;;
  *) die "unsupported platform: $platform (need Linux x86_64 or macOS arm64/x86_64)" ;;
  esac

  local tmp_root="${TMPDIR:-/tmp}"
  WORKDIR="$(mktemp -d "${tmp_root%/}/plane-cli-install.XXXXXX")"
  trap 'rm -rf "$WORKDIR"' EXIT

  local tag
  if [ -n "$version_arg" ]; then
    tag="v${version_arg#v}"
  else
    fetch "$API_URL" "$WORKDIR/releases.json" ||
      die "could not list releases from GitHub (rate limited or offline?). Pin a version: install.sh v0.3.0"
    tag="$(pick_tag "$WORKDIR/releases.json" "${PLANE_CLI_PRERELEASE:-0}")"
    [ -n "$tag" ] || die "could not resolve latest release. Pin a version: install.sh v0.3.0
For prereleases set PLANE_CLI_PRERELEASE=1"
  fi
  case "$tag" in
  v[0-9]*) ;;
  *) die "invalid version tag: $tag" ;;
  esac
  local ver="${tag#v}"

  # Compare the binary at the install target, not whichever plane-cli is first on PATH.
  if [ "${PLANE_CLI_FORCE:-0}" != "1" ] && [ -x "$bin_dir/plane-cli" ]; then
    local current
    current="$("$bin_dir/plane-cli" --version 2>/dev/null | awk '{print $2}')" || current=""
    if [ -n "$current" ] && [ "$current" = "$ver" ]; then
      echo "plane-cli $ver already installed ($bin_dir/plane-cli)"
      return 0
    fi
  fi

  local stem="plane-cli-${ver}-${triple}"
  local archive="${stem}.tar.gz"
  local checksum="${archive}.sha256"

  fetch "${DOWNLOAD_URL}/${tag}/${archive}" "$WORKDIR/$archive" ||
    die "could not download $archive for $tag"
  fetch "${DOWNLOAD_URL}/${tag}/${checksum}" "$WORKDIR/$checksum" ||
    die "could not download checksum $checksum; refusing to install"

  local expected actual
  expected="$(awk 'NR==1 {print tolower($1)}' "$WORKDIR/$checksum")"
  case "$expected" in
  "" | *[!0-9a-f]*) die "checksum file $checksum is empty or malformed; refusing to install" ;;
  esac
  [ "${#expected}" -eq 64 ] || die "checksum file $checksum is malformed; refusing to install"
  actual="$(sha256_of "$WORKDIR/$archive")"
  [ "$expected" = "$actual" ] || die "checksum mismatch for $archive (expected $expected, got $actual)"
  echo "Checksum OK: $archive"

  tar xzf "$WORKDIR/$archive" -C "$WORKDIR"
  [ -f "$WORKDIR/$stem/plane-cli" ] || die "expected $stem/plane-cli inside archive"

  mkdir -p "$bin_dir"
  # Write beside the target and rename, so a running binary is replaced atomically.
  local staged="$bin_dir/.plane-cli.new.$$"
  install -m 0755 "$WORKDIR/$stem/plane-cli" "$staged" || {
    rm -f "$staged"
    die "could not write to $bin_dir"
  }
  mv -f "$staged" "$bin_dir/plane-cli" || {
    rm -f "$staged"
    die "could not replace $bin_dir/plane-cli"
  }

  echo "Installed plane-cli $tag -> $bin_dir/plane-cli"
  case ":${PATH}:" in
  *":${bin_dir}:"*) ;;
  *)
    echo ""
    echo "Add to PATH (e.g. in ~/.profile or ~/.zshrc):"
    echo "  export PATH=\"$bin_dir:\$PATH\""
    ;;
  esac
  local first
  first="$(command -v plane-cli 2>/dev/null || true)"
  if [ -n "$first" ] && [ "$first" != "$bin_dir/plane-cli" ]; then
    echo ""
    echo "Note: $first comes first on PATH and is not the binary just installed."
  fi
}

# Everything runs from here, so a truncated download through a pipe cannot run half a script.
main "$@"
