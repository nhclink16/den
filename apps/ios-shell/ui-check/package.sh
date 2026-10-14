#!/bin/bash
# Packs the made-up Den that Xcode Cloud's "UI check" runs Den Shell against (see ci_scripts/ci_pre_xcodebuild.sh):
# den-server, the web app, LiveKit and a database seeded with made-up people. Runs on a macOS arm64 GitHub runner
# from the repo root, after `cargo build -p den-server` and `npm --prefix apps/web run build`.
#   apps/ios-shell/ui-check/package.sh <out.tar.gz>
set -euo pipefail
out=$1
stage=$(mktemp -d)
trap 'kill "${pid:-}" 2>/dev/null || true; rm -rf "$stage"' EXIT
# LiveKit publishes no macOS server builds; Homebrew has one.
HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_CLEANUP=1 brew install --quiet livekit
cp "$(brew --prefix)/bin/livekit-server" "${CARGO_TARGET_DIR:-target}/debug/den-server" "$stage/"
"$stage/livekit-server" --version
cp -R apps/web/dist "$stage/web"
# Seed through the API on the same port and origin the test machine uses.
mkdir "$stage/data"
DEN_BIND=127.0.0.1:7000 DEN_ORIGIN=http://localhost:7000 DEN_WEB_DIR="$stage/web" DEN_DB="$stage/data/den.db" \
    DEN_UPLOADS="$stage/data/uploads" DEN_BOOTSTRAP_FILE="$stage/data/bootstrap.key" "$stage/den-server" &
pid=$!
for _ in $(seq 30); do curl -fs http://localhost:7000/health >/dev/null && break; sleep 1; done
python3 apps/ios-shell/ui-check/seed.py http://localhost:7000 "$stage/data/bootstrap.key"
kill "$pid"; wait "$pid" || true
tar -czf "$out" -C "$stage" .
ls -l "$out"
