# M7c brief: call layouts

Owner: Astra. Read `docs/IDEAS.md` (the two non-negotiables: every stream visible at once, and your call, your layout), `docs/M3-NOTES.md`, `docs/M7A-NOTES.md`, `docs/M7B-NOTES.md`, and the call grid, strip, and tile code before writing anything. Visual language unchanged.

Done when: in the expanded call view, two people share their screens and both shares are visible large at the same time by default, Nicholas drags a tile somewhere else and resizes another with a corner handle, pins a cam next to a share, pops a tile out into its own floating window, the arrangement survives leaving and rejoining the hangout, and a preset button puts it all back. The strip mode is unchanged.

## Model
- A layout is per room, stored locally: `{ preset, tiles: { [tileKey]: { col, row, w, h, pinned } } }` on a 12-column grid with square-ish rows, where `tileKey` is `<participant id>:<kind>` and kind is `cam`, `screen`, `canvas:<object id>`, or `terminal:<object id>`. Unknown tiles auto-place; tiles that leave free their cells. Persist in `localStorage` per room id; a `Reset layout` action clears it.
- **Never a single-stream switcher.** The default placement gives every screen share an equal large slot, then canvases and terminals, then cams in a row beneath. Two shares means two large tiles side by side. Three means three. The layout engine must not have a "focused stream" concept that hides others.
- Presets, in a small segmented control top-left of the expanded view, mono 11px labels: `Auto` (the default placement above, recomputed as people join and leave), `Even` (all tiles equal), `Focus` (pinned tiles large, everything else in a strip along the bottom). Manual moves switch the preset to `Custom` automatically. Presets are starting points; a manual layout wins until Reset.

## Interactions
- **Drag** a tile by its name pill or any empty area of the tile; while dragging, show a 12-column ghost grid at 6% white and a drop target outline in `var(--lamp)`. Other tiles shift to make room (use a simple packing pass, not a physics library). Drop snaps to cells.
- **Resize** with a 14px corner handle bottom-right that appears on hover, snapping to cells, min 2×2 cells, aspect free. Screen shares and canvases keep 16:9 content with letterboxing; cams use `object-fit: cover`.
- **Pin** via a small pin icon top-left on hover; pinned tiles get a 2px `var(--lamp-dim)` ring at rest and are what `Focus` enlarges. Multiple pins allowed.
- **Pop out** via an icon next to pin: use the Document Picture-in-Picture API where available (Chromium) so the tile becomes an always-on-top floating window that keeps playing when Den is behind a game; fall back to a plain `window.open` with the tile in it. The popped tile leaves a placeholder in the grid with `popped out · bring back`.
- **Keyboard**: with a tile focused, arrow keys move it one cell, Shift plus arrows resize, `P` pins, `O` pops out, `R` resets the layout, `Escape` deselects. Tiles are focusable with a visible focus ring.
- **Mobile** (under 900px): presets only, no drag or resize, pins still work and `Focus` is the default there. Pop-out hidden.
- **Transitions**: 140ms ease on tile moves, disabled under `prefers-reduced-motion`.

## Copy
Segmented control labels as above. Hover tooltips `Pin`, `Pop out`, `Resize`. Reset action in the palette and the call toolbar's overflow: `Reset layout`. Placeholder text for a popped tile: `popped out · bring back`.

## Verification
Extend the M3 smoke or add `scripts/m7c-smoke.mjs`: three fake-media contexts, two of them share screens, assert both share tiles are large and neither is hidden; drag one tile to a new cell and assert its stored position; resize another and assert size; pin a cam and switch to Focus and assert it grew; leave and rejoin and assert the layout is restored; Reset and assert the default. Screenshots of Auto with two shares, Custom after a drag, Focus, and a popped-out placeholder at 1440×900 and the mobile Focus view at 390×844 to `docs/shots/m7c-*.png`. Look at them. Release with `deploy/release.sh`, rerun the smoke against `https://denchat.app`, write `docs/M7C-NOTES.md`, stop.
