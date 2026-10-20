#!/bin/sh
set -eu
# Xcode Cloud runs this before each xcodebuild step. Only the test step of the "UI check" and "Visual tour" workflows
# does anything: it starts a made-up Den on this machine for DenShellUITests, which the simulator reaches on localhost.
# The test machine gets only ci_scripts, not the repo, so everything comes from the branch's UI check build
# (.github/workflows/ui-check.yml): den-server, this commit's web app, LiveKit and a seeded database.
case ${CI_WORKFLOW:-} in 'UI check' | 'Visual tour') ;; *) exit 0 ;; esac
[ "${CI_XCODEBUILD_ACTION:-}" = test-without-building ] || exit 0
commit=${CI_PULL_REQUEST_SOURCE_COMMIT:-$CI_COMMIT}
dir=${TMPDIR:-/tmp}/den-ui-check
rm -rf "$dir" && mkdir -p "$dir" && cd "$dir"
url="https://github.com/nhclink16/den/releases/download/ui-check/den-ui-check-$commit.tar.gz"
# GitHub Actions starts that build on push; give it time to finish if this run started first.
tries=0
until curl -fsSL --retry 2 -o den.tar.gz "$url"; do
    tries=$((tries + 1))
    [ "$tries" -lt 20 ] || { echo "No UI check build for $commit at $url" >&2; exit 1; }
    echo "Waiting for the UI check build of $commit" >&2
    sleep 30
done
tar -xzf den.tar.gz
# A plain run in the background ends with this script, so these go to launchd.
cat > den.sh <<SH
#!/bin/sh
export DEN_BIND=127.0.0.1:7000 DEN_ORIGIN=http://localhost:7000 DEN_WEB_DIR=$dir/web
export DEN_DB=$dir/data/den.db DEN_UPLOADS=$dir/data/uploads DEN_BOOTSTRAP_FILE=$dir/data/bootstrap.key
export DEN_LIVEKIT_URL=ws://localhost:7880 DEN_LIVEKIT_API_KEY=devkey DEN_LIVEKIT_API_SECRET=secret
exec $dir/den-server > $dir/den.log 2>&1
SH
# The simulator reaches LiveKit's signalling on localhost, but not its media: see test5Call.
cat > livekit.sh <<SH
#!/bin/sh
exec $dir/livekit-server --dev --bind 0.0.0.0 > $dir/livekit.log 2>&1
SH
# Den Shell's own log, for ci_post_xcodebuild.sh. The simulator boots after this script, so keep trying.
cat > shell-log.sh <<SH
#!/bin/sh
until xcrun simctl spawn ${CI_TEST_DESTINATION_UDID:-booted} log stream --level info --style compact \\
    --predicate 'subsystem == "app.denchat.shell" OR subsystem BEGINSWITH "io.livekit"' >> $dir/shell.log 2>/dev/null; do sleep 2; done
SH
# Stands in for Apple's push service, which the made-up Den can't reach: when the UI test posts a "push check"
# message, this sends the simulator the alert Den would send for it (the payload of crates/den-server/src/push.rs).
cat > push.sh <<SH
#!/bin/sh
last=
while sleep 1; do
    alert=\$(sqlite3 $dir/data/den.db "SELECT json_object('aps', json_object('alert', json_object('title', 'You were mentioned in Den',
        'body', content), 'sound', 'default', 'badge', 1, 'category', 'message'), 'type', 'notification', 'channel_id', channel_id,
        'message_id', id, 'reason', 'mention', 'to_user_id', (SELECT id FROM users WHERE username = 'ellis')) FROM messages WHERE content LIKE '%push check%' ORDER BY id DESC LIMIT 1" 2>/dev/null)
    [ -n "\$alert" ] && [ "\$alert" != "\$last" ] || continue
    last=\$alert
    printf '%s' "\$alert" > $dir/push.json
    xcrun simctl push ${CI_TEST_DESTINATION_UDID:-booted} app.denchat.ios $dir/push.json >> $dir/push.log 2>&1
done
SH
chmod +x den.sh livekit.sh shell-log.sh push.sh
for job in livekit den shell-log push; do
    launchctl submit -l "app.denchat.ui-check.$job" -- "$dir/$job.sh" || nohup "$dir/$job.sh" >/dev/null 2>&1 &
done
tries=0
until curl --noproxy '*' -fsS http://localhost:7000/health; do
    tries=$((tries + 1))
    [ "$tries" -lt 30 ] || { echo 'Den did not start' >&2; cat den.log >&2; exit 1; }
    sleep 1
done
echo
# The keyboard test needs the on-screen keyboard, not the Mac's.
defaults write com.apple.iphonesimulator ConnectHardwareKeyboard -bool false
echo "Made-up Den for $commit is up on localhost:7000"
