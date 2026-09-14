#!/bin/sh
set -eu
if [ "$#" -ne 1 ]; then
  echo 'Usage: curl -fsSL https://denchat.app/install-host.sh | sh -s -- <enrollment-code>' >&2
  exit 1
fi
if [ "$(id -u)" -eq 0 ]; then echo 'Run this as your own user, without sudo.' >&2; exit 1; fi
case "$(uname -s):$(uname -m)" in
  Linux:x86_64) platform=linux-x86_64 ;;
  Darwin:arm64) platform=macos-arm64 ;;
  *) echo 'Supported: Linux x86_64 and Apple silicon macOS. On Windows, use install-host.ps1.' >&2; exit 1 ;;
esac
command -v curl >/dev/null || { echo 'Install curl first.' >&2; exit 1; }
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
# Resolve latest once so the binary and checksum always come from the same tag.
url=$(curl --proto '=https' --tlsv1.2 -fsSL -o /dev/null -w '%{url_effective}' https://github.com/nhclink16/den/releases/latest)
tag=${url##*/}
case "$tag" in v[0-9]*) ;; *) echo 'No published release was found.' >&2; exit 1 ;; esac
case "$tag" in *[!A-Za-z0-9.-]*) echo 'Invalid release tag.' >&2; exit 1 ;; esac
base="https://github.com/nhclink16/den/releases/download/$tag"
asset="den-host-$platform"
curl --proto '=https' --tlsv1.2 -fsSL "$base/SHA256SUMS" -o "$tmp/SHA256SUMS"
curl --proto '=https' --tlsv1.2 -fsSL "$base/$asset" -o "$tmp/den-host"
expected=$(awk -v name="$asset" '$2 == name { print $1 }' "$tmp/SHA256SUMS")
if command -v sha256sum >/dev/null; then actual=$(sha256sum "$tmp/den-host" | cut -d ' ' -f 1)
else actual=$(shasum -a 256 "$tmp/den-host" | cut -d ' ' -f 1); fi
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then echo 'Checksum mismatch. Nothing was installed.' >&2; exit 1; fi
mkdir -p "$HOME/.local/bin"
install -m 755 "$tmp/den-host" "$HOME/.local/bin/den-host.new"
mv -f "$HOME/.local/bin/den-host.new" "$HOME/.local/bin/den-host"
"$HOME/.local/bin/den-host" login "$1"
"$HOME/.local/bin/den-host" install
echo "Installed Den Host $tag. The machine will appear in Settings, Machines."
