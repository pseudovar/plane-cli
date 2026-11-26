#!/usr/bin/env bash
# Generate Rust sources from a Plane OpenAPI schema you supply (optional workflow).
# The schema is not vendored in this repo. It is copied or downloaded into gen/, which is
# gitignored, along with the generated code. Requires Docker.
#
# Usage:
#   ./scripts/gen-openapi-types.sh <path-or-url>
#   PLANE_OPENAPI_SPEC=<path-or-url> ./scripts/gen-openapi-types.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE="${1:-${PLANE_OPENAPI_SPEC:-}}"
OUT="${ROOT}/gen/plane-openapi"

if [[ -z "$SOURCE" ]]; then
  echo "usage: $0 <path-or-url to a Plane OpenAPI schema> (or set PLANE_OPENAPI_SPEC)" >&2
  exit 2
fi

case "$SOURCE" in
*.json) SPEC_NAME="plane-openapi-spec.json" ;;
*) SPEC_NAME="plane-openapi-spec.yaml" ;;
esac
SPEC="${ROOT}/gen/${SPEC_NAME}"
mkdir -p "$OUT"

case "$SOURCE" in
http://* | https://*)
  curl -fsSL "$SOURCE" -o "$SPEC"
  ;;
*)
  if [[ ! -f "$SOURCE" ]]; then
    echo "missing spec: $SOURCE" >&2
    exit 1
  fi
  cp "$SOURCE" "$SPEC"
  ;;
esac

# Docs: https://openapi-generator.tech/docs/generators/rust/
docker run --rm \
  -u "$(id -u):$(id -g)" \
  -v "$ROOT:/local" \
  openapitools/openapi-generator-cli:v7.10.0 generate \
  -i "/local/gen/${SPEC_NAME}" \
  -g rust \
  -o "/local/gen/plane-openapi" \
  --additional-properties=packageName=plane_openapi,supportAsync=false

echo "Generated under $OUT (review and wire into workspace as a separate crate if desired)."
