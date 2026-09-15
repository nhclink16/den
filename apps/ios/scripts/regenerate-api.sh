#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
# Keep the exact server contract separate from the tolerant Swift generation view.
# An explicit argument can target a local server built from the shared Rust types.
origin="${1:-https://denchat.app}"
temporary=$(mktemp)
trap 'rm -f "$temporary"' EXIT
curl --fail --silent --show-error --max-time 30 "$origin/openapi.json" > "$temporary"
python3 - "$temporary" <<'PY'
import json, sys
spec = json.load(open(sys.argv[1]))
assert spec['openapi'].startswith('3.')
assert 'Session' in spec['components']['schemas']
PY
mkdir -p Packages/DenAPI/Contract
cp "$temporary" Packages/DenAPI/Contract/openapi.json
python3 scripts/prepare-client-schema.py
