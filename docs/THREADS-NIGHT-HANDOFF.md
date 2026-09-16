# Threads overnight handoff to Opus

Nicholas directly requested on September 15, 2026: "hand off the rest of the work to opus for the night. i need to chill on astra".

## Ownership and authority

Opus in Herdr w11:pY now leads the remaining THREADS milestone, including design decisions, implementation, review coordination, verification, feature-branch commits, PR preparation/publication, and CI monitoring. Astra is stopping for the night. Do not wait for further Astra approval or send it routine questions. The old lane-only instructions saying "wait for Astra", "root owns commits", or "no edits until my next brief" are superseded by this handoff. Keep the actual product and system boundaries below.

Use the existing Opus peer `impl-thread-contract` in w11:pX for independent review and a separately owned native iOS lane. Agree file ownership and handoffs directly through Herdr; ask each other questions. At most these two implementation lanes at once. Monitor Herdr rather than leaving a peer silently blocked. Stop finished lanes once their work is integrated and no further assigned task remains. Do not touch unrelated Fable, SQLx, Spotify, AV or other workflows.

You may make routine design, split and verification decisions yourself. If a diff grows too large, choose another coherent PR seam rather than waiting for Astra or cutting promised behavior. Keep implementation and independent review distinct. Do not approve your own unverified claims by citing a passing helper test.

## Boundaries that remain

- No merge, deployment, release, push to main, private-profile use, real keyring unlocking, or fleet changes. Publish reviewable feature PRs, leave them unmerged.
- Shared servers :7000 and :5173, unrelated worktrees, and their targets are protected.
- Use separate worktrees for concurrent writers; stage explicit owned paths only. Preserve dirty source and failed evidence before changes/reruns. No reset --hard, clean or indiscriminate add.
- Web work needs no Rust build, SQLx/schema/migration/dependency work. Reuse the existing server/host binaries. Serialize genuinely needed heavy builds and use /mnt/storage targets, low jobs.
- PR41 remains a draft with its separately documented standalone-host SIGINT shutdown failure. Do not investigate it as part of this milestone. A clean SIGTERM exit is neither a reproduction nor a control for SIGINT.
- Native iOS IS included. Use the existing native placeholders for terminal/canvas cards in the right conversation; do not invent native object renderers.
- Genuine externally blocked acceptance may remain clearly documented in a draft PR. Do not claim an unexecuted Mac/device/OS-toast test passed. Do not interrupt Nicholas for routine choices tonight.

## Current state, verified at transfer

Main checkout /home/nicholas/den is stale local main at 5152c5e. It has unrelated untracked .pi, .shot.mjs, design directories and docs/CUA-PI-CLI-SKILL-RESEARCH.md. Do not build or integrate this feature from that checkout. This handoff is its only newly owned path.

Active source: /mnt/storage/wt-thread-web-ui, branch feat/thread-web-ui, HEAD 22d67b9d4fe81503e60f4fe9b26d506fa21febfe. Work is UNCOMMITTED, 19 modified tracked paths plus new message-fetch/routes/thread-order helpers, ThreadPanel/ThreadStrip, tests, manual runners and docs/shots/pr/thread-web-ui. No carving or publication yet.

Current source freeze: /mnt/storage/den-thread-web-state-evidence/web-ui-source-freeze.txt
Aggregate f27cd7686891da4193ba8ac503104d6b5891cfad48f18a407696832db3526b2b, 22 source files. Astra verified all 22 hashes. Store SHA cdd846b6e249dfaa0a5e92fd45dc71ea4346cc482bfad2cf2887edccfdc8e36e; message-fetch SHA 739b6d8f8b616d3acc5601f7a0abc87dceeea4ae9167628a862b85955ef1273b. Preserve this version before fixing the findings below.

Other local worktree heads, rechecked by git worktree list:

| PR | Worktree | Branch/head |
|---|---|---|
| 36 | wt-thread-contract | feat/thread-contract 9fdf55f |
| 37 | branch only | feat/thread-server 5a126b2, last recorded |
| 38 | wt-thread-ci | feat/thread-lifecycle da477ae |
| 39 | wt-thread-server | feat/thread-unread 328ff4b |
| 40 | wt-thread-objects | feat/thread-objects 79c6ce8 |
| 41 | wt-thread-cli | feat/thread-cli f20e8b3 |
| 42 | wt-thread-web-state | feat/thread-web-state 22d67b9 |

PR36-42 were last reported open, expected CI green; PR41 draft/manual failure persists. Remote state was not refreshed at this transfer. Verify exact heads and base branches before publishing anything. PR42 is based on PR40, independent of CLI PR41.

## First work: finish the fetch correction completely

Read /mnt/storage/den-implementation-briefs/thread-web-ui-fetch-rule.md. The request-scoped delta model remains the approved design; the new module is only partly wired. The latest correction is not approved for shipping yet.

Astra executed actual Store/ReadState/MessageFetches through TypeScript transpilation, with rune cells and external services stubbed. Controlled promises, no browser/network claim. The new preserved script and result are:

- /mnt/storage/den-thread-web-state-evidence/astra-review4/fetch-boundaries.cjs
- /mnt/storage/den-thread-web-state-evidence/astra-review4/fetch-boundaries-before.json

Do NOT run that script unchanged after editing: it writes the BEFORE result path. Make a copy with a distinct AFTER path. Earlier astra-review2 and astra-review3 scripts/results must also remain untouched.

Seven observed failures on f27cd768:

1. Room loadLatest crossed by live root 020 ends with [010], expected [010,020]. Room loadLatest and loadOlder still bypass the registry, ordering and account cancellation entirely.
2. loadRoot on a cached root makes zero HTTP requests and leaves content "old", expected "edited offline". A refresh cannot use fetchMessage's cache hit as an authoritative server read.
3. loadRoot held across forgetThread returns and rebuilds the removed root. Root requests have threadId undefined, so current cancellation misses them.
4. loadOlderThreadReplies held from [040,050], followed by latest replacement [100,110], then old response [020,030], ends [020,030,100,110]. It invents contiguous history across a gap.
5. refreshThreads with a held root/window calls only latest replies and metadata. Neither root nor window is fetched. Reconnect repair has not been implemented for them.
6. beforeRange([],050,25) and afterRange([],050,25) return an empty mathematical interval. An empty response is a SHORT page and proves the requested end; it must remove stale entries inside that established range.
7. Registry begin replacement A, begin replacement B, end B, then cancel an unrelated view makes A valid again. cancel prunes the newest map because B ended, resurrecting A's validity.

### Exact answer to the pending identity question

Rooms and threads can use the SAME latest operation kind. Conversation identity separates them: room(channelId) versus thread(threadId), with no possibility of a room/thread collision. A thread key must not change when previously missing channel metadata arrives.

Separate the identity of the CACHE BEING WRITTEN from the operation mode. Latest and older requests that write the same tail cache share a cancellation/order identity; kind='older' and its page edge must not make them independent caches. A root is a separate view keyed by root id. A window is a separate view keyed by conversation plus target; open and extend must use that same target representation, not target for one and whole window key for the other.

A small way to fix the registry: a replacement synchronously cancels the older conflicting open requests, so superseded IDs can never become valid again. There need not be a permanent newest map after requests finish. Serialize or refuse older-page dispatch while a replacement for that same cache is pending. Different views remain independent; never await a per-view acknowledgement from the single WS handler chain. Clear cancellation-owned spinners synchronously; old finally blocks cannot clear a newer spinner.

Wire room latest/older through the same rule and account lifetime. Give actual root/window refreshes a server fetch, preserving existing content on transient failure and offering Retry. Reconnect refreshes loaded roots/windows without blocking WS delivery; a bounded target-window replacement resets outer paging, as already designed. Root removal must cancel by the root belonging to the removed thread even though the root message itself has thread_id null. Recheck original resync epoch before scheduling each follow-up.

Fix empty ranges to match short-page authority. Keep full-page limits, cross-conversation replay filtering and out-of-window puts correct. Remove stale mergeReplies/edited_at comments that no longer describe the implementation.

Also inspect the actual callers: closeWindow currently has no UI caller, only forgetThread, so navigation/Jump to newest never exercises the claimed close cancellation. The ThreadPanel reply load and ChannelView room load still have no error handling for a rejected initial page. Close/refresh/error claims need actual entry points, not just private methods. These last two are inspection findings, not executed red runs.

### Acceptance for this correction

Keep the old original-three and review3 probes green and make the seven new observations match expected results. Add a few meaningful, tracked regressions for request lifetime/range/order through actual Store callers where wiring matters. Current 65 node tests never import message-fetch.ts; that count does not establish its behavior. Do not add a large test framework, stress loop or a second browser matrix.

Preserve failure output. Independent Opus review should inspect all load paths, cancellation, reconnect and rendered copies. Source plus focused tests was 2452 lines before these fixes; count the actual PR after them. Finish narrow checks first, then one final affected web node/check/build and bridge pass. Run the existing full combined browser/Electron acceptance only after the final source is ready.

## Electron evidence

The original CDP zoom claim was false: applied:true only meant no exception, while DPR=1 and CSS width stayed700. It is preserved and relabelled.

The new isolated actual webContents probe JSON was read at transfer and records:
- Native window stays1200x900.
- Before getZoomFactor1, CSS width1200, DPR1.
- After getZoomFactor2, CSS width600, DPR2.
- Panel, way back, draft and no horizontal overflow asserted.

Evidence: /mnt/storage/den-thread-web-state-evidence/web-ui-electron-pass/zoom-probe/{probe.mjs,zoom-probe.json,zoom-before.png,zoom-after.png,README.md}. Astra read the JSON but has not inspected the new images/probe implementation. Have the peer finish that review and integrate the real mechanism into the final manual acceptance runner without product IPC.

Disposable Secret Service recipe and provenance: /mnt/storage/den-thread-electron-environment-evidence/disposable-secret-service.sh and RESULT.md. Use private XDG paths and dbus-run-session; wait with non-activating ListNames; assert owner PID equals your daemon. Never repurpose HOME/CODEX_HOME or replace/unlock the real daemon. Verify local origin before credentials. Existing Electron native landscape,820px portrait,narrow,keyboard evidence stands as those runs; PAIR=920 fixed the real820 portrait failure. Exact source/bundle provenance matters for final acceptance.

## Packaging and publication

Read /mnt/storage/den-implementation-briefs/thread-web-ui-packaging.md. Its root-only approval/publication restriction is superseded by Nicholas's overnight delegation; its small-PR structure remains:

A. Manual browser/Electron runners on #42, explicit baseline mode and dependency notes.
B. Complete web/Electron activation stacked on A, including focused tests and final evidence. Do not cut old-target windows or ship roots-only without reachable UI.

After independent review, freeze the combined source/runners/evidence with hashes, cut commits by explicit paths and prove the combined tree matches. Publish both linked PRs unmerged, link exact runner revision in the activation body, monitor each PR's own CI run by head SHA and event type rather than a duplicate push check. If required gates remain genuinely blocked, use draft status and a precise reason. No hidden feature cuts or 3500-line omnibus PR.

## Native iOS, remaining milestone work

The contract lane has already audited 22d67b9. Start from docs/DIRECTION.md, docs/THREADS-PLAN.md and /mnt/storage/den-implementation-briefs/thread-client-acceptance.md. Author the native brief yourself from that audit, then implement in its own worktree. No API gap or schema regeneration is currently known.

Use two coherent PRs:
1. Service wrappers plus read ordering/account lifetime and store-owned draft/upload conversation identity, preserving existing flat UI behavior.
2. Complete activation: roots-only listing AND scoped reads, visible open strip, reachable thread view, resolve/reopen/rename/follow, unread resolved reachability, search/deep-link/notification routing, typing, correct object placeholder placement. Do not separate navigation/notifications from activation and leave hidden unread.

Preserve product decisions: no follow on passive opening; posting re-follows but does not advance an existing position; Follow advances to current tail; Unfollow is not a DM/subscription mute. Reading the room never reads collapsed replies; explicit mark-all reads flat through a captured tail. Resolve stops new content, never a PTY or canvas edit. ThreadSummary is metadata, ThreadView includes personal read state. Filtered/paged results never prove absence. Unknown WS variants are already tolerated; no new opt-in flag.

Audit hotspots: DenService.swift wrappers; AppStore.swift and Realtime.swift unconditional read-state overwrite/account reset; ComposerView.swift local draft and text-equality clearing; Uploads.swift channel-only queues; OfflineCache.swift persistence/reset; ConversationView markTailRead and targetMessageId; Notifications.swift channel-only suppression/tap. Missing thread context in old push payloads must not falsely certify a collapsed conversation as on screen.

The committed apps/ios/ci_scripts/fixture-source.tar.gz stops at migration0018. Repackage via package-fixture-source.py when native acceptance uses that fixture. This is the Xcode Cloud fixture path, not a prerequisite for every GitHub stub test or local explicit-server run. GitHub native CI uses DenTests; no local Mac build has been executed in this milestone. Follow current native README/project instructions, explicit generated-project staging, package-pin stability and ad-hoc signing. Existing docs specify Xcode26.6, Swift6.3.3, XcodeGen2.45.4 and iOS26.5; verify before running. CODE_SIGNING_ALLOWED=NO breaks keychain tests; use documented ad-hoc configuration.

Do not add terminal/canvas renderers: native currently shows the desktop-open placeholder. Prove task cards appear in the correct thread and label that limitation honestly.

## Morning deliverable

Leave a concise THREADS-NIGHT-RESULTS.md beside this handoff, with PR links/heads/bases, implemented pieces, actual tests and device/browser evidence, remaining blockers, all owned processes cleaned up, and next human actions. Keep source/evidence claims separate. No routine Astra wake-ups. If externally blocked on one task, record it and continue independent authorized work.

## Fresh-thread prompt

You are Opus leading Den THREADS overnight by Nicholas's direct delegation. Read /home/nicholas/den/docs/THREADS-NIGHT-HANDOFF.md, verify Herdr inherited context and the active worktree HEAD/status, then continue the fetch correction using the preserved actual-Store red probe. Coordinate independent review/native work with impl-thread-contract. Own bounded decisions and feature PRs without waiting for Astra. Keep no-merge/no-deploy and all isolated-resource boundaries. Finish web/Electron, then native iOS, with separate reviewable PRs and a morning result file.
