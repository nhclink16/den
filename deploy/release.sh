#!/usr/bin/env bash
# Run from codexbox. Provision once with deploy/install-vps.sh first.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --locked --release -p den-server -p den -p den-host
build_dir="${CARGO_TARGET_DIR:-target}/release"
npm --prefix apps/web ci
npm --prefix apps/web run build
ssh -o BatchMode=yes vps 'test "$(hostname)" = vps-2fd9743a && mkdir -p /home/debian/den-release/bin /home/debian/den-release/web'
rsync -a "$build_dir/den-server" "$build_dir/den" vps:/home/debian/den-release/bin/
rsync -a --delete apps/web/dist/ vps:/home/debian/den-release/web/
ssh vps 'sudo install -m 755 /home/debian/den-release/bin/den-server /opt/den/bin/den-server.new && sudo mv /opt/den/bin/den-server.new /opt/den/bin/den-server && sudo install -m 755 /home/debian/den-release/bin/den /opt/den/bin/den && sudo rsync -a --delete /home/debian/den-release/web/ /opt/den/web/ && sudo systemctl restart den-server'
curl --fail --silent --show-error --retry 10 --retry-connrefused --retry-delay 1 https://den.nicholascaron.com/health
