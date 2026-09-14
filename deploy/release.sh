#!/usr/bin/env bash
# Usage: deploy/release.sh [vX.Y.Z]. A tag redeploy needs curl, tar, SSH and rsync.
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ $# -gt 0 ]]; then
  tag=$1
  [[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$ ]] || { echo 'Expected a release tag, such as v0.1.0' >&2; exit 1; }
  stage=$(mktemp -d)
  trap 'rm -rf "$stage"' EXIT
  base="https://github.com/nhclink16/den/releases/download/$tag"
  asset=den-server-linux-x86_64.tar.gz
  curl -fsSL "$base/$asset" -o "$stage/$asset"
  curl -fsSL "$base/SHA256SUMS" -o "$stage/SHA256SUMS"
  expected=$(awk -v name="$asset" '$2 == name { print $1 }' "$stage/SHA256SUMS")
  if command -v sha256sum >/dev/null; then actual=$(sha256sum "$stage/$asset" | cut -d ' ' -f 1)
  else actual=$(shasum -a 256 "$stage/$asset" | cut -d ' ' -f 1); fi
  [[ -n $expected && $actual = "$expected" ]] || { echo 'Release checksum mismatch' >&2; exit 1; }
  tar -xzf "$stage/$asset" -C "$stage"
  build_dir=$stage
  web_dir=$stage/web
  deploy_dir=$stage/deploy
else
  cargo build --locked --release -p den-server -p den -p den-host
  build_dir="${CARGO_TARGET_DIR:-target}/release"
  npm --prefix apps/web ci
  npm --prefix apps/web run build
  web_dir=apps/web/dist
  deploy_dir=deploy
fi
ssh -o BatchMode=yes vps 'test "$(hostname)" = vps-2fd9743a && mkdir -p /home/debian/den-release/bin /home/debian/den-release/web /home/debian/den-release/deploy'
rsync -a "$build_dir/den-server" "$build_dir/den" vps:/home/debian/den-release/bin/
rsync -a --delete "$web_dir/" vps:/home/debian/den-release/web/
rsync -a --delete "$deploy_dir/" vps:/home/debian/den-release/deploy/
ssh vps 'sudo install -m 755 /home/debian/den-release/bin/den-server /opt/den/bin/den-server.new && sudo mv /opt/den/bin/den-server.new /opt/den/bin/den-server && sudo install -m 755 /home/debian/den-release/bin/den /opt/den/bin/den && sudo rsync -a --delete /home/debian/den-release/web/ /opt/den/web/ && sudo rsync -a --delete /home/debian/den-release/deploy/ /opt/den/deploy/ && sudo install -m 644 /opt/den/deploy/systemd/den-server.service /etc/systemd/system/den-server.service && sudo systemctl daemon-reload && sudo systemctl restart den-server'
curl --fail --silent --show-error --retry 10 --retry-connrefused --retry-delay 1 https://denchat.app/health
