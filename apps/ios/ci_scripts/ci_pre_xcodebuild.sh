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
        : "${CI_TEST_DESTINATION_UDID:?Cloud must provide the exact simulator UUID for first-run privacy reset}"
        den_server=$(./build-fixture-server.sh)
        python3 ./fixture.py start --binary "$den_server" --credentials "$DEN_UI_FIXTURE_PATH"
        # The full UI suite includes first-use dictation. Never substitute a pregrant
        # or a guessed booted device for the exact destination selected by Cloud.
        if ! python3 ./reset-dictation-privacy.py --simulator "$CI_TEST_DESTINATION_UDID" --fixture "$DEN_UI_FIXTURE_PATH"; then
            python3 ./fixture.py stop --credentials "$DEN_UI_FIXTURE_PATH"
            exit 1
        fi
        ;;
    *) printf '%s\n' 'This action does not run tests; no fixture server started.' ;;
esac
