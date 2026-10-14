#!/bin/sh
# After the "UI check" tests: put the made-up Den's logs, LiveKit's and Den Shell's own in the Xcode Cloud log,
# for when a flow fails.
[ "${CI_WORKFLOW:-}" = 'UI check' ] && [ "${CI_XCODEBUILD_ACTION:-}" = test-without-building ] || exit 0
dir=${TMPDIR:-/tmp}/den-ui-check
echo '--- den.log (last 40 lines)'
tail -n 40 "$dir/den.log" 2>/dev/null
echo '--- livekit.log (warnings, connection states and joins)'
grep -E 'INFO|WARN|ERROR|ICE|ice|state|connect' "$dir/livekit.log" 2>/dev/null | grep -v 'sent signal response' | tail -n 150
echo '--- Den Shell log'
xcrun simctl spawn "${CI_TEST_DESTINATION_UDID:-booted}" log show --last 30m --info --style compact \
    --predicate 'process == "DenShell" AND (subsystem == "app.denchat.shell" OR subsystem CONTAINS[c] "livekit" OR messageType >= 16)' 2>&1 | tail -n 200
exit 0
