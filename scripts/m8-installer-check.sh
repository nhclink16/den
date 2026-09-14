#!/usr/bin/env bash
# A corrupted release download must fail before replacing or running a host.
set -euo pipefail
cd "$(dirname "$0")/.."
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
cat > "$fixture/curl" <<'SH'
#!/bin/sh
out=
url=
while [ "$#" -gt 0 ]; do
  case "$1" in -o) shift; out=$1 ;; https://*) url=$1 ;; esac
  shift
done
case "$url" in
  */latest) printf '%s' 'https://github.com/nhclink16/den/releases/tag/v0.0.0' ;;
  */SHA256SUMS) printf '%064d  den-host-linux-x86_64\n%064d  den-host-macos-arm64\n' 0 0 > "$out" ;;
  *) printf '%s' 'corrupted release bytes' > "$out" ;;
esac
SH
chmod +x "$fixture/curl"
if PATH="$fixture:$PATH" sh deploy/install-host.sh test-code > "$fixture/result" 2>&1; then
  echo 'FAIL: corrupted binary accepted' >&2; exit 1
fi
grep -q 'Checksum mismatch. Nothing was installed.' "$fixture/result"
echo 'PASS: corrupt release rejected before installation or enrollment'
