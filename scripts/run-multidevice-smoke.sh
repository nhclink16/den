#!/usr/bin/env bash
# Isolated localhost server, database and LiveKit. Never joins public hangout.
set -euo pipefail
cd "$(dirname "$0")/.."
scratch=$(mktemp -d /tmp/den-multidevice.XXXXXX)
container="den-multidevice-$$"
server_pid=''
cleanup() {
  [[ -z "$server_pid" ]] || kill "$server_pid" 2>/dev/null || true
  docker stop "$container" >/dev/null 2>&1 || true
  if [[ ${DEN_SMOKE_KEEP:-0} != 1 ]]; then rm -rf -- "$scratch"; fi
}
trap cleanup EXIT
umask 077
python3 - "$scratch" <<'PY'
import pathlib,secrets,sys,json
p=pathlib.Path(sys.argv[1]);key='API'+secrets.token_hex(12);secret=secrets.token_urlsafe(48)
(p/'server.env').write_text(f'DEN_LIVEKIT_API_KEY={key}\nDEN_LIVEKIT_API_SECRET={secret}\n')
(p/'credentials.json').write_text(json.dumps({'password':secrets.token_urlsafe(24)}))
(p/'livekit.yaml').write_text(f'''port: 17880
bind_addresses: [127.0.0.1]
rtc:
  node_ip: 127.0.0.1
  tcp_port: 17881
  port_range_start: 50600
  port_range_end: 50800
  use_external_ip: false
  enable_loopback_candidate: true
  ips:
    includes: ["127.0.0.1/32"]
turn:
  enabled: false
keys:
  {key}: {secret}
webhook:
  api_key: {key}
  urls: ["http://127.0.0.1:17400/livekit/webhook"]
logging:
  level: warn
''')
PY
set -a
source "$scratch/server.env"
set +a
docker run --rm -d --name "$container" --network host -v "$scratch/livekit.yaml:/etc/livekit.yaml:ro" livekit/livekit-server:v1.9.0 --config /etc/livekit.yaml >/dev/null
DEN_BIND=127.0.0.1:17400 DEN_ORIGIN=http://127.0.0.1:17400 DEN_DB="$scratch/den.db" DEN_UPLOADS="$scratch/uploads" DEN_BOOTSTRAP_FILE="$scratch/bootstrap.key" DEN_WEB_DIR="$PWD/apps/web/dist" DEN_LIVEKIT_URL=ws://127.0.0.1:17880 "${CARGO_TARGET_DIR:-target}/release/den-server" > "$scratch/server.log" 2>&1 &
server_pid=$!
for _ in {1..50}; do
  kill -0 "$server_pid"
  if curl -fsS http://127.0.0.1:17400/health >/dev/null 2>&1 && curl -fsS http://127.0.0.1:17880 >/dev/null 2>&1; then break; fi
  sleep 0.2
done
export DEN_SMOKE_CREDENTIALS="$scratch/credentials.json" DEN_SMOKE_URL=http://127.0.0.1:17400 DEN_SMOKE_MEDIA_IP=127.0.0.1 DEN_SMOKE_SHOTS="file://$scratch/shots/"
node scripts/multidevice-smoke.mjs "$scratch" | tee "$scratch/multidevice.log"
# Account setup above shares the server login budget with the next smoke.
sleep 60
node scripts/m3-smoke.mjs | tee "$scratch/m3.log"
[[ ${DEN_SMOKE_KEEP:-0} != 1 ]] || printf 'Smoke evidence: %s\n' "$scratch"
