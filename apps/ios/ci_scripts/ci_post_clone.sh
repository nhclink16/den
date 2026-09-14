#!/bin/sh
set -eu
cd "$(dirname "$0")"
den_xcodegen=$(./install-xcodegen.sh)
python3 ./verify-project.py --xcodegen "$den_xcodegen"
python3 ./package-fixture-source.py --check
# Apple's generator requires plugin trust on the Cloud worker. CI_BUILD_ID is
# always available there; checking it also keeps local Cloud-verifier probes
# from changing this Mac's Xcode preferences. The documented key has this spelling.
if [ "${CI_XCODE_CLOUD:-}" = "TRUE" ] && [ -n "${CI_BUILD_ID:-}" ]; then
    defaults write com.apple.dt.Xcode IDESkipPackagePluginFingerprintValidatation -bool YES
fi
# Cloud resolves committed Package.resolved. Do not resolve against live versions here.
# The OpenAPI plugin consumes the committed snapshot; do not fetch the production API.
printf '%s\n' 'Den Cloud checkout checks passed.'
