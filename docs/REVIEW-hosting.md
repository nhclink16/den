# Hosting review

1. Keep the MIT client. AGPL for `den-server` and `den-host` is reasonable, but stop calling it fork protection. Competitors can sell hosting; modified networked versions must offer Corresponding Source to their users, not submit changes upstream. Binary releases also need matching source. [AGPL sections 6, 13](https://opensource.org/license/agpl-3.0).

   MIT deliberately permits closed UI forks. I accept that tradeoff for independent clients and plugins. Existing MIT copies remain usable under their granted terms; audit ownership before changing future licensing. [MIT](https://opensource.org/license/mit). An MIT SDK alone cannot exempt plugins combined with AGPL code. Nor is every bundled dependency MIT: [tldraw 3.15.6](https://github.com/tldraw/tldraw/blob/v3.15.6/LICENSE.md) imposes watermark and redistribution restrictions. Putting the client under AGPL requires resolving that compatibility first. Specify `AGPL-3.0-only` or `-or-later`; update the MIT commitment in [IDEAS](IDEAS.md) deliberately.

2. One process and SQLite directory per tenant is sensible. The proposed preparatory configuration work mostly already exists: [main.rs](../crates/den-server/src/main.rs) accepts bind address, database, uploads, bootstrap path, origin, and LiveKit credentials. Future provisioning must parameterize the hardcoded deployment scripts and isolate service users, disk quotas, CPU, and memory. Instance names do not provide isolation.

   Reject the shared-LiveKit assumption. My reading of [v1.9.0 authorization](https://github.com/livekit/livekit/blob/v1.9.0/pkg/service/auth.go) and [room lookup](https://github.com/livekit/livekit/blob/v1.9.0/pkg/service/roomservice.go) is that API keys authenticate signers without partitioning rooms. [calls.rs](../crates/den-server/src/calls.rs) uses bare channel IDs and verifies webhooks with one key. Restored clones duplicate those IDs. Before cohosting, require deployment-specific room namespaces and verified webhook routing; namespaces alone cannot contain a compromised signing key. Start with separate LiveKit instances, or an enforcing broker later. Defer cloud/Stripe. Retain M6's proven Caddy/HAProxy/TURN path; wildcard certificates alone do not replace it.

3. Export is worth doing, but this ZIP is an operator backup, not ordinary account portability. It exposes every DM, password hash, token hash, host credential hash, grant, and recording. Giving every Den admin a download expands current API access. Start with an offline operator CLI; defer the HTTP endpoint and call its eventual button "Export server backup" in server settings.

   `VACUUM INTO` snapshots SQLite only. [uploads.rs](../crates/den-server/src/uploads.rs) renames partial uploads; [terminal.rs](../crates/den-server/src/terminal.rs) appends and renames recordings. Stop Den for the first exporter. Include partial-upload state and recoverable recordings; exclude deployment secrets. Record archive version, migration checksums, file hashes, and sizes. Import into an unused data directory, validate traversal/symlinks, expansion limits, integrity and foreign keys, migrate supported older schemas, then install atomically. Reject newer or divergent schemas. Migration imports should invalidate sessions, tokens, invites, host credentials and grants, end terminals, and require host re-enrollment; preserve credentials only for explicit disaster recovery. Test both against the real server.

   Add a bounded instance name through `den-core` and a new migration. Defer icons until their upload visibility is independent of private rooms. Keep Minecraft deferred; Docker control needs separate authorization from terminal access.

4. The wordmark popover works for four instances. Show origins for duplicate names, keyboard focus, and a textual mention indicator. Browser Ctrl+1..9 already selects tabs; avoid it. Keep one call with its originating session attached, and make logout disconnect it explicitly.

   The store-map proposal understates the work. [api.ts](../apps/web/src/lib/api.ts) uses same-origin fetch; [auth.rs](../crates/den-server/src/auth.rs) enforces strict cookies and origin checks. Storage prefixes cannot enable arbitrary-server login. First design native credential storage or browser cross-origin authentication. Keep browser instances in separate tabs meanwhile. Defer merged Inbox/search. WebSockets already transmit message bodies; ignoring events saves no bandwidth. Scope plugin state and requests by instance/account, preserve background notifications, and resync after gaps.

5. Build order, engineer-hours including acceptance:

   | Order | Deliverable | Hours |
   | --- | --- | --- |
   | 1 | Linux x86_64 release bundle including SPA; host/CLI for friends' Mac ARM and Windows; checksums, source, installers, real enrollment/PTY checks | 16–24 |
   | 2 | Offline export/import and restore drills, including partial uploads and recordings | 20–32 |
   | 3 | Instance name, generated types, admin UI | 3–5 |
   | 4 | Authorized downloadable export after privacy policy and bounded snapshot design | 6–10 |

   Initial scope: 45–71 hours. Remaining platform combinations: another 16–24 hours; icon: 4–6. M6's two-person media acceptance and M7b's untested native installers remain gates. Reconcile [DESIGN](DESIGN.md)'s no-multi-server decision and stale [ROADMAP](ROADMAP.md) before expanding scope.
