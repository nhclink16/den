# Custom layouts across orientation changes

2026-09-15. Packaged Electron AppImage tested on codexbox with one published
screen share and two active cameras. The first run used PR #9, `1ce8def`; the
current acceptance used PR #11 head `932ad0c`, now merged as `5f189bc`.

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
a471a8cd241189edbd765863dce433e4ea11a6fd4e93cf52409df89c96d524be
```

The smoke compares raw localStorage strings, not reserialized objects or approximate
coordinates. A separate byte-buffer comparison and hashes are recorded in
[storage-integrity.json](shots/pr/window-layout-bucket/storage-integrity.json).
The raw strings, rendered rectangles, decoded frame counts, and live connection
states are in [verification.json](shots/pr/window-layout-bucket/verification.json).
Both peer connections stayed connected and all three videos kept decoding.

## How the choice feels

I would keep a layout per orientation for actual rotations. The first portrait
view becomes readable immediately, and returning restores exactly what I built.
After making a distinct portrait arrangement, both views felt predictable. I
would not expect a six-column landscape arrangement to become a useful portrait
arrangement just by shrinking it. Keeping each version avoids that compromise.

PR #11 removes the surprising trigger from the first run. A panel toggle now
keeps the arrangement I am editing, while a real window rotation selects the
arrangement for that shape. I would keep this behavior.

## Fixed-window sidebar regression passes on PR #11

Both screenshots use the same 1400 × 1100 native window. The container changes
shape, but `matchMedia('(orientation: portrait)').matches` remains false.

| Build | People visible, call area 900 × 917 | People hidden, call area 1120 × 917 |
| --- | --- | --- |
| PR #9 | Wrong portrait Custom, share 900 pixels wide | Switches to landscape Custom, share shrinks to 556 pixels |
| PR #11 | Landscape Custom, share 446 pixels wide | Same Custom cells and saved bytes, share grows to 556 pixels |

The test asserted the original landscape cell coordinates in both states and
compared both raw saved values. It also asserted a growing share width. The
assertion failed against the PR #9 AppImage with `Opening people swapped the
saved landscape Custom arrangement`, then passed against the PR #11 AppImage.

Real native resizing to 1100 × 1900 still selects the portrait bucket. With no
saved portrait arrangement it selects Auto, matching the 65% height baseline.
After editing portrait, rotations restore each separate Custom layout, and the
landscape string remains byte-identical. Reset in portrait removes both keys;
landscape returns to Auto without pressing the Auto button.

Historical evidence from PR #9 remains under
[portrait-layout-memory](shots/pr/portrait-layout-memory/verification.json).
Compare [people visible before](shots/pr/portrait-layout-memory/sidebar-visible.png)
and [people hidden before](shots/pr/portrait-layout-memory/sidebar-hidden.png)
with the current screenshots below.

## Screenshots and recording

Same private room and light theme. Native window sizes are 1900 × 1100 and
1100 × 1900; the sidebar reproduction keeps the window at 1400 × 1100 throughout.

- [Landscape Custom before rotation](shots/pr/window-layout-bucket/custom-half-width.png)
- [First portrait visit, full-width Auto](shots/pr/window-layout-bucket/custom-to-portrait-default.png)
- [Original landscape Custom restored](shots/pr/window-layout-bucket/custom-landscape-return.png)
- [A separately authored portrait Custom](shots/pr/window-layout-bucket/portrait-custom.png)
- [Landscape preserved after editing portrait](shots/pr/window-layout-bucket/separate-landscape-custom.png)
- [Portrait Custom restored](shots/pr/window-layout-bucket/portrait-custom-return.png)
- [Reset in portrait](shots/pr/window-layout-bucket/reset-both.png)
- [Landscape Auto after Reset](shots/pr/window-layout-bucket/auto-restored.png)
- [People visible](shots/pr/window-layout-bucket/sidebar-visible.png)
- [People hidden, same window size](shots/pr/window-layout-bucket/sidebar-hidden.png)
- [Silent Custom rotation recording](shots/pr/window-layout-bucket/custom-rotation.mp4)

## Validation and limits

- Updated `scripts/electron-portrait-smoke.mjs` passes the live Custom flow, raw
  storage comparisons, independent portrait edits, Reset, connected peers, and
  continued video decoding. The sidebar case now asserts unchanged Custom cells
  and saved bytes while the share widens. The script explicitly opens people at
  the start, so a prior failed run cannot reverse the screenshot labels.
- All five existing `scripts/m7c-layout.test.ts` tests pass.
- The [desktop checklist](../apps/desktop/README.md#vertical-monitor-checklist)
  now checks the per-orientation behavior, raw string integrity, Reset, and
  fixed-window panel toggles.
- Actual Electron client-window resizing used X11 commands on a dedicated
  2400 × 2160 Xvfb/XFWM desktop. It did not use Playwright viewport emulation.
- Camera and display inputs were generated moving patterns published through
  Den and LiveKit. No media tiles or call state were mocked in the application.
- Physical monitor moves, mixed-DPI scaling, Windows, and macOS were not tested.
  Windows was not woken. No shared server, deployment, or release was changed.

PR #11 passed all web and desktop CI checks and this live regression before merge.
The Custom round trip, Reset, and fixed-window sidebar behavior are accepted.

## Morning CI follow-up

The PR #9 Linux install hit a transient `onnxruntime-node` CDN timeout and passed
on retry. It is a required dependency of the dictation package, so omitting optional
dependencies does not avoid the download. Raise bounded `npm ci` retries in
`web.yml`, `electron.yml`, and `release.yml` with Nicholas after the parallel lanes
settle. No CI workflow was edited during this QA pass.
