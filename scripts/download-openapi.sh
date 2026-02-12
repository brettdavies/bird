#!/usr/bin/env bash
# Download latest X API OpenAPI spec into openapi/x-api-openapi.json.
# Run from repo root. Then rebuild: cargo build --release

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT="$REPO_ROOT/openapi/x-api-openapi.json"
URL="${X_API_OPENAPI_URL:-https://api.x.com/2/openapi.json}"

mkdir -p "$(dirname "$OUT")"
curl -sSfL -o "$OUT" "$URL"
echo "Wrote $OUT"
