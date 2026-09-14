#!/bin/sh
set -eu
cd "$(dirname "$0")"
case "${CI_XCODEBUILD_ACTION:-}" in
    test-without-building)
        # Set this same, nonsecret path in the workflow Environment for XCTest propagation.
        : "${DEN_UI_FIXTURE_PATH:?Set DEN_UI_FIXTURE_PATH=/tmp/den-ios-cloud-fixture.json in the Cloud workflow}"
        if [ "${DEN_UI_REQUIRE_FIXTURE:-}" != '1' ]; then
            echo 'Set DEN_UI_REQUIRE_FIXTURE=1 in the Cloud workflow so missing fixtures fail UI tests.' >&2
            exit 1
        fi
        den_server=$(./build-fixture-server.sh)
        python3 ./fixture.py start --binary "$den_server" --credentials "$DEN_UI_FIXTURE_PATH"
        ;;
    *) printf '%s\n' 'This action does not run tests; no fixture server started.' ;;
esac
