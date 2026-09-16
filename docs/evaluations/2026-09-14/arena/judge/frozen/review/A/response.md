## Finding 1 — WS leaks other users' read state

* **Affected operation / line:** `GET /ws` event filtering in `allowed()` — `crates/den-server/src/ws.rs:129`:
  `Event::ReadStateUpdated { state, .. } => Some(&state.channel_id)`
  replaced `if user_id != &a.user.id { return false; }`.

* **Reproduction (executed via checked-in test):**
  Identities: `admin` (outsider), `alice`, `bob`; `general` text channel.
  Sequence from `crates/den-server/tests/api/m2_live.rs:29-48`:
  1. `bob` opens `socket(&t,&bob.token)`, `admin` opens second socket, both consume `Resync`.
  2. `alice` `POST /channels/{general}/messages {"content":"ordinary"}`.
  3. `bob` collects `until(... matches ReadStateUpdated)`.
  4. `alice` `POST ... {"content":"hello @bob"}`; collect again, count `Notification{user_id==bob, reason==Mention}`.
  `inbox::changed()` (`crates/den-server/src/inbox.rs:135-176`) sends a personalized `ReadStateUpdated{user_id,state}` per channel member, where `state()` (`inbox.rs:70-86`) is per-user (`last_read_id, unread/mention/notification_count`).

* **Expected vs actual / impact:**
  Expected: `bob` only receives `ReadStateUpdated{user_id==bob}` that is `visible()`; `admin` socket only receives `user_id==admin` (asserts at `m2_live.rs:82-84,101-103`).
  Actual: any channel-visible user receives every member's state. Leaks `last_read_id` (how far they read) + `unread/mention/notification_count` (derived from their `read_state` + `notification_preferences`). Functional: web `store.svelte.ts:setRead()` keys by `channel_id`, so foreign state overwrites own badge.
  **Executed:** `cargo test -p den-server --test api websocket_targets_notifications_and_read_state_without_dm_leaks` → `FAILED` at `m2_live.rs:48: left 0 right 1` — first `ReadStateUpdated` is now foreign, so `until()` stops before `bob`'s `Notification` arrives.

## Finding 2 — Cookie CSRF check weakened from AND to OR

* **Affected operation / line:** cookie-session auth for non-`GET/HEAD/OPTIONS` REST, e.g. `POST /tokens`, `POST /auth/ws-ticket` — `crates/den-server/src/auth.rs:135`:
  `if origin != Some(&s.origin) && hash(csrf) != row.csrf_hash`

* **Reproduction (executed via checked-in test):**
  Identities: `alice` member with `Session{token,csrf_token}` from `/auth/register` or `/auth/login`.
  Sequence from `crates/den-server/tests/api/auth.rs:58-78`:
  1. `Cookie: den_session=<alice.token>` `POST /tokens {"name":"cookie"}` with no headers → 403.
  2. Same + `Origin: https://evil.example` + `X-CSRF-Token: <valid csrf>` → should be 403.
  3. Same + `Origin: <server url>` + valid CSRF → 200.
  **Executed:** `cargo test -p den-server --test api auth_consumes_invites_protects_cookies_and_revokes_credentials` → `FAILED` at `auth.rs:69: left 200 right 403` — case 2 now returns `200`, creating the token. Inferred extension of same line: correct `Origin` alone without CSRF now also passes (`false && true == false`).

* **Expected vs actual / impact:**
  Expected: require both `Origin==s.origin` and `csrf==csrf_hash` (`||` fail).
  Actual: either alone suffices. Any request presenting a valid CSRF token from a wrong `Origin`, or a correct `Origin` without token, is authorized as the victim session. Turns two-factor (secret token + bound origin) into single-factor.

## No finding — `sort()` → `sort_unstable()` (`ws.rs:22`)

Same sorted output for `online_user_ids: Vec<String>`; stability only matters for equal elements which are indistinguishable. No auth/ordering impact.

## No finding — cookie split `.map(str::trim)` (`auth.rs:110-112`)

`split(';').map(trim).find_map(strip_prefix("den_session="))` is equivalent to prior `find_map(|p| p.trim().strip_prefix(...))`. Whitespace handling unchanged.

## Checks / uncertainty

* Ran: `cargo test -p den-server --test api auth_consumes...` → FAIL (200 vs 403); `cargo test -p den-server --test api websocket_targets...` → FAIL (0 vs 1). Both fail only after patch, confirming reachable regressions.
* Inspected `ws.rs`, `auth.rs`, `inbox.rs:70-86`, `m2_live.rs`, `auth.rs` test, `desktop.rs:52-70`.
* Remaining: did not run browser end-to-end CSRF (reqwest spoofs `Origin`); did not run full 37-test suite after patch.