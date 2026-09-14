#!/usr/bin/env bash
set -euo pipefail
cert=$(find /var/lib/caddy/.local/share/caddy/certificates -type f -path '*/rtc.denchat.app/rtc.denchat.app.crt' -print -quit)
[[ -n "$cert" ]] || { echo 'Caddy has not issued the TURN certificate yet' >&2; exit 1; }
key=${cert%.crt}.key
openssl x509 -checkend 0 -noout -in "$cert" >/dev/null
[[ $(openssl x509 -in "$cert" -pubkey -noout | sha256sum) = $(openssl pkey -in "$key" -pubout | sha256sum) ]]
if cmp -s "$cert" /etc/livekit/certs/turn.crt && cmp -s "$key" /etc/livekit/certs/turn.key; then exit 0; fi
install -d -o root -g livekit -m 750 /etc/livekit/certs
install -o root -g livekit -m 640 "$cert" /etc/livekit/certs/turn.crt
install -o root -g livekit -m 640 "$key" /etc/livekit/certs/turn.key
systemctl try-restart livekit.service
