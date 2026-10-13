#!/bin/sh
set -eu
# Xcode Cloud runs this after cloning. It fails the build if the committed
# DenShell.xcodeproj no longer matches what project.yml generates.
cd "$(dirname "$0")/.."
den_xcodegen=$(./ci_scripts/install-xcodegen.sh)
"$den_xcodegen" generate --spec project.yml --quiet
git diff --exit-code -- DenShell.xcodeproj DenShell/Info.plist || {
    echo 'DenShell.xcodeproj is out of date. Run "xcodegen generate" in apps/ios-shell/ and commit the result.' >&2
    exit 1
}
if [ -n "$(git status --porcelain -- DenShell.xcodeproj DenShell/Info.plist)" ]; then
    echo 'xcodegen created files that are not committed.' >&2
    git status --porcelain -- DenShell.xcodeproj DenShell/Info.plist >&2
    exit 1
fi
printf '%s\n' 'Den Shell project matches project.yml.'
