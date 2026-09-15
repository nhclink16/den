# Desktop portrait call acceptance

2026-09-15. Tested the packaged Electron AppImage built from `43531a9`, the merged
PR #7, on codexbox. No application layout change was needed for Auto.

## Findings

**Auto passes a live landscape → portrait → landscape transition.** One remote
screen share and two camera videos remained connected and continued decoding.
The native client window changed from 1900 × 1100 to 1100 × 1900 and back through
X11 window-manager commands. Playwright viewport emulation was not used.

| Case | Available call area | Result |
| --- | --- | --- |
| Auto, landscape | 1400 × 917 | Full-width share, two 696-pixel-wide cameras below; 99% of available height used |
| Auto, portrait | 600 × 1717 | Share and both cameras each 600 pixels wide, stacked; group occupies 65% of height and is centered |
| Auto, back to landscape | 1400 × 917 | Original tile rectangles restored exactly |
| Custom, landscape | 1400 × 917 | A six-column share and two three-column cameras reproduce the half-width group |
| Custom, portrait and back | 600 × 1717, then 1400 × 917 | Custom positions preserved, including the unused space |
| Select Auto after Custom | 1400 × 917 | Original full-width Auto layout restored |

Portrait leaves about 299 pixels above and below the group. With both sidebars
visible, the central column is only 600 pixels wide. Three wide video sources
cannot fill every pixel of a tall column without stretching or cropping. The
share stays contained and readable, the cameras stack, and all controls remain
visible. There is no overlap or content confined to the top third.

### Andy's landscape screenshot

The original screenshot shows the **Custom** label beside Auto, Even, and Focus.
`CallGrid.svelte` renders that label only when the effective preset is Custom;
this is also true of the original implementation in `3f2a1ce`. The saved custom
mode is therefore established by the screenshot, rather than inferred from the
empty space alone.

Using the app's keyboard move/resize controls, I recreated the same half-width
arrangement: a share occupying six of twelve columns with two three-column
cameras beneath it. Auto uses the entire available width with the same participants.
This identifies a saved arrangement, not a general landscape width failure. I did
not retrieve Andy's localStorage, so the exact saved cell values are a reproduction.

**PR #7 does not make Andy's existing Custom arrangement automatically stack.**
It preserves manual positions. He must select Auto to get responsive stacking.
Custom still leaves space in portrait; the passing portrait result applies to Auto.

## Evidence

All screenshots show the same private room and light theme. The video is a silent
capture of the real X11 desktop and includes the Electron window frame.

- [Landscape](shots/pr/desktop-portrait/landscape.png)
- [Portrait](shots/pr/desktop-portrait/portrait.png)
- [Return to landscape](shots/pr/desktop-portrait/landscape-return.png)
- [Silent native window rotation](shots/pr/desktop-portrait/rotation.mp4)
- [Half-width Custom reproduction](shots/pr/desktop-portrait/custom-half-width.png)
- [Custom in portrait](shots/pr/desktop-portrait/custom-portrait.png)
- [Custom returned to landscape](shots/pr/desktop-portrait/custom-landscape-return.png)
- [Auto restored](shots/pr/desktop-portrait/auto-restored.png)
- [Measured rectangles and decoded frames](shots/pr/desktop-portrait/verification.json)

Across the first round trip, the share's decoded frame count increased from 44 to
134; the cameras increased from 62 to 182 and 59 to 178. Both peer connections
remained connected. The automated run passed without renderer errors.

## Reproduction and coverage

The [desktop verification checklist](../apps/desktop/README.md#vertical-monitor-checklist)
now includes both orientation transitions, media continuity, saved Custom layouts,
and an attended check for physical monitors with different display scaling.
[The smoke script](../scripts/electron-portrait-smoke.mjs) asserts these results
against actual rendered videos and native window sizes.

Acceptance used a dedicated 2400 × 2160 Xvfb display with XFWM on Linux. The
camera and display inputs are Chromium-generated moving test patterns. They are
published through the real Den/LiveKit call path, not injected into the DOM as
placeholder video elements. This verifies layout with live media and native
resizing; it is not a physical camera, physical monitor rotation, mixed-DPI,
Windows, or macOS acceptance pass. Windows was not woken for this task.

The existing isolated Den servers and local LiveKit were reused. No public call,
shared server on port 7000, deployment, or release was changed. Both test
participants left the call after the smoke.

No actionable layout defect was found in Auto for this case. Approve that tested
case; the physical monitor and other-platform checks remain unverified.
