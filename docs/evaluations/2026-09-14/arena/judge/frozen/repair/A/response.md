Cause (executed evidence + inference):
- `crates/den-cli/src/stream.rs:24-28` built `auth_headers` once then `extend(auth_headers.drain())`. `drain()` empties the map. Harness pre-fix evidence: `attempts_authenticated=[true,false,false,false]`, `accepted=[1]`, `timeout=true`, only `event 1` on stdout. This explains “no longer delivers new messages” — reconnects were unauthenticated and rejected.
- `stream.rs:78-79` only bailed on `403`. `401` fell through to `Err(_)=>{}` + `Connection lost…sleep…reconnect`, causing infinite reconnect instead of terminating on explicit rejection. Inference from code + post-fix behavior; pre-fix log showed `Reconnecting in 1s/2s/4s/8s` loop.

Implementation (`crates/den-cli/src/stream.rs` only):
- Added `ws_request(url,token)` that builds a fresh `into_client_request()` + `insert(Authorization: Bearer …)` every loop iteration. `tail` now calls it per-connection.
- Added `is_auth_rejection(status)=matches!(401|403)` and used in `Err(Http)` guard to `bail!("Authentication rejected…")`. Transient `503/500` still retries via `Err(_)` + backoff.
- Extracted existing filtering into `passes_filter(event,channel)` unchanged semantics: `None` filter prints all; `Some` prints matching `channel_id` + all channel-less globals (`Resync`, `Presence`, etc.). `Ping→Pong`, `Close/Err→break→reconnect`, `Gap:` + `Connection lost` to stderr, `print(event)` JSON lines to stdout preserved — no invented replay.
- Added `#[cfg(test)] mod tests` with 3 unit tests.

Exact verification (executed):
- `cargo build -p den` → `Finished dev profile` (3.35s initial, 2.7s/4.07s after).
- `/tmp/den-arena-20260914/runtime/bin/python tools/arena_repro.py tools/arena-den` → `passed:true` in 2.55s (2.6s/2.74s repeats), `returncode:1 timed_out:false`, `attempts_authenticated=[true,true,true] accepted=[1,2] requests=[/ws,/ws,/ws]`, all 9 checks true: `exits_on_auth_rejection, one_connection_on_initial_rejection, auth_on_every_connection, expected_events_once(ids 1,2), json_only_stdout, ping_response, gap_diagnostics, global_resync_events(2), global_presence_events`. stdout has `resync,event1,resync,event2`; stderr has `Gap:…` x2 + `Connection lost…1s` x2 + `Error: Authentication rejected…`.
- `cargo test -p den` → `3 passed;0 failed`: `tail_stops_on_401_and_403_but_retries_transient`, `tail_authenticates_every_connection`, `tail_preserves_filtering_and_global_events`.
- `rustfmt --check` → fixed formatting, then `rustfmt` + re-test/build/repro all green.
- `git diff/status` shows only `crates/den-cli/src/stream.rs` modified (+ helpers/tests, -drain, -`==403`); untracked `tools/arena-den` is harness symlink, not edited.

Regression red/green (executed):
- RED1: temporarily `is_auth_rejection=>status==403`, `cargo test -p den tail_stops…` → `FAILED assertion failed:is_auth_rejection(401) 0 passed/1 failed`.
- Restored `matches!(401|403)` → pass.
- RED2: temporarily `ws_request` to omit `Authorization`, `cargo test -p den tail_authenticates…` → `FAILED unwrap on None 0/1`.
- Restored insert → `cargo test -p den` `3/3 ok`. Demonstrates tests guard both bugs.

Alternatives considered/rejected (inference):
- `extend(auth_headers.clone())` / `iter().cloned()` — preserves map but keeps stale global state; rejected for fresh-per-connection `insert` which is simpler and stateless.
- Bail on all `4xx/5xx` — rejected; would break required `transient 503` retry (repro expects `[2]` after one 503).
- Replay/cache events to hide gap — rejected; violates “honest resync/gap, no invented replay” and `DESIGN.md` “resync-by-refetch, no durable replay”.
- Integration test with live mock WS server — rejected as flaky/heavy under 20-min limit; pure unit tests cover decision points.

Unfinished / limits:
- Did not execute `401/403/transient/filter` exercise modes end-to-end (only default `reconnect` via allowed exact command); those paths verified by inference from `is_auth_rejection` + `passes_filter` unit tests, not live run.
- No new dependency, migration, `den-core`, or repro edits. No commit per scope.
