# M9 appearance and themes

Complete. Deployed source `c3b2761` to https://denchat.app on 2026-09-13 Eastern
and passed the public M9 smoke. The acceptance screenshots below were captured
from the public site and inspected. Stopped after recording these results.

## Shipped

Appearance is the first Settings section and the default Settings page. It offers
all nine exact built-in palettes, light/dark/system mode, separate light and dark
selections, live mini previews, font controls, radius, density, and the ten-role
color editor. Custom themes save to the account, can be deleted, and export/import
as `.den-theme.json`. Reset or leaving Appearance discards an unsaved preview.

The shared schema and built-in JSON are in den-core. Migration
`0007_appearance.sql` stores preferences per user. `GET/PUT /users/me/appearance`
validate theme fields, IDs, matching light/dark selections, the 12-theme limit,
and 16 KiB total. `appearance_updated` goes only to the owning user's sessions.
Generated OpenAPI and TypeScript types include the new contract. [Themes](THEMES.md)
documents the schema, exact palettes, derivation math, and native client mapping.

One function applies the root variables, derived accent values, radii, density,
fonts, color-scheme, and page background. Vite embeds that same function before app
boot to apply the localStorage cache. The early background is set even while the
stylesheet and app modules are unavailable. Only active font families are requested
through one Google Fonts link; bundled IBM Plex Mono remains available offline.
Unknown or slow families fall back within three seconds with the requested error.

The color audit removed authored hex/RGB/HSL colors from app.css and components.
Call overlays use theme roles; pop-out windows mirror root variables and font links.
Mounted tldraw canvases follow light/dark appearance. Ghostty 0.4 lacks runtime VT
palette replacement, so the renderer maps its initial default RGB values to the
active theme without changing parser state or remounting the session. An explicit
program color identical to an initial default also follows this mapping. Other
program colors retain their values.

## Verification

- `cargo test --workspace`: passed, including all 23 server API tests and both
  host integration tests. The appearance behavior test covers authentication,
  cookie CSRF denial, stored preferences, malformed values, theme limits,
  matching selections, and WebSocket account isolation.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings`:
  passed. All Cargo commands used `CARGO_TARGET_DIR=/mnt/storage/den-m9-target`.
- `npm --prefix apps/web run check`: zero errors and warnings. Production build
  passed. Vite retains its existing large-chunk advisory for lazy plugin bundles.
- `scripts/m9-contrast.mjs`: all 18 required pairs pass 4.5:1. Primary-text ratios
  range from 13.62 to 15.79; secondary-text ratios from 5.73 to 7.03. The script runs
  before every web build and fails on a deficient pair.
- `scripts/m9-smoke.mjs`: passed locally and on https://denchat.app with Chromium.
  It checks nine cards, instant Tide accent, cached reload, system light/dark,
  both System selections, live font/radius/density edits, a saved custom accent
  in an independent login context and after reload, JSON export/import equality
  except collision-reassigned IDs, invalid import rejection, live session sync,
  a nonexistent font's fallback, and desktop/phone layout.
- The first-paint check holds application modules, samples at 50 ms after navigation,
  requires the app mount to still be empty, and checks Tide's actual root background
  and accent. CDP captures that early frame without Playwright waiting for fonts.
  The intentionally empty Tide-colored screenshot proves the cached paint occurs
  before hydration.
- `scripts/m9-popout-smoke.mjs`: passed actual window opening, initial variables,
  live theme/color-scheme changes, and return of the original mounted content.
- `scripts/m9-embedded-smoke.mjs`: passed mounted Ghostty and tldraw appearance
  changes, including actual terminal canvas pixel checks. Their local screenshots
  were inspected. This uses the isolated local M9 server and Vite on port 17901.

Browser acceptance used Chromium, including phone viewport emulation. Native apps
and screen-reader testing were not part of this milestone.

## Public acceptance and screenshots

The existing VPS backup completed before deployment. `deploy/release.sh` built and
deployed the server, CLI and SPA; public `/health` returned `ok: true`. The public
smoke used the existing `m6_bob` smoke account, restored its original appearance,
and deleted its temporary self-DM message. No credentials were written to the repo.

```sh
DEN_SMOKE_URL=https://denchat.app \
DEN_SMOKE_USER=m6_bob \
DEN_SMOKE_CREDENTIALS="$HOME/.local/share/den-m6/smoke-credentials.json" \
node scripts/m9-smoke.mjs
```

| Public screenshot | Size |
| --- | --- |
| [Appearance](shots/m9-appearance-desktop.png) | 1440 x 900 |
| [Live color editor](shots/m9-editor-desktop.png) | 1440 x 900 |
| [Appearance on phone](shots/m9-appearance-mobile.png) | 390 x 844 |
| [Font controls on phone](shots/m9-appearance-mobile-controls.png) | 390 x 844 |
| [Den chat](shots/m9-chat-den.png) | 1440 x 900 |
| [Paper chat](shots/m9-chat-paper.png) | 1440 x 900 |
| [Tide chat](shots/m9-chat-tide.png) | 1440 x 900 |
| [Terminal chat](shots/m9-chat-terminal.png) | 1440 x 900 |
| [Cached first paint](shots/m9-first-paint-50ms.png) | 1440 x 900 |

## Lane coordination

Implementation used the `m9-themes` branch in an isolated worktree, with pulls
before commits and explicit-path staging. Main was fast-forwarded after taking
in astra-m8's call commits through `552533d`. The CSS-only CallView conflict kept
the new call layout. The overlapping call files, Palette, Composer, and pop-out
integration are named in commit `3f18bca`. The other lane's work and design
artifacts were not staged into M9 commits.

## M9b

Eight families now have sixteen authored palettes, following Nicholas's correction
of the brief's nine/eighteen count. All supplied colors and each family's M9
fonts, radius and density are preserved. One selected family follows Light, Dark
or System mode. The gallery shows both halves diagonally, dims the inactive half
to 70% in explicit modes, and loads each caption's display font.

The color editor edits either half, keeps the draft when Mode changes, and can
derive a missing half with Copy from dark/light. Generated halves carry a hint;
editing a color clears that marker. Legacy one-palette exports import as paired
families. Custom limits remain twelve families and 16 KiB.

Shared types, generated TypeScript and `docs/THEMES.md` describe the paired schema.
Migration 0008 selects the old dark family first, then light, then Den; `den-light`
becomes `den`. SQL preserves authored custom colors and marks the missing half
null. A transactional Rust startup companion generates those halves before HTTP
starts and safely retries after interruption. The migration test covers old dark
and light custom themes, selection fallbacks and an identical second completion.

Derivation swaps OKLCH lightness for the background/ink pairs, places line 0.12
lightness units from the new background, and adjusts semantic colors to 4.5:1.
Rust and browser code use the same OKLab matrices and quantized contrast checks.
Out-of-gamut RGB is clipped. Authored custom colors retain M9's validation policy.
Legacy arrays that expand beyond 16 KiB stay readable, but subsequent writes must
fit the limit. The public preflight found two appearance rows and no custom themes.

Local verification passed:

- `cargo test -p den-core -p den-server`, including all 25 server API tests, color
  conversion, appearance authentication/CSRF/private events and migration recovery.
- `cargo fmt --all -- --check` and core/server Clippy with warnings denied.
- Web type checking with zero errors/warnings and production builds. The contrast
  script checks all sixteen palettes: ink/bg ranges from 13.62 to 18.88, ink2/bg2
  from 5.60 to 7.03. Vite retains its existing large-plugin-chunk advisory.
- Updated M9 smoke: one Tide selection follows both halves; legacy first-paint
  cache converts before hydration at 50 ms; explicit modes dim the opposite card
  half; editor changes and generated hints work; imports, independent-session
  persistence, private live sync, fonts, density and mobile layout pass.
- Mounted Ghostty and tldraw follow Paper light and Terminal dark; terminal canvas
  pixels match. Pop-out variables and color-scheme update live, and returning the
  tile preserves its mounted content. Local screenshots were inspected.

Cargo used `/mnt/storage/den-m9-target`. Work was committed in `m9b-theme-pairs`,
then merged and pushed to main through `c85f4c3`. The desktop lane received advance
notice of the shared-type/migration merge. Its per-origin cache and captured-origin
save behavior remain intact; its native-session files and design artifacts were
not included in M9b commits.

Public deployment and smoke are pending the desktop lane's v0.2.1 signed update
acceptance. v0.2.0 bundled the retired appearance schema, so the desktop lane asked
that its update be published and checked before the server rollout.
