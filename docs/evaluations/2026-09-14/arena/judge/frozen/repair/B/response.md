Done. Report:

## Cause

Two defects in `crates/den-cli/src/stream.rs`, both in `tail`'s reconnect loop:

1. **Credentials survived only one connection.** The `Authorization` header lived in a `HeaderMap` that was moved into each request with `extend(auth_headers.drain())` (`stream.rs:24-28` before the fix). `drain()` empties the map, so the first connect carried the token and every reconnect after a dropped socket was anonymous. The server then answered 401 forever — the CLI reconnected but never delivered another message.
2. **Only 403 was fatal.** The error arm matched `status == 403`; a 401 fell into `Err(_) => {}` and was retried with backoff forever, so an explicitly rejected token never terminated the command. The two defects compounded: the drained header produced the 401 that the CLI then refused to treat as final.

Executed evidence for the diagnosis: the baseline repro run reported `attempts_authenticated: [true, false, false, false]`, `accepted_connections: [1]`, `timed_out: true`, `returncode: -9`.

## Implementation

`crates/den-cli/src/stream.rs`, 8 lines changed:

- Parse the header value once as a `tungstenite::http::HeaderValue` and `insert` a clone into every handshake request, so each attempt authenticates.
- Treat `matches!(status, 401 | 403)` as fatal: `bail!("Authentication rejected; log in again")`.

Everything else is untouched: backoff and reset, the 50s/10s socket timeouts, ping→pong, close handling, channel filtering with global (channel-less) events always printed, the forwarded `Resync` gap notice, and diagnostics on stderr only. No new dependencies; no files outside `crates/den-cli`.

## Verification (all executed)

| Command | Result |
|---|---|
| `cargo build -p den` | ok |
| `…/python tools/arena_repro.py tools/arena-den` (before fix) | `passed: false` — `exits_on_auth_rejection`, `auth_on_every_connection`, `expected_events_once` false; killed at 8.01s |
| same (after fix) | `passed: true`, all 9 checks true, `attempts_authenticated: [true,true,true]`, `accepted_connections: [1,2]`, ids `…0001`,`…0002`, `returncode: 1`, 2.59s |
| `cargo test -p den` | 1 passed, 0 failed |
| `cargo fmt -p den` | clean (reformatted only the new test file) |

## Regression test: red → green

`crates/den-cli/tests/tail.rs` — one integration test that spawns the built binary against a raw `TcpListener` mock: connection 1 sends a resync + event and is dropped abruptly, connection 2 must re-authenticate and closes cleanly, connection 3 gets a raw `401`. It asserts the bearer token on all three handshakes, non-zero exit within 20s, exactly the two message ids in order, one resync (forwarded, not invented), JSON-only stdout (every line parsed), and `Gap:`/`Connection lost` on stderr.

Each fault was restored separately and the test failed for the right reason, then the fix was restored:

- Header fault restored (`drain()`): `FAILED … assertion left == right failed: reconnect must send the token again / left: None / right: Some("Bearer tail-regression-token")` (2.51s).
- 401 fault restored (`== 403`): `FAILED … tail kept reconnecting after a 401 instead of exiting` (22.83s).
- Fix restored: `test tail_reauthenticates_on_reconnect_and_stops_when_rejected ... ok`.

No existing assertions were weakened — there were no prior tests in `den-cli`.

## Alternatives considered and rejected

- **Bail on any non-101 HTTP response.** Rejected: a 503 during a server restart must be retried, per `docs/DESIGN.md` ("reconnect with backoff") and the repro's `transient` mode. Only 401/403 are treated as final.
- **Synthesize a local `Resync` on reconnect, or silence the gap lines.** Rejected: `den_core::Event::Resync` and DESIGN.md promise no durable replay; a client-invented resync would fake a guarantee. The server's resync is forwarded verbatim and gaps stay loud on stderr.
- **Send the token as a query parameter or subprotocol.** Rejected: leaks credentials into URLs/logs and would need a server change, which is out of scope.
- **A unit test over an extracted "is this status fatal" helper.** Rejected as too weak: it would not have caught the drained header, the actual incident. A binary-level test matches AGENTS.md, which names WebSocket reconnect as behavior that matters.
- **Re-inserting into the `HeaderMap` each iteration.** Equivalent; the single parsed `HeaderValue` is smaller and avoids re-parsing per attempt.

## Unfinished / limits (inference vs. evidence)

- I ran only the repro's default `reconnect` mode — that is what its `__main__` executes, and I was restricted to the exact invocation. The `401`, `403`, `transient`, `filter`, and `channel=True` modes were **not executed**. Inferred, not proven, from the code and my test: the 401/403 initial-rejection path is covered by my test's third connection (raw 401 → exit 1, no further connect attempt); `transient` still retries because only 401/403 are fatal; filtering and global-event handling are byte-for-byte unchanged.
- `tools/arena-den` is the harness-created symlink and shows as untracked; I left it in place. No commits were made.
- File tools are confined to this checkout, so nothing outside it was read; I did not inspect sibling directories, other sessions, or git history. My earlier attempt to run the repro with `$CARGO_TARGET_DIR` expanded was blocked by the permission layer until the symlink was supplied.