Both suspicious changes reproduce against the checked-in tests. Findings:

## Finding 1 — CSRF/Origin check is now OR instead of AND (cookie-auth writes)

- **Operation / location:** every cookie-authenticated non-GET request, e.g. `POST /tokens`. `crates/den-server/src/auth.rs:135` — `if origin != Some(&s.origin) && hash(csrf) != row.csrf_hash` (was `||`).
- **Reproduction (executed):** `cargo test -p den-server --test api`, test `auth::auth_consumes_invites_protects_cookies_and_revokes_credentials`. Identities: admin bootstraps an invite; `alice` registers and logs in via `POST /auth/login`, yielding a `den_session` cookie plus `csrf_token`. The request is `POST /tokens` with `Cookie: den_session=…`, `Origin: https://evil.example`, `X-CSRF-Token: <alice's csrf_token>`.
- **Expected vs actual:** the test expects 403. Actual: **200** — `crates/den-server/tests/api/auth.rs:69` panics `left: 200, right: 403`. Symmetrically, a request with the correct `Origin` and a blank/garbage `X-CSRF-Token` is now also accepted, so the token check is dead for all browser traffic.
- **Impact:** the two-layer check becomes single-layer, and each layer is separately forgeable by a different attacker position: any non-browser holder of the cookie (malicious native/Tauri-adjacent client, proxy, extension) satisfies the check by simply setting `Origin: <DEN_ORIGIN>`, and any foreign-origin context that has learned the CSRF token now passes. The affected endpoint mints long-lived bearer tokens (`auth.rs:215`+), so success converts a cookie into a persistent API credential. Mitigating factor I verified: the session cookie is `HttpOnly; SameSite=Strict` (`auth.rs` set-cookie, asserted at `tests/api/auth.rs:52-56`), so the textbook cross-site browser POST still fails to carry the cookie at all; the loss here is the layered defense and the explicit cross-origin rejection the project tests for. The `/ws` origin check at `auth.rs:139` is unaffected.
- **Method:** executed.

## Finding 2 — `read_state_updated` is broadcast to every channel member, not just its owner

- **Operation / location:** WebSocket event fan-out. `crates/den-server/src/ws.rs:129` — `Event::ReadStateUpdated { state, .. } => Some(&state.channel_id)`; the `user_id != &a.user.id` guard was removed, so the event is now gated only by channel visibility.
- **Reproduction (executed):** `cargo test -p den-server --test api`, test `m2_live::websocket_targets_notifications_and_read_state_without_dm_leaks`. Identities: `bob` and `alice` (members) plus `admin`; bob and admin each open `/ws`; alice posts to `#general` (`POST /channels/{id}/messages`, content `"hello @bob"`). `inbox::changed` (`crates/den-server/src/inbox.rs:133-176`) emits one `ReadStateUpdated` per channel member per write.
- **Expected vs actual:** bob's socket should see only his own read state (the test's own assertion at `tests/api/m2_live.rs:103` states this explicitly). Actual: bob receives alice's and admin's read states too. Observed failure at `tests/api/m2_live.rs:48`: `left: 0, right: 1` — the `until(…, ReadStateUpdated)` loop now terminates on another user's read-state event that arrives before bob's own `Notification`, so bob's mention notification is never counted.
- **Impact:** two effects. (a) Disclosure: every channel member learns every other member's `last_read_id`, `unread_count`, `mention_count`, `notification_count` in real time — including inside DMs, for both DM participants. (b) Correctness: the web client applies the payload unconditionally to its own map — `apps/web/src/lib/store.svelte.ts:293` `case 'read_state_updated': this.setRead(ev.state)` — keyed only by `channel_id`, so another user's counts overwrite yours; badges show wrong unread counts and `markRead`'s early return (`store.svelte.ts:88`) can skip marking a channel read. Part (b) is inferred from reading the client; part (a) and the server-side event delivery are executed.
- **Method:** server-side leak executed (test failure above); client corruption inferred from source.

## No finding

- `crates/den-server/src/ws.rs:22` `sort()` → `sort_unstable()`: the vector is built from `HashMap` keys, so all elements are distinct and stability is unobservable. Harmless.
- `crates/den-server/src/auth.rs:110-111` cookie parsing, `find_map(|p| p.trim().strip_prefix(…))` → `.map(str::trim).find_map(|p| p.strip_prefix(…))`: each segment is still trimmed before the prefix test; identical behavior. Harmless.

## Checks run

1. `cargo test -p den-server --test api` → **35 passed, 2 failed**: `auth::auth_consumes_invites_protects_cookies_and_revokes_credentials` (auth.rs:69, 200 vs 403) and `m2_live::websocket_targets_notifications_and_read_state_without_dm_leaks` (m2_live.rs:48, 0 vs 1). No other suite regressions.
2. Source reads: `auth.rs:81-160`, `ws.rs:1-140`, `inbox.rs:40-176`, `den-core/src/lib.rs:170-195`, `apps/web/src/lib/store.svelte.ts:83-93,287-299`.

Remaining uncertainty: I did not run the web test suite or a browser, so the unread-badge corruption in Finding 2(b) is source-level inference. For Finding 1 I did not build a realistic end-to-end attacker scenario that obtains the CSRF token cross-origin; the executed evidence is that the server now accepts a request the project's own test requires it to reject, and that a correct-`Origin` request with a wrong CSRF token is accepted by inspection of the same condition. I made no changes to any file.