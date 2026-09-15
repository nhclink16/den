#!/bin/sh
# Install the optional external music tools into a caller-chosen bin directory.
set -eu
if [ "$#" -ne 1 ]; then echo "Usage: scripts/install-music.sh BIN_DIRECTORY" >&2; exit 2; fi
bin=$(mkdir -p "$1" && cd "$1" && pwd)
repo=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
command -v ffmpeg >/dev/null
command -v node >/dev/null
command -v go >/dev/null
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT HUP INT TERM
version=2026.08.19
curl -fL "https://github.com/yt-dlp/yt-dlp/releases/download/$version/yt-dlp" -o "$work/yt-dlp"
curl -fL "https://github.com/yt-dlp/yt-dlp/releases/download/$version/SHA2-256SUMS" -o "$work/sums"
(cd "$work" && awk '$2 == "yt-dlp" {print}' sums > check && test -s check && sha256sum -c check)
(cd "$repo/tools/den-dj" && go build -trimpath -o "$work/den-dj" .)
install -m 755 "$work/yt-dlp" "$bin/yt-dlp"
install -m 755 "$work/den-dj" "$bin/den-dj"
printf 'Installed yt-dlp %s and den-dj in %s\n' "$version" "$bin"
