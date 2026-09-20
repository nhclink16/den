#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

scratch=$(mktemp -d "${TMPDIR:-/tmp}/den-spotify-smoke.XXXXXX")
server_pid=
vite_pid=
cleanup() {
  [[ -z "$vite_pid" ]] || kill "$vite_pid" 2>/dev/null || true
  [[ -z "$server_pid" ]] || kill "$server_pid" 2>/dev/null || true
  wait "$vite_pid" "$server_pid" 2>/dev/null || true
  rm -rf -- "$scratch"
}
trap cleanup EXIT INT TERM

cargo build -p den-server
npm ci
npm --prefix apps/web ci

DEN_BIND=127.0.0.1:17000 \
DEN_ORIGIN=http://127.0.0.1:5173 \
DEN_DB="$scratch/den.db" \
DEN_UPLOADS="$scratch/uploads" \
DEN_BOOTSTRAP_FILE="$scratch/bootstrap.key" \
DEN_WEB_DIR="$PWD/apps/web/dist" \
  target/debug/den-server >"$scratch/server.log" 2>&1 &
server_pid=$!
DEN_API_TARGET=http://127.0.0.1:17000 npm --prefix apps/web run dev -- --host 127.0.0.1 >"$scratch/vite.log" 2>&1 &
vite_pid=$!

for _ in $(seq 1 100); do
  if curl --fail --silent http://127.0.0.1:5173/health >/dev/null; then break; fi
  sleep 0.1
done
curl --fail --silent http://127.0.0.1:5173/health >/dev/null || {
  sed -n '1,200p' "$scratch/server.log" >&2
  sed -n '1,200p' "$scratch/vite.log" >&2
  exit 1
}

DEN_SMOKE_BOOTSTRAP="$scratch/bootstrap.key" \
DEN_SMOKE_API=http://127.0.0.1:17000 \
DEN_SMOKE_COMMIT="$(git rev-parse HEAD)" \
  node scripts/spotify-jam-smoke.mjs
