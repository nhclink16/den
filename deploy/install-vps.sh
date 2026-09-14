#!/usr/bin/env bash
# Copy deploy/ to the VPS, then sudo bash deploy/install-vps.sh.
set -euo pipefail
umask 077
cd "$(dirname "$0")"
[[ $(hostname) = vps-2fd9743a ]]
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y dnsutils
for name in denchat.app rtc.denchat.app; do
  [[ $(dig @1.1.1.1 "$name" A +short) = 135.148.120.197 ]] || { echo "DNS mismatch: $name" >&2; exit 1; }
done
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -y curl gnupg debian-keyring debian-archive-keyring apt-transport-https rsync sqlite3 nftables python3 ca-certificates
if [[ ! -f /etc/apt/sources.list.d/caddy-stable.list ]]; then
  curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/gpg.key | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt > /etc/apt/sources.list.d/caddy-stable.list
  chmod 644 /usr/share/keyrings/caddy-stable-archive-keyring.gpg /etc/apt/sources.list.d/caddy-stable.list
fi
apt-get update -qq
apt-get install -y caddy haproxy
id den >/dev/null 2>&1 || useradd --system --home-dir /var/lib/den --shell /usr/sbin/nologin den
id livekit >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin livekit
install -d -m 755 /opt/den/bin /opt/den/web /etc/den
install -d -o root -g livekit -m 750 /etc/livekit
install -d -o den -g den -m 700 /var/lib/den /var/lib/den/uploads
if [[ ! -e /etc/den/den.env ]]; then
  python3 - <<'PY'
import os, pathlib, secrets, pwd
key, secret = 'API' + secrets.token_hex(12), secrets.token_urlsafe(48)
p = pathlib.Path('/etc/den/den.env')
p.write_text(f'''DEN_BIND=127.0.0.1:7000
DEN_ORIGIN=https://denchat.app
DEN_DB=/var/lib/den/den.db
DEN_UPLOADS=/var/lib/den/uploads
DEN_BOOTSTRAP_FILE=/var/lib/den/bootstrap.key
DEN_WEB_DIR=/opt/den/web
DEN_LIVEKIT_URL=wss://rtc.denchat.app
DEN_LIVEKIT_API_KEY={key}
DEN_LIVEKIT_API_SECRET={secret}
RUST_LOG=den_server=info
''')
p.chmod(0o600)
u = pwd.getpwnam('den'); os.chown(p, u.pw_uid, u.pw_gid)
p = pathlib.Path('/etc/livekit/livekit.yaml')
p.write_text(pathlib.Path('livekit-public.yaml').read_text().replace('${LIVEKIT_API_KEY}', key).replace('${LIVEKIT_API_SECRET}', secret))
p.chmod(0o600)
u = pwd.getpwnam('livekit'); os.chown(p, u.pw_uid, u.pw_gid)
PY
fi
if [[ ! -x /opt/den/bin/livekit-server ]]; then
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  curl -fL https://github.com/livekit/livekit/releases/download/v1.9.0/livekit_1.9.0_linux_amd64.tar.gz -o "$tmp/livekit_1.9.0_linux_amd64.tar.gz"
  curl -fL https://github.com/livekit/livekit/releases/download/v1.9.0/checksums.txt -o "$tmp/checksums.txt"
  (cd "$tmp" && sha256sum --ignore-missing -c checksums.txt)
  tar -xzf "$tmp/livekit_1.9.0_linux_amd64.tar.gz" -C "$tmp"
  install -m 755 "$tmp/livekit-server" /opt/den/bin/livekit-server
fi
install -m 755 sync-turn-cert.sh rotate-livekit-keys.py /opt/den/bin/
install -m 644 systemd/den-server.service systemd/livekit.service systemd/den-turn-cert.service systemd/den-turn-cert.timer /etc/systemd/system/
install -m 644 haproxy.cfg /etc/haproxy/haproxy.cfg
haproxy -c -f /etc/haproxy/haproxy.cfg
install -m 644 Caddyfile /etc/caddy/Caddyfile
caddy fmt --overwrite /etc/caddy/Caddyfile
caddy validate --config /etc/caddy/Caddyfile
install -m 644 nftables.conf /etc/nftables.conf
nft -c -f /etc/nftables.conf
nft -f /etc/nftables.conf
systemctl enable nftables
systemctl daemon-reload
systemctl enable --now caddy den-turn-cert.timer
systemctl reload caddy
systemctl enable --now haproxy
systemctl restart haproxy
# Start Den only after the database migration; LiveKit after certificate sync.
systemctl enable den-server livekit
