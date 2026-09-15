# Conversation reading review follow-up

Addresses Worth fixing **17, 24, 29, 36** from `docs/UI-REVIEW-2026-09-15.md`.

## Changes

- Images reserve a 4:3, at-most-512px thumbnail box before decoding. Portrait and unusually wide pictures fit inside it without cropping; the link still opens the original file. The placeholder uses the raised surface color.
- Image load completion keeps the conversation at the bottom only while the reader is following new messages.
- Compact density uses 1.35 message leading; Comfortable keeps 1.45.
- The inner members component is named `roster`, so wallpaper tint/blur only paints the outer rail once.
- Admin labels, compact-message timestamps, and sent-quality labels are at least 11px with secondary ink.

## Recheck and skipped work

Started at current main `43531a9` after PRs #1–#7; all four findings still reproduced. Item **16** was skipped because PR #7 already centers short expanded grids and handles portrait rows; PR #9 subsequently adds per-orientation layouts. Fix now items shipped in PR #1 and are out of scope. Navigation is PR #10; appearance/settings changes are a separate PR. No call-layout geometry or server/core API change here.

## Verification

```sh
DEN_SMOKE_PASSWORD=… node scripts/ui-conversations-smoke.mjs
npm --prefix apps/web run check
npm --prefix apps/web run build
```

The browser script creates a temporary room and real uploaded PNG on an isolated database/server, delays its image response, checks layout and scroll behavior, and deletes its room. It captures the same room content at 1440×900 and 390×844 using Den dark. BEFORE=1 runs against detached main `43531a9`. Evidence is under `docs/shots/pr/fix/ui-conversations/`, including short silent recordings of the delayed image.

| Check | Before | After |
| --- | --- | --- |
| Loading → decoded image height | 0 → 384px | 384 → 384px |
| Distance from bottom after decode | 137px | 0px |
| Scroll displacement while reading older messages | 0px | 0px |
| Comfortable / Compact line-height | 21.75 / 21.75px | 21.75 / 20.25px |
| Wallpaper layers on members rail | 2 | 1 |
| Compact timestamp font | 10.5px | 11px |

The 390px thumbnail ends at x=370, inside the viewport. Browser assertions, typecheck, and production build pass. The existing unused `.small` Login.svelte warning and large-bundle warning remain.

The delayed request is a controlled browser-network fixture; decoding, rendering, uploads, and scrolling are real. The call-label screenshot uses a disconnected canvas track with fixed 1080p/30 statistics to check typography. No live call quality/statistics or native device QA is claimed.
