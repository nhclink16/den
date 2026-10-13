#!/bin/sh
set -eu
# Release asset + digest from https://github.com/yonaskolb/XcodeGen/releases/tag/2.45.4.
den_tool_dir="${TMPDIR:-/tmp}/den-xcodegen-2.45.4"
den_binary="$den_tool_dir/xcodegen/bin/xcodegen"
# macOS clears old files from TMPDIR but can leave the binary, so check a preset too.
if [ ! -x "$den_binary" ] || [ ! -f "$den_tool_dir/xcodegen/share/xcodegen/SettingPresets/base.yml" ]; then
    mkdir -p "$den_tool_dir"
    curl --fail --location --silent --show-error --retry 2 \
      'https://github.com/yonaskolb/XcodeGen/releases/download/2.45.4/xcodegen.zip' \
      --output "$den_tool_dir/xcodegen.zip"
    printf '%s  %s\n' '090ec29491aad50aec10631bf6e62253fed733c50f3aab0f5ffc86bc170bdbef' \
      "$den_tool_dir/xcodegen.zip" | shasum -a 256 --check >&2
    unzip -q -o "$den_tool_dir/xcodegen.zip" -d "$den_tool_dir"
fi
[ "$("$den_binary" --version)" = 'Version: 2.45.4' ] || { echo 'Unexpected XcodeGen version' >&2; exit 1; }
printf '%s\n' "$den_binary"
