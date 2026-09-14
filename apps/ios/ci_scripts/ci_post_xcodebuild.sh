#!/bin/sh
set -eu
cd "$(dirname "$0")"
if [ "${CI_XCODEBUILD_ACTION:-}" = 'test-without-building' ]; then
    : "${DEN_UI_FIXTURE_PATH:?Missing Cloud UI fixture path}"
    python3 ./fixture.py stop --credentials "$DEN_UI_FIXTURE_PATH"
fi
# No custom credential/log artifacts are exported. XCTest screenshots live in its xcresult.
printf 'Den action finished with xcodebuild status %s.\n' "${CI_XCODEBUILD_EXIT_CODE:-unknown}"
