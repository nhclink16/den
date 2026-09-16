# Repair cross-judgment

## Decision

Prefer **B**. Both candidates repair the two production defects by inspection. B makes the smaller production change and adds a regression test through the actual CLI, reconnect loop, handshake, and output. A's tests exercise helpers without proving that the reconnect loop uses them correctly.

This is a read-only source judgment of neutral frozen candidates A and B. I read their source, diff, status, response, and supplied reproduction script. I did not read native transcripts or authorship metadata, run builds or tests, modify either candidate, or use outside research. Reported executions below are candidate claims, not independently witnessed executions. Independent hidden checks are being run separately.

## Production correctness

| Requirement | A | B |
| --- | --- | --- |
| Authenticate every reconnect | `stream.rs:30-35,48-50` constructs a fresh authenticated request each iteration | `stream.rs:24-31` parses the header once and clones it into each fresh request |
| Stop on 401 and 403 | Helper at `14-15`, used at `86-89` | Direct match at `80-83` |
| Retry 503 | Falls through to existing reconnect/backoff at `91-95` | Falls through at `85-89` |
| Channel filtering and global events | Existing expression moved to `passes_filter`, same event variants and conditions | Existing implementation untouched |
| Ping, close, gap, JSON output | Existing branches and output calls preserved | Existing branches and output calls untouched |

I found no production behavior regression in either change. Both preserve backoff reset and socket timeouts. Neither invents event replay. Parsing a constant bearer header once, as B does, does not create stale credentials relative to A: both borrow the same immutable client token for the lifetime of `tail`.

## Regression tests

### A

Three unit tests call `is_auth_rejection`, `ws_request`, and `passes_filter` directly. They check useful local values, including 401/403 versus 503, two separately created authenticated requests, and matching/nonmatching/global events.

They do not execute `tail`, connect, reconnect, or inspect output. Restoring the original header-drain code inside `tail` while leaving the helpers would not fail these tests. Nor would changing the HTTP match in `tail` back to `== 403`. The reported mutations instead break the helpers themselves. Those mutations demonstrate that the assertions run, but do not establish regression protection for the actual loop wiring. The filtering test was not reported as separately mutation-tested.

The extraction of filtering expands the production diff without contributing to the repair. It is behavior-preserving by inspection, but should be described as adapted, not untouched.

### B

`crates/den-cli/tests/tail.rs:74-173` starts the actual built CLI and a local TCP/WebSocket server. It drops the first connection, sends another event on the second, closes that connection, then rejects the third with 401. Assertions inspect credentials on all three requests, both message IDs exactly once and in order, JSON-only stdout, one forwarded resync, stderr gap/reconnect diagnostics, and nonzero termination.

This exercises the incident directly. With only header draining restored, the server still accepts the anonymous request so the test can reach and fail its credential assertion. With only the old 403 guard restored, the post-401 exit deadline fails. B reports executing both independent mutations and restoring the fix. I have not independently verified those execution claims.

**Test robustness issue:** `listener.accept()` at lines 51 and 99 blocks without a deadline. The 20-second watchdog only starts after the third handshake at line 115. A client that exits before connecting, never reconnects, or regresses to stopping on the first close can hang this test indefinitely. Add deadline-aware accepting and check child exit during waits. The per-stream read timeout does not bound listener acceptance. This is a test repair, not a defect in B's production fix.

B's test does not exercise 403, an initial rejection, 503 retry, channel filtering, presence, or ping/pong. It checks close-triggered reconnect, not completion of the close handshake. Its single resync assertion checks count, not the exact reason payload. These limits do not negate its stronger coverage of the reported incident.

## Scope and claims

B's tracked production diff changes only authentication-header reuse and fatal-status matching. Its integration test is untracked and therefore absent from `diff.patch`, but is present in the frozen source and disclosed in `status.txt`. It must be included in any selected patch. A adds three production helpers and colocated unit tests. Neither candidate changes dependencies or server code. Both disclose the existing untracked harness symlink.

Both responses distinguish the default reproduction from unexecuted modes. Neither response proves the complete requirement matrix end to end. In `tools/arena_repro.py:56-64`, `one_connection_on_initial_rejection` and `global_presence_events` are automatically true outside their applicable modes. Therefore the reported default run's "all 9 checks true" is not evidence of initial-rejection or presence coverage. A lists those names as executed checks but later correctly discloses the missing modes. Read that receipt narrowly.

B says "401/403 initial-rejection path is covered by my test's third connection" within its inference section. The test establishes only a later 401, not either initial rejection or 403. The production code makes the broader behavior plausible, but it remains inferred here. Neither candidate's supplied response is an independent execution log.

## Selection rationale

Select B as the repair base, keep its integration test, and bound all test accept waits before adopting it. Use the independent hidden results to close the unexecuted 401/403, 503, filter, global, and ping requirements. A's working production fix is an acceptable alternative, but its helper tests offer weaker protection against recurrence of the actual incident and its production refactoring is unnecessary for this scope.
