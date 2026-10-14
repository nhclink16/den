#!/bin/sh
# After the "UI check" tests: show the made-up Den's logs in the Xcode Cloud log, for when a flow fails.
[ "${CI_WORKFLOW:-}" = 'UI check' ] && [ "${CI_XCODEBUILD_ACTION:-}" = test-without-building ] || exit 0
dir=${TMPDIR:-/tmp}/den-ui-check
for log in den livekit; do
    echo "--- $log.log (last 60 lines)"
    tail -n 60 "$dir/$log.log" 2>/dev/null
done
exit 0
