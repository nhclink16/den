#!/usr/bin/env bash
# Sign the Electron update manifests with the Den update key (Tauri format).
# Use minisign's legacy mode: the app verifies inside Electron, whose BoringSSL
# has no BLAKE2b, so prehashed signatures (what `tauri signer` makes) fail there.
set -euo pipefail
cd "$(dirname "$0")/.."
dir=${1:?Usage: sign-electron-manifests.sh <dir>}
: "${TAURI_SIGNING_PRIVATE_KEY:?}"
pubkey=$(sed -n "s/^const publicKey = '\(.*\)'$/\1/p" apps/desktop/electron/updater.cjs)
key=$(mktemp)
trap 'rm -f "$key"' EXIT
base64 -d <<<"$TAURI_SIGNING_PRIVATE_KEY" > "$key"
shopt -s nullglob
manifests=("$dir"/electron-*.json)
(( ${#manifests[@]} )) || { echo "no Electron manifests in $dir" >&2; exit 1; }
for manifest in "${manifests[@]}"; do
  printf '%s\n' "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" | minisign -S -l -s "$key" -m "$manifest" -x "$manifest.minisig" >/dev/null
  minisign -V -q -P "$pubkey" -m "$manifest" -x "$manifest.minisig"
  base64 -w0 "$manifest.minisig" > "$manifest.sig"
  rm "$manifest.minisig"
done
