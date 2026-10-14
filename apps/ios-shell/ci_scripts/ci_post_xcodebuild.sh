#!/bin/sh
# After the "UI check" tests: put the made-up Den's logs, LiveKit's and Den Shell's own in the Xcode Cloud log,
# for when a flow fails.
[ "${CI_WORKFLOW:-}" = 'UI check' ] && [ "${CI_XCODEBUILD_ACTION:-}" = test-without-building ] || exit 0
dir=${TMPDIR:-/tmp}/den-ui-check
echo '--- den.log (last 40 lines)'
tail -n 40 "$dir/den.log" 2>/dev/null
echo '--- livekit.log (warnings, connection states and joins)'
grep -E 'INFO|WARN|ERROR|ICE|ice|state|connect' "$dir/livekit.log" 2>/dev/null | grep -v 'sent signal response' | tail -n 150
echo '--- Den Shell log (its own lines, LiveKit, ICE)'
tail -n 200 "$dir/shell.log" 2>/dev/null
exit 0
