# Smoke cleanup

Completed 2026-09-14 against `https://denchat.app`.

## Production

Ran `den-backup` before deleting anything. The service returned `Result=success`, `ExecMainStatus=0`. Preserved a private copy at `/mnt/storage/den-cleanup-backup/den-before-cleanup.zip` before the next same-day backup could replace the daily archive. Its database contains all 39 original messages, including all 35 deleted IDs.

Deleted 35 messages and cards through the API:

| Artifact | Count |
| --- | ---: |
| Ended terminal cards, including shared cards | 15 |
| Access requests in the nicholas / m6_bob DM | 8 |
| Canvases named `smoke` in general | 2 |
| `M3 smoke` messages | 5 |
| Explicit M4 Windows/Linux notification and attachment acceptance messages | 5 |

Matched the brief's authors and rooms, including self-DMs. No `Desktop smoke` messages, `Den-M4-*` hosts, or `M9 sea glass` custom theme on m6_bob remained to delete.

Deleting a message cascades its objects but leaves uploads. Removed 14 associated uploads: 11 terminal recordings, two canvas thumbnails, and one attachment. Before deleting each database row and stored file, checked that no surviving message, object state, thumbnail, terminal session, or avatar referenced it. Kept unrelated uploads. The private exact-ID inventory and deletion report are `/mnt/storage/den-cleanup-inventory.json` and `/mnt/storage/den-cleanup-production-report.json`.

Four messages remain:

| Message ID | Room | Reason kept |
| --- | --- | --- |
| `01M25SRQ9PGY3BRYQBZW4N8RBC` | general | Original welcome message |
| `01M25WNS9JXF019XPQPT3BTFVS` | plans | Ordinary human message, “Hi” |
| `01M2DX4DNXC4CYSFHE8JQA9G2D` | DM | Ordinary human message, “Yo?” |
| `01M2DWMN959S93G6Z2CTHD5DN6` | clips | Earlier M6 deployment check with a 41-second clip, outside this brief's named cleanup scope |

Final production inventory: four messages, zero objects/access requests, two preserved uploads. SQLite `integrity_check` returned `ok`; `foreign_key_check` returned no rows.

## Future smoke runs

`scripts/smoke-cleanup.mjs` records exact creation IDs from Node fetch calls and browser responses. Each smoke runs cleanup in `finally`: close browsers, end created terminal sessions, delete created messages/cards, revoke created grants, remove associated uploads, and restore changed appearance/settings. Cleanup continues after an individual deletion failure and reports accumulated errors. It never deletes a room-wide before/after difference. That difference is used only to assert no new room artifacts remain.

Set `DEN_SMOKE_KEEP=1` to retain artifacts and changed preferences for inspection. Browser processes still close. M1 and the combined media runner normally remove their disposable database/upload directories; KEEP retains those directories. Seeded test accounts and empty DM containers are reusable prerequisites, not deleted room content.

M7b now enrolls a unique temporary host instead of rewriting the installed host configuration. Its host process and enrollment are cleaned up even on a failed run. The production host service was left running.

Added `DELETE /uploads/{id}` because recordings otherwise could not be cleaned through the API. It requires the owner or an admin with room access and rejects uploads still referenced by live content. It removes complete files, partial files, thumbnails, and recording mappings. No migration or shared contract changes were needed.

## Verification

Used isolated dev servers on 17820 and 17830/17831, Vite on 17832, and a separate LiveKit container. Builds used `CARGO_TARGET_DIR=/mnt/storage/den-m9-target`. No test joined public hangout.

All requested smoke suites passed their behavior checks and their no-new-artifacts assertions:

| Script | Cleanup verified |
| --- | --- |
| m1 | Disposable database/uploads removed; KEEP separately retained four messages |
| m3 | One message removed |
| m6-turn | No messages/objects created; direct and TURN relay media passed |
| m7a | Canvas card/object and thumbnail removed |
| m7b | Four cards/objects, including two access requests; recording and temporary host removed |
| m7c | Full layout/multiple-share/quality checks; no room artifacts created |
| m8-host | Real remote PTY check; terminal card/object and recording removed |
| m9 | Theme checks passed; message removed and original appearance restored |
| m4 native stub | Both servers passed; one message removed per server, settings/appearance restored |

Also passed M7c navigation, M8 instance name, M9 embedded clients, and the combined multi-device/M3 runner. Their created messages and objects were removed. The combined runner's entire scratch directory was confirmed absent after exit. All three persistent dev fixture databases ended with zero messages, objects, uploads, and hosts. Stopped this task's dev servers, host, Vite process, and LiveKit container.

The dedicated cleanup integration check throws after browser-message, canvas, and partial-upload creation. Both normal cleanup and KEEP passed; an existing baseline message survived. Actual M7b failures also removed their cards, requests, recording, and host. Fixed smoke timing by waiting for Bob's UI control state before typing and for M7c's layout transition before comparing widths. The combined runner waits for the login budget to reset between suites. The TURN fixture needed its reachable Tailscale address rather than loopback for relay candidates.

Server API tests: 26 passed. `cargo clippy -p den-server --all-targets -- -D warnings`, formatting, and release build passed. Logs and private screenshots are under `/mnt/storage/den-cleanup-*`.

## Deployment

Backed up again, then deployed source commit `290ef6c` using `deploy/release.sh` from the isolated worktree. This adds the cleanup endpoint to the server; the reported version remains 0.2.1. It is a source deployment, not a replacement desktop release asset.

The deployed server SHA256 matches the verified local release binary: `e35c08bcff317285aff4257aa03a2b6557e9e764e3398f5f5d09132191a36313`. Public health passed. Created and deleted a temporary upload through the public API, then verified both its metadata and file returned 404. The four preserved messages and zero-object inventory remained unchanged.

Implementation commits: `72bcc05` for upload deletion, `290ef6c` for smoke cleanup, and `828db09` for disposable media-runner cleanup.
