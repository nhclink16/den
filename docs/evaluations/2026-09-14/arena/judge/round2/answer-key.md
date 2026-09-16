# Round 2 sealed answer key

Status: verified. The nine-state matrix produced exactly the expected failures and passes. Direct read-state baseline and restored cases passed, and its isolated mutant failed on an explicit socket-owner mismatch. See results.json and direct-read-results.json. Storage guards interrupted preparation; no scored contestant work was involved.

Baseline: archived Den commit e188e4e. No main-worktree edits. The proposal changes two server files in four small hunks. No dependencies or API types change.

## Real regression: another member's read state reaches shared-channel sockets

`ws.rs`, `allowed`, `Event::ReadStateUpdated` loses the recipient user-ID guard. The remaining channel visibility check does not substitute for recipient identity. Members of a shared channel receive another member's last-read position and unread state.

Direct check: `arena_review::read_state_events_are_owner_only_in_shared_channel`. Alice posts public messages while Bob and admin have open authenticated sockets. A later public-message barrier makes the event order observable. Each socket must receive only its owner's read-state events. The assertion compares the recipient socket owner with every delivered read-state user ID before the barrier. The existing `m2_live::websocket_targets_notifications_and_read_state_without_dm_leaks` also fails, but first on downstream notification ordering, so the supplemental direct test establishes the privacy claim separately.

Do not credit a claim that this leaks private DM messages to nonmembers. The shared-channel visibility check remains intact. The read-state event is the demonstrated privacy leak.

## Real regression: cookie writes no longer require both origin and CSRF validation

`auth.rs`, `Auth::from_request_parts`, changes the rejection condition from OR to AND. A cookie-authenticated state-changing request is now accepted when either check succeeds. The original contract requires both to succeed.

Concrete check: `auth::auth_consumes_invites_protects_cookies_and_revokes_credentials`. Log in as Alice. POST `/tokens` with her session cookie, `Origin: https://evil.example`, her correct `X-CSRF-Token`, and a token name. Expected 403; the CSRF-only mutant returns 200 and creates a token. Correct origin without a matching CSRF token is the other logical bypass, not separately executed by this existing test.

Keep severity claims bounded. This proof uses a known session cookie and CSRF token. It does not establish cookie theft, an unauthenticated bypass, or an arbitrary third-party website obtaining the token. The change affects cookie writes, not bearer requests. The independent WebSocket Origin check remains.

## Harmless: unstable sorting of online IDs

`sort()` to `sort_unstable()` preserves lexicographic output for String IDs. These IDs come from distinct HashMap keys, and each online user remains represented once even with multiple sockets. Sort stability cannot expose a different equal-key record here.

Check: `arena_review::presence_order_remains_sorted_and_unique_across_tabs`. Open Bob, Alice, admin and a second Alice socket, then assert the exact sorted three-ID presence list. Prove the check can fail using deterministic descending sort, then restore.

## Harmless: split/trim cookie parsing refactor

Moving `trim()` into `.map(str::trim)` preserves token order, whitespace handling, prefix matching and the first matching cookie. Both forms use the same lazy iteration and `find_map` short-circuit behavior.

Check: `arena_review::cookie_parser_preserves_trim_and_first_matching_cookie`. A whitespace-prefixed valid first cookie followed by an invalid duplicate authenticates the same user; an invalid first cookie followed by a valid duplicate stays unauthorized. Prove the check can fail by deleting trim, then restore.

## Reproduction artifacts

- `review-proposal.diff`: only file to share with contestants, together with `candidate-prompt.md`.
- `prove.py`: runs the matrix in the isolated judge copy, monitors free space, records each test exit status and restores both server source files in a finally block.
- `arena_review.rs`: three additional behavioral checks; judge only.
- Existing API integration tests supply the two regression proofs without modifying their assertions.
- Each `*-build.log` / `*-read.log` / `*-csrf.log` / `*-sort.log` / `*-cookie.log` records actual execution.

Run `python3 prove.py` to reproduce the full proof in the task's isolated judge copy. It installs its hidden test module, runs the matrix, restores server and test sources, and rebuilds the normal baseline test target without hidden checks. The script pins the task's absolute paths and does not edit the archived baseline. All mutations are judge-only. The initial execution used the existing realtime test, then `prove-direct-read.py` independently established the direct owner mismatch. Subsequent full reproductions use the direct test for the read-state row.
