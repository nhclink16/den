#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
# The checked-in snapshot keeps offline and Cloud builds reproducible.
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
cp "$temporary" Packages/DenAPI/Sources/DenAPI/openapi.json
