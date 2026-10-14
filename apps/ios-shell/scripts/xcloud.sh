#!/bin/bash
# Start a Den Shell build in Xcode Cloud, wait for it, and download its logs and test results.
#   xcloud.sh run [workflow] [branch] [out-dir]   start, wait, download (defaults: "Branch check", current branch)
#   xcloud.sh start [workflow] [branch]           print the new build run's id
#   xcloud.sh wait <run-id>                       print progress until it finishes; exit 1 unless it succeeded
#   xcloud.sh fetch <run-id> [out-dir]            download its logs and .xcresult bundles (test screenshots are inside)
#   xcloud.sh ui [branch] [out-dir]               "UI check": screenshots of Den Shell running the branch's web app
#                                                 against made-up people, as PNG files in <out-dir>/shots
# Goes through the App Store Connect key on the iMac (`ssh imac`); the key never leaves it.
# The branch must be pushed to GitHub first. Compute time is shared with H3: 25 hours a month.
set -euo pipefail
product=7d8a3fb2-c2d5-4b41-95c1-63408aa227bf  # Xcode Cloud product "DenShell"

# Prints the API's JSON; fails on an ssh error or an HTTP error status.
asc() {
    local q="" out
    for a in "$@"; do q="$q $(printf '%q' "$a")"; done
    out=$(ssh -n imac "export PATH=\$HOME/.bun/bin:\$PATH; cd ~/.config/h3/asc/bin && bun asc.ts $q") || return 1
    jq -e '.status < 400' <<<"$out" >/dev/null || { echo "App Store Connect: $out" >&2; return 1; }
    printf '%s\n' "$out"
}

workflow_id() {
    asc "/v1/ciProducts/$product/workflows" | jq -er --arg n "$1" '.data[] | select(.attributes.name == $n) | .id'
}

start() {
    local workflow=${1:-Branch check} branch=${2:-$(git branch --show-current)} wf repo ref
    wf=$(workflow_id "$workflow")
    repo=$(asc "/v1/ciWorkflows/$wf/repository" | jq -er '.data.id')
    ref=$(asc "/v1/scmRepositories/$repo/gitReferences?limit=200" |
        jq -er --arg b "refs/heads/$branch" '.data[] | select(.attributes.canonicalName == $b) | .id') ||
        { echo "Xcode Cloud can't see branch $branch. Push it first." >&2; exit 1; }
    asc /v1/ciBuildRuns --method POST --data "$(jq -nc --arg wf "$wf" --arg ref "$ref" '{data: {type: "ciBuildRuns",
        relationships: {workflow: {data: {type: "ciWorkflows", id: $wf}},
                        sourceBranchOrTag: {data: {type: "scmGitReferences", id: $ref}}}}}')" |
        jq -er '.data.id // error(tostring)'
}

wait_run() {
    local run=$1 state
    while :; do
        state=$(asc "/v1/ciBuildRuns/$run" | jq -r '.data.attributes | "\(.executionProgress) \(.completionStatus)"') || return 1
        echo "$(date +%T) $state" >&2
        [ "${state%% *}" = COMPLETE ] && break
        sleep 60
    done
    asc "/v1/ciBuildRuns/$run/actions" | jq -r '.data[].attributes | "\(.name): \(.completionStatus)"' >&2 || return 1
    [ "${state#* }" = SUCCEEDED ]
}

# Logs and .xcresult bundles only (test screenshots are inside the .xcresult); skips test products and archives.
fetch() {
    local run=$1 out=${2:-xcloud-$1} actions action list name url
    mkdir -p "$out"
    actions=$(asc "/v1/ciBuildRuns/$run/actions" | jq -r '.data[].id') || return 1
    for action in $actions; do
        list=$(asc "/v1/ciBuildActions/$action/artifacts?limit=200" | jq -r '.data[].attributes |
            select(.fileType == "LOG_BUNDLE" or .fileType == "RESULT_BUNDLE") | "\(.fileName)\t\(.downloadUrl)"') || return 1
        while IFS=$'\t' read -r name url; do
            [ -n "$name" ] || continue
            echo "$out/$name" >&2
            curl -fsSL --retry 2 -o "$out/$name" "$url" || return 1
        done <<<"$list"
    done
}

# Screenshots the result bundles hold, as <out>/shots/<test>-<name>.png.
shots() {
    local out=$1 zip
    for zip in "$out"/*.zip; do
        [ -e "$zip" ] || continue
        python3 "$(dirname "$0")/xcresult-shots.py" "$zip" "$out/shots" || return 1
    done
}

# "UI check" needs the branch's test server build from GitHub Actions (.github/workflows/ui-check.yml), which a
# push starts. Wait for it (or start it), then run the workflow and bring the screenshots back.
ui() {
    local branch=${1:-$(git branch --show-current)} out=${2:-} sha runs ghrun run status=0
    sha=$(gh api "repos/nhclink16/den/commits/$branch" -q .sha) || { echo "GitHub has no branch $branch. Push it first." >&2; exit 1; }
    if ! gh api repos/nhclink16/den/releases/tags/ui-check -q '.assets[].name' 2>/dev/null | grep -qx "den-ui-check-$sha.tar.gz"; then
        runs="repos/nhclink16/den/actions/workflows/ui-check.yml/runs?head_sha=$sha"
        ghrun=$(gh api "$runs" -q '.workflow_runs[0].id // empty')
        if [ -z "$ghrun" ]; then
            gh workflow run ui-check.yml -R nhclink16/den --ref "$branch"
            for _ in 1 2 3 4 5 6; do sleep 10; ghrun=$(gh api "$runs" -q '.workflow_runs[0].id // empty'); [ -z "$ghrun" ] || break; done
        fi
        [ -n "$ghrun" ] || { echo "No UI check server build for $sha on GitHub." >&2; exit 1; }
        echo "waiting for the test server build: https://github.com/nhclink16/den/actions/runs/$ghrun" >&2
        gh run watch "$ghrun" -R nhclink16/den --exit-status --interval 30 >/dev/null ||
            { echo "The test server build failed: https://github.com/nhclink16/den/actions/runs/$ghrun" >&2; exit 1; }
    fi
    run=$(start "UI check" "$branch")
    echo "build run $run" >&2
    out=${out:-xcloud-$run}
    wait_run "$run" || status=$?
    fetch "$run" "$out" || status=1
    shots "$out" || status=1
    ls "$out/shots" >&2 2>/dev/null || true
    exit $status
}

case ${1:-} in
    start) shift; start "$@" ;;
    wait) wait_run "$2" ;;
    fetch) fetch "$2" "${3:-}" ;;
    ui) shift; ui "$@" ;;
    run)
        shift
        run=$(start "${1:-Branch check}" "${2:-}")
        echo "build run $run" >&2
        status=0; wait_run "$run" || status=$?
        fetch "$run" "${3:-}" || status=1
        exit $status ;;
    *) sed -n '2,10p' "$0"; exit 2 ;;
esac
