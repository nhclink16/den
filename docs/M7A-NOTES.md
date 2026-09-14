# M7a canvas

Released 2026-09-13 at https://denchat.app with `deploy/release.sh`.
Application release commit: `48b666e`. Public smoke object:
`01M2E5K0GF3WWBAHWZNHVD490X`. The dev smoke and public smoke passed. Screenshots below are from the public
release. All eight were opened and inspected at 1440×900 and 390×844.

## Use it

Type `/canvas raid layout` in `#plans`, or use the palette action
`New canvas in #plans`. A card appears in chat. Click it to draw above the
messages, drag its bottom separator to resize, or expand it. Each object's dock
height is saved locally. The keyboard also resizes the separator with arrow
keys. While in a call, the same object has a live, read-only tile. Click that
tile to focus the editable canvas. Canvas keys do not trigger call shortcuts.

Settings → Plugins exposes the Canvas switch to admins. Turning it off hides
creation commands and blocks new objects with `canvas_disabled`; existing
canvases remain usable. The setting updates other clients through the event
stream. `/settings` returns JSON to API requests and HTML to page navigation.
Both representations use `Vary: Accept` and `Cache-Control: no-store` so the
browser cannot reuse the page HTML as API JSON.

## Server and sync

Migration `0004_objects.sql` adds `objects` and the singleton `settings` row.
Object creation and its empty chat message share one transaction. Message
summaries include objects without document state. Deleting the message cascades
to its object. Access follows the channel; DM outsiders get 404.

`GET /objects/{id}` returns metadata plus a record map under `state`.
`GET /objects/{id}/summary` omits state. `POST /objects/{id}/patch` accepts
`base_version`, `put`, and `remove`; each put replaces a complete record by ID.
Writes merge into the current map under the server write lock and increment
`version`. A stale base version is accepted. Patches are limited to 1 MiB and
stored state to 8 MiB. `PATCH /objects/{id}` renames or sets a completed PNG
thumbnail owned by the caller in that channel.

Every authorized channel viewer receives `object_patched`, including the author.
The canvas coalesces document changes for 150 ms and applies server changes with
`mergeRemoteChanges`. A version gap or socket resync reloads the full state,
retaining queued edits. Document records are `document`, `page`, `shape`,
`binding`, and `asset`; session and instance records never enter patch traffic.
This is record-level last-writer-wins, so simultaneous changes to the same shape
can overwrite one another. It is not a text CRDT or durable offline queue.

Socket `object_open` and `object_close` track each connection. Closing one device
preserves another device's presence. `/presence` includes visible object presence
for cards on initial load; `object_presence` keeps it current thereafter.
Cursors go only to other users with the object open, at most 20 per second per
socket. They use tldraw collaborator cursors, Den names, and the avatar hue
formula. The lowest present user ID exports a 640px-wide PNG ten seconds after
the last change, through the existing chunked upload API. Cards receive the new
thumbnail through the message update event. Completed thumbnail uploads follow
the existing upload retention behavior, including superseded previews.

## Client plugin API

`apps/web/src/plugins/index.ts` exports `registerPlugin` and `plugins`.
A plugin supplies exactly three hooks:

- `objectKinds[kind]` has Svelte `card`, `view`, and optional `tile` components.
  Each receives `object: ObjectSummary` from the generated API types.
- `slashCommands` contains `{ name, hint, run(ctx) }`. The context supplies
  `channelId`, the remaining command `args`, and `post(content)` for chat.
- `paletteActions` contains `{ id, label, hint, run }`.

Getter properties let the canvas expose commands only when a text room or DM
is active and creation is enabled. Core reads the registry in message cards,
the composer, palette, object dock, and call grid. `main.ts` imports the canvas
registration once. Core owns the active object and presence cache in
`lib/objects.svelte.ts`; `store.onEvent` and `store.sendEvent` use the existing
socket. No plugin discovery, server plugin runtime, or separate sync service.

The Svelte view dynamically imports `plugins/canvas/editor.ts` on first open.
That module mounts React with `react-dom/client` and unmounts it on destruction.
React, tldraw, and tldraw CSS stay out of the initial bundle. Tiles mount the same
view read-only and refit content when the pane changes size. The SDK's default
fonts and icon assets still load from tldraw's CDN; document sync and uploaded
thumbnails use Den.

## Agents

```
den canvas create plans "raid layout"
den canvas get OBJECT_ID
den canvas patch OBJECT_ID --file patch.json
den canvas patch OBJECT_ID --file -
den canvas rename OBJECT_ID "north entrance"
```

`get` prints the state map; create prints the full object; patch prints the new
version; rename prints the summary. The CLI resolves room names and DMs just as
`den send` does. Its token determines the author, including bot identities.

[The Den skill](../skills/den/SKILL.md#shared-canvases) contains a complete,
validated JSON example for a rectangle, arrow, and text label. Every shape needs
`id`, `typeName`, `type`, `x`, `y`, `rotation`, `index`, `parentId`, `isLocked`,
`opacity`, `meta`, and complete `props`. IDs start with `shape:`; parent IDs come
from the document's page records. Geo rectangles use `type: geo` and
`props.geo: rectangle`; text uses `props.richText`; arrow labels in 3.15.6 still
use `props.text`, with start/end points relative to the arrow position. Always
read an existing record before changing its fields.

## Bundle and license

Vite production build sizes, decimal KB after gzip:

| Initial asset | Before | After | Growth |
| --- | ---: | ---: | ---: |
| JavaScript | 141.42 | 144.96 | 3.54 |
| CSS | 6.12 | 6.66 | 0.54 |
| Total | 147.54 | 151.62 | 4.08 |

HTML remains 0.48 KB. The lazy editor chunk is 500.17 KB JavaScript and
13.91 KB CSS. Base growth stays below the brief's 5 KB limit.

Pinned dependencies are `tldraw 3.15.6`, `react 18.3.1`, and `react-dom 18.3.1`.
The [3.15.6 license](https://github.com/tldraw/tldraw/blob/v3.15.6/LICENSE.md)
permits bundled use with the watermark. Its full text ships at
`/licenses/tldraw-3.15.6.md`, and README links it. The "Made with tldraw"
watermark and license validation remain intact. Current tldraw releases require
a production license key; do not upgrade this pin on the assumption that their
terms match 3.15.6. No `@tldraw/sync` package or server is used.

`npm audit` reports 28 moderate dependency paths for
[GHSA-cp6q-959q-f8rh](https://github.com/advisories/GHSA-cp6q-959q-f8rh), with no
high or critical findings. The installed Tiptap 2.27.3 already includes the
own-data-property fix. A runtime check of JSON-origin `__proto__` input confirmed
that `mergeAttributes` retains `Object.prototype` and inherits neither `src` nor
`onerror`; the advisory's version range still includes this 2.x backport.

## Verification and rerun

Passed `cargo test --workspace`, all 17 server integration tests, web type checks
with zero errors or warnings, production builds, and the drawing-record example
against tldraw's real store schema. The final Settings cache-header change also
passed its targeted server integration test. Production has all four migrations
successful, `PRAGMA integrity_check` returns `ok`, and foreign-key checks are
empty. A fresh production backup succeeded before release.

`m7a-smoke.mjs` uses separate Chromium contexts as `nicholas` and `m6_bob`. It
checks slash selection, card creation, a rectangle drawn through pointer events,
Bob's visible cursor, a `clanker` CLI text patch in both browsers within two
seconds, a 640px thumbnail within fifteen seconds, an edit made while Nicholas
is offline and recovered on resync, closing presence, a live read-only call tile
with a decoded camera, the palette action, and admin-only settings after page
navigation. It revokes its temporary bot credential on exit and leaves a labeled
`smoke` canvas. These are two browser contexts on codexbox reaching the public
VPS, not a human check from two physical client machines. Screen-reader behavior
has not been manually verified.

```
# Dev: run ~/.local/share/den-dev/run-server.sh and the Vite dev server first.
node scripts/m7a-smoke.mjs

# Public: build and release, then test the installed URL with the release CLI.
./deploy/release.sh
DEN_SMOKE_URL=https://denchat.app \
DEN_SMOKE_CREDENTIALS="$HOME/.local/share/den-m6/smoke-credentials.json" \
DEN_SMOKE_CLI=./target/release/den node scripts/m7a-smoke.mjs
```

Logs from this run are `/tmp/den-m7-tests.log`, `/tmp/den-m7-settings-test.log`,
`/tmp/den-m7-smoke.log`, `/tmp/den-m7-release.log`, and
`/tmp/den-m7-public-smoke.log`. Credentials remain in the existing private files.

| View | Desktop | Mobile |
| --- | --- | --- |
| Card | [1440×900](shots/m7a-card-desktop.png) | [390×844](shots/m7a-card-mobile.png) |
| Docked | [1440×900](shots/m7a-docked-desktop.png) | [390×844](shots/m7a-docked-mobile.png) |
| Expanded | [1440×900](shots/m7a-expanded-desktop.png) | [390×844](shots/m7a-expanded-mobile.png) |
| Call grid | [1440×900](shots/m7a-call-grid-desktop.png) | [390×844](shots/m7a-call-grid-mobile.png) |
