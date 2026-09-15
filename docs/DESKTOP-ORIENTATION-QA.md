# Custom layouts across orientation changes

2026-09-15. Packaged Electron AppImage built from merged PR #9, `1ce8def`, tested
with one published screen share and two active cameras on codexbox.

## The requested round trip passes

| Check | Observed result |
| --- | --- |
| Landscape Custom → first portrait visit | Portrait starts in Auto, all tiles 600 pixels wide, 65% of available height used |
| Return to landscape | Original tile rectangles and the exact stored layout string return |
| Edit Custom in portrait, rotate both ways | Each orientation keeps its own arrangement; neither saved string changes |
| Reset while in portrait | Both keys disappear; the next landscape view is Auto |

The [PR #8 baseline](DESKTOP-PORTRAIT-QA.md) squeezed the landscape Custom share
into 296 pixels and used 38% of the portrait height. PR #9's first portrait visit
now matches the independent Auto result exactly: 600 pixels and 65%. The group
is centered and the controls remain visible.

The landscape value is **356 UTF-8 bytes**. Its SHA-256 before rotating, after
returning, and after authoring a separate portrait Custom layout is identical:

```
b628ccac3f7efa51ad471ecc31774930b3517d957cb615bccf9b385022ac79c9
```

The smoke compares raw localStorage strings, not reserialized objects or approximate
coordinates. A separate byte-buffer comparison and hashes are recorded in
[storage-integrity.json](shots/pr/portrait-layout-memory/storage-integrity.json).
The raw strings, rendered rectangles, decoded frame counts, and live connection
states are in [verification.json](shots/pr/portrait-layout-memory/verification.json).
Both peer connections stayed connected and all three videos kept decoding.

## How the choice feels

I would keep a layout per orientation for actual rotations. The first portrait
view becomes readable immediately, and returning restores exactly what I built.
After making a distinct portrait arrangement, both views felt predictable. I
would not expect a six-column landscape arrangement to become a useful portrait
arrangement just by shrinking it. Keeping each version avoids that compromise.

The **trigger** has a surprising edge case, though. It uses the call area's aspect
ratio, which changes when a sidebar opens or closes even if the window does not move.

### Hiding people switches layouts and makes the share smaller

| Severity | Location | Before | After | Why |
| --- | --- | --- | --- | --- |
| MEDIUM | `apps/web/src/ui/CallGrid.svelte:15` and its layout-opening effect | Fixed 1400 × 1100 window, people visible: 900 × 917 call area, portrait Custom, 900-pixel share | Hide people: 1120 × 917 call area, landscape Custom, 556-pixel share | A panel toggle should not replace the arrangement the user is editing; gaining space unexpectedly makes the share smaller |

This happened in the live call, and both stored strings remained intact. It is a
selection issue, not storage corruption. Closing people changes which saved value
is loaded. Both views say Custom, so the label does not explain the switch.

My recommendation is to select the stored orientation from the **window's** shape
and keep container-based sizing for the responsive preset grid. That separates
"which arrangement did I choose for this window shape?" from "how much space is
available right now?" The application change is left to the layout owner; this
QA PR records the reproduction and updates the smoke/checklist.

## Screenshots and recording

Same private room and light theme. Native window sizes are 1900 × 1100 and
1100 × 1900; the sidebar reproduction keeps the window at 1400 × 1100 throughout.

- [Landscape Custom before rotation](shots/pr/portrait-layout-memory/custom-half-width.png)
- [First portrait visit, full-width Auto](shots/pr/portrait-layout-memory/custom-to-portrait-default.png)
- [Original landscape Custom restored](shots/pr/portrait-layout-memory/custom-landscape-return.png)
- [A separately authored portrait Custom](shots/pr/portrait-layout-memory/portrait-custom.png)
- [Landscape preserved after editing portrait](shots/pr/portrait-layout-memory/separate-landscape-custom.png)
- [Portrait Custom restored](shots/pr/portrait-layout-memory/portrait-custom-return.png)
- [Reset in portrait](shots/pr/portrait-layout-memory/reset-both.png)
- [Landscape Auto after Reset](shots/pr/portrait-layout-memory/auto-restored.png)
- [People visible](shots/pr/portrait-layout-memory/sidebar-visible.png)
- [People hidden, same window size](shots/pr/portrait-layout-memory/sidebar-hidden.png)
- [Silent Custom rotation recording](shots/pr/portrait-layout-memory/custom-rotation.mp4)

## Validation and limits

- Updated `scripts/electron-portrait-smoke.mjs` passes the live Custom flow, raw
  storage comparisons, independent portrait edits, Reset, connected peers, and
  continued video decoding. It also captures the sidebar selection issue.
- All five existing `scripts/m7c-layout.test.ts` tests pass.
- The [desktop checklist](../apps/desktop/README.md#vertical-monitor-checklist)
  now checks the per-orientation behavior, raw string integrity, and Reset.
- Actual Electron client-window resizing used X11 commands on a dedicated
  2400 × 2160 Xvfb/XFWM desktop. It did not use Playwright viewport emulation.
- Camera and display inputs were generated moving patterns published through
  Den and LiveKit. No media tiles or call state were mocked in the application.
- Physical monitor moves, mixed-DPI scaling, Windows, and macOS were not tested.
  Windows was not woken. No shared server, deployment, or release was changed.

Approve the requested Custom round trip and storage behavior. The medium-severity
sidebar selection issue remains for the layout owner.

## Morning CI follow-up

The PR #9 Linux install hit a transient `onnxruntime-node` CDN timeout and passed
on retry. It is a required dependency of the dictation package, so omitting optional
dependencies does not avoid the download. Raise bounded `npm ci` retries in
`web.yml`, `electron.yml`, and `release.yml` with Nicholas after the parallel lanes
settle. No CI workflow was edited during this QA pass.
