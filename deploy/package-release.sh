#!/usr/bin/env bash
# The same build/package path is used by Actions and native fallback builders.
set -euo pipefail
cd "$(dirname "$0")/.."
platform=${1:?Usage: package-release.sh <platform> [output-dir]}
out=${2:-release-artifacts}
mkdir -p "$out"
out=$(cd "$out" && pwd)
suffix=
case "$platform" in
  linux-x86_64) packages=(-p den-server -p den -p den-host); target=x86_64-unknown-linux-gnu ;;
  macos-arm64) packages=(-p den -p den-host); target=aarch64-apple-darwin ;;
  windows-x86_64) packages=(-p den -p den-host); suffix=.exe; target=x86_64-pc-windows-msvc ;;
  *) echo "Unsupported release platform: $platform" >&2; exit 1 ;;
esac
[[ $(rustc -vV | sed -n 's/^host: //p') = "$target" ]] || { echo "Use a native $target builder" >&2; exit 1; }
cargo build --locked --release "${packages[@]}"
bin="${CARGO_TARGET_DIR:-target}/release"
cp "$bin/den-host$suffix" "$out/den-host-$platform$suffix"
cp "$bin/den$suffix" "$out/den-$platform$suffix"
if [[ $platform = linux-x86_64 ]]; then
  npm --prefix apps/web ci
  npm --prefix apps/web run build
  stage=$(mktemp -d "$out/.bundle.XXXXXX")
  trap 'rm -rf "$stage"' EXIT
  cp "$bin/den-server" "$bin/den" "$bin/den-host" "$stage/"
  cp -R apps/web/dist "$stage/web"
  cp -R deploy "$stage/deploy"
  cp LICENSE "$stage/"
  cp -R LICENSES "$stage/LICENSES"
  tar -czf "$out/den-server-linux-x86_64.tar.gz" -C "$stage" .
fi
