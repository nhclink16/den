# Spotify Jam acceptance ledger

This is the review and release record for the Spotify Jam lane. It covers the
server, web/Tauri client, portability, deployment inputs, and compatibility with
clients that predate Jam events. Production deployment is not part of this work.

## Acceptance inventory

| Surface | Acceptance condition | Evidence |
| --- | --- | --- |
| OAuth start/callback | Authenticated, CSRF-protected writes; random single-use state bound to the initiating Den user; exact loopback or production redirect; only the two read-only playback scopes | `an_authorize_state_is_single_use_and_bound_to_the_user_who_started_it`, `oauth_rejects_missing_scopes` |
| Provider boundary | No real account or secret in tests; Basic and Bearer headers are correct; provider errors are opaque; connect and total request time are bounded | Loopback Spotify stub in `tests/api/spotify.rs`; `oauth_exchange_refresh_and_now_playing_use_the_stub_provider`, `oauth_bounds_a_slow_provider_and_keeps_secrets_opaque` |
| Stored credentials | Refresh token encrypted with ChaCha20-Poly1305, bound to the Den user as associated data; access tokens memory-only; data-key file is regular and private | Three crypto/key unit tests; SQLite ciphertext assertion in the OAuth integration test |
| Refresh lifecycle | Access tokens refresh before expiry; rotated refresh material replaces the old ciphertext without extending the original six-month authorization; expired/revoked grants become `reauthorize` | OAuth, expiry, and revoked-account integration tests |
| Rate limits and outages | `429 Retry-After` suppresses repeated playback calls; transient failures do not revoke the grant; no provider call can hang a Den request | Rate-limit and timeout integration tests |
| Multi-user access | Spotify account events go only to their owner; Jam reads/writes require room visibility; DMs do not leak to outsiders; only the current host or an admin can end a Jam | OAuth socket assertions, private-DM test, Jam lifecycle test |
| Concurrent Jam state | One live Jam per room; replacement retires the prior Jam; duplicate joins are idempotent; reconnect refetches every visible room | Partial unique index, Jam lifecycle integration test, deterministic browser reconnect gap |
| Older clients/servers | Jam and Spotify events require `?jam=true`; a new client treats an absent Spotify endpoint as unavailable instead of failing its whole resync | WebSocket gate assertions; store fallback in `resync()` |
| Web and narrow UI | Composer suggestion, room card, voice-room form, account settings, callback route, inline confirmation, external-link naming, keyboard focus, reduced-motion-safe status, 320 px reflow | `scripts/run-spotify-jam-smoke.sh`; [desktop](shots/pr/feat/spotify-jam/desktop.png), [320 px](shots/pr/feat/spotify-jam/mobile-320.png), [settings](shots/pr/feat/spotify-jam/settings-mobile.png) |
| Logout/reconnect cleanup | Account and Jam state cannot cross a logout; a Jam started during a forced socket gap appears after reconnect | Browser smoke `logoutIsolation` and `reconnectGap` assertions |
| Portability | Jams and joins survive export/import; default import drops Spotify credentials; disaster-recovery import can retain the encrypted row but still requires the separately restored data key | Extended portability roundtrip test; `deploy/README.md` |
| Contracts and migration | Migration follows current main's thread migration; OpenAPI is generated from the real server; web and Swift snapshots consume shared Rust types | `0021_spotify.sql`; actual-binary schema regeneration with 104 paths and 136 schemas; Swift schema check/build |

## Confirmed defects fixed before review

- Rebasing the preserved work exposed a migration-number collision and thread-era
  router/store/composer conflicts. The Spotify migration moved after the thread
  migration, and Jam state now participates in the current resync/logout model.
- Provider requests had no deadline. A shared client now has connect and total
  timeouts, with deterministic slow-provider coverage.
- A reconnect only refetched rooms already present in the Jam map, so a Jam
  created during a socket gap could remain invisible. Resync now checks every
  visible room.
- Missing OAuth scopes, expired or revoked grants, rotated refresh tokens, and
  `429 Retry-After` were not covered end to end. Each now has a provider-stub test.
- The authorization request asked for broader Spotify playback-state access even
  though Den only calls the currently-playing endpoint. It now requests only
  `user-read-currently-playing`, the scope Spotify documents for that endpoint.
- Playback backoff truncated Spotify's requested delay to 60 seconds. It now
  preserves the full `Retry-After` up to the connection's remaining lifetime.
- A token refresh already in flight could finish after Disconnect and restore an
  access token and playback sample. Account generations now make older provider
  work ineligible, and Disconnect clears on both sides of the refresh lock.
- An OAuth callback already exchanging its code could reconnect after Disconnect.
  Authorization attempts now carry the account generation, and Disconnect
  cancels both pending and in-flight attempts before they can store a grant.
- A callback could pass its final generation check, then write after a newer
  authorization returned. Authorization changes now serialize with callback and
  refresh writes, so older account work cannot land after the new attempt returns.
- Refresh-token rotation initially extended the grant. Spotify documents that
  access-token refresh does not extend the six-month lifetime, so rotation now
  replaces only the encrypted token material.
- A failed database write could discard a rotated refresh token while caching its
  paired access token. Den now refuses that access token unless the replacement
  refresh credential is durable first.
- The Spotify data key accepted exposed or non-regular files. Startup now rejects
  both cases and creates a new Unix key file with mode `0600`.
- Spotify and Jam state survived client logout. Both are cleared synchronously
  with the rest of the account epoch.
- An in-flight Jam read from the prior account could land after logout when the
  next account reused the same room id. Jam reads now bind to the account epoch.
- An in-flight Spotify account read could likewise repopulate the next account
  or overwrite a newer socket update. Account reads now bind to both the account
  epoch and the latest accepted Spotify update.
- Delayed Jam write responses could overwrite a newer socket event, cross logout,
  or clear the same room in another Electron instance. Every Jam mutation now
  stays with its concrete Store, account generation, and per-room sequence.
- Pinning a Jam cleared text typed while the request was pending. The pin now
  uses the composer's existing draft owner, lifetime, conversation, and revision
  checks before clearing only the submitted draft.
- The initial card used perpetual pulse motion, ambiguous external-link labels,
  and a voice form that offered no keyboard path to validation. Those controls
  now use a static status, descriptive link names, and focused inline errors.
- Deployment and recovery documentation omitted the optional Spotify secret,
  external key file, callback allowlist, and backup relationship. The compose
  example and operator notes now cover them.
- Local Vite proxying was fixed to port 7000, which macOS Control Center may own.
  `DEN_API_TARGET` now permits an isolated test server while preserving the
  default.

All findings above existed only on the unmerged Spotify feature branch and were
fixed there. No shipped-defect issue was filed for them.

## Verification record

Green local checks:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`, including 99 server API tests
- Five Spotify/Jam unit tests
- Fifteen deterministic Spotify/Jam API integration tests
- Portable export/import roundtrip
- `npm run check --prefix apps/web` (one inherited unused-selector warning)
- Eighty-eight web state/behavior tests
- `npm run build --prefix apps/web`, including all authored contrast palettes
- Three Electron security tests and a signed macOS package build
- Actual-server OpenAPI regeneration for web and Swift
- Swift client schema consistency check and package build
- `scripts/run-spotify-jam-smoke.sh`

Every new automated test was mutation-proven before the mutation was removed:

- One temporary crypto/URL/key-permission mutation made all four original unit
  tests fail; the backoff test separately failed against the 60-second cap.
- Temporary authorization, refresh, timeout, scope, ownership, privacy, rate-limit,
  disconnect/callback races, and event-gate regressions made the original thirteen
  Spotify/Jam integrations fail.
- The least-privilege assertion failed against the broader scope; allowing a new
  authorization to return during older account work failed its lifecycle test;
  and using a rotated grant after its database write failed the durability test.
- Retaining Spotify credentials in a default import made the portability test fail.
- Dropping Jam socket handling made the browser smoke fail on shared state.
- Allowing a prior account's Jam read to resolve after logout made the account
  isolation regression test fail.
- Allowing a stale Spotify account read to cross logout or overwrite a socket
  update made both account-state regressions fail.
- Removing Jam mutation guards made the stale-socket and cross-account tests
  fail; resolving End through the active Store made the instance test fail.
- Holding the pin response while typing made the browser smoke fail when the
  completion erased the newer draft.

After restoration, the focused suites returned green and the temporary breaks
were absent from the diff. The browser runner writes its machine-readable result
to [verification.json](shots/pr/feat/spotify-jam/verification.json).

## External contract checked

The implementation was compared with Spotify's current primary documentation:

- [Authorization Code flow](https://developer.spotify.com/documentation/web-api/tutorials/code-flow)
- [Redirect URI requirements](https://developer.spotify.com/documentation/web-api/concepts/redirect_uri)
- [Refreshing tokens](https://developer.spotify.com/documentation/web-api/tutorials/refreshing-tokens)
- [Currently playing](https://developer.spotify.com/documentation/web-api/reference/get-the-users-currently-playing-track)
- [Rate limits](https://developer.spotify.com/documentation/web-api/concepts/rate-limits)
- [Development-mode limits](https://developer.spotify.com/documentation/web-api/concepts/quota-modes)

The secure backend owns the client secret, so Authorization Code is appropriate;
the browser never receives that secret or a Spotify token. Development mode now
requires a Premium app owner and permits at most five allowlisted Spotify users,
which matches Den's five-person scope but remains an operator prerequisite.

## Dependencies and remaining release gates

- No live Spotify account was used. The deterministic provider stub exercises the
  documented wire contract without exposing a real credential; the first operator
  setup should still confirm the registered production and loopback callbacks.
- The inherited production dependency audit is tracked separately in
  [#45](https://github.com/nhclink16/den/issues/45) and
  [#46](https://github.com/nhclink16/den/issues/46). Those findings come from
  the unchanged web dependency baseline. The server declares `ring` directly
  for token encryption; that package was already present transitively.
- AV PR #50 and native threads PR #51 are included through `main` commit
  `3a841048`. Spotify was rebased onto that exact commit; the migration sequence
  is unique and contiguous through `0021_spotify.sql`, and the regenerated
  contract contains AV, thread, Jam, and Spotify paths and schemas. Independent
  review and exact-head CI remain the final merge gates.
