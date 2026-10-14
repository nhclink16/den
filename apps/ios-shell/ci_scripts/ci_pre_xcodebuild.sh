#!/bin/sh
set -eu
# Xcode Cloud runs this before each xcodebuild step. Only the "UI check" workflow's test step does anything:
# it starts a made-up Den on this machine for DenShellUITests, which the simulator reaches on localhost.
# The test machine gets only ci_scripts, not the repo, so everything comes from the branch's UI check build
# (.github/workflows/ui-check.yml): den-server, this commit's web app, LiveKit and a seeded database.
[ "${CI_WORKFLOW:-}" = 'UI check' ] && [ "${CI_XCODEBUILD_ACTION:-}" = test-without-building ] || exit 0
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
cat > livekit.sh <<SH
#!/bin/sh
exec $dir/livekit-server --dev --bind 0.0.0.0 > $dir/livekit.log 2>&1
SH
# Den Shell's own log, for ci_post_xcodebuild.sh. The simulator boots after this script, so keep trying.
cat > shell-log.sh <<SH
#!/bin/sh
until xcrun simctl spawn ${CI_TEST_DESTINATION_UDID:-booted} log stream --level info --style compact \\
    --predicate 'process == "DenShell"' >> $dir/shell.log 2>/dev/null; do sleep 2; done
SH
chmod +x den.sh livekit.sh shell-log.sh
# LiveKit's media reaches the simulator over this machine's own addresses, not localhost, so the macOS
# firewall must let it in.
fw=/usr/libexec/ApplicationFirewall/socketfilterfw
echo "Firewall: $($fw --getglobalstate 2>&1)"
{ sudo -n "$fw" --add "$dir/livekit-server" && sudo -n "$fw" --unblockapp "$dir/livekit-server"; } ||
    echo "Couldn't add LiveKit to the firewall" >&2
for job in livekit den shell-log; do
    launchctl submit -l "app.denchat.ui-check.$job" -- "$dir/$job.sh" || nohup "$dir/$job.sh" >/dev/null 2>&1 &
done
tries=0
until curl --noproxy '*' -fsS http://localhost:7000/health; do
    tries=$((tries + 1))
    [ "$tries" -lt 30 ] || { echo 'Den did not start' >&2; cat den.log >&2; exit 1; }
    sleep 1
done
echo
for _ in $(seq 20); do curl --noproxy '*' -fs -m 2 http://localhost:7880 >/dev/null && break; sleep 1; done
for ip in $(ifconfig | awk '/inet / && $2 != "127.0.0.1" { print $2 }'); do
    echo "LiveKit on $ip:7880: $(curl --noproxy '*' -sS -m 3 "http://$ip:7880" 2>&1)"
done
# The keyboard test needs the on-screen keyboard, not the Mac's.
defaults write com.apple.iphonesimulator ConnectHardwareKeyboard -bool false
echo "Made-up Den for $commit is up on localhost:7000"
