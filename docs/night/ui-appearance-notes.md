# Appearance and settings review follow-up

Addresses Worth fixing **21, 23, 26–28, 30–33** from `docs/UI-REVIEW-2026-09-15.md`.

## Changes

- Settings share 26px titles, one section-heading treatment, centered content widths, and 42px form controls.
- Interactive field/button/reaction edges use a derived `--line-strong`; ordinary panel dividers keep their existing color. All 16 built-in palette halves reach at least 3:1 against their three surface colors. An extreme custom palette can have incompatible surfaces; the fallback chooses the black/white edge with the best minimum contrast.
- Corners now reach composer controls, reactions, menus, call controls, labels, and the floating toolbar. Status dots remain circles.
- Theme miniatures scale with their card and use each theme's display, body, and monospace fonts. Selected badges use their own palette's background for the glyph.
- Mobile scheme choices fit one row. Color editor rows explain which surface each color paints.

## Recheck and skipped work

Started from `43531a9` after PRs #1–#7, then integrated newer main. Item **16** is stale: PR #7 centers short call canvases and supports portrait rows; PR #9 adds orientation-specific layouts. Preserve those behaviors. For **26**, outer call tiles already used theme radii; only remaining hardcoded control/menu corners changed. PR #1's Fix now items are out of scope. Navigation findings are in PR #10; reading/image findings have a separate PR.

## Verification

Isolated server `127.0.0.1:7013`, own database/uploads, Vite `localhost:5183`; the shared development instance was untouched.

```sh
DEN_SMOKE_PASSWORD=… node scripts/ui-appearance-smoke.mjs
DEN_SMOKE_PASSWORD=… node scripts/ui-corners-smoke.mjs
DEN_SMOKE_PASSWORD=… node scripts/ui-navigation-smoke.mjs
npm --prefix apps/web run check
npm --prefix apps/web run build
```

Before captures use detached main `43531a9`; after captures use this branch. Same Den light theme, desktop 1440×1000 (corners 1440×900), mobile 390×844. Screenshots and measured JSON are in `docs/shots/pr/fix/ui-appearance/`.

- Themes appears at y=434px on mobile instead of below the first screen.
- Appearance/Machines titles both 26px; room-name field/button both 42px.
- Sharp/Round controls measure 2px/10px; expanded toolbar measures 6px/18px.
- Palette edge contrast is at least 3:1 for all built-ins; Terminal dark badge is black on its green accent even while Den light is active.
- Browser assertions, the integrated navigation regression smoke, typecheck, and production build pass. Typecheck retains the pre-existing Login.svelte unused `.small` warning; build retains its large-chunk warning.

Call screenshots use an explicitly disconnected participant fixture to inspect geometry. No live media, device audio, or native Windows/macOS manual QA is claimed. No runtime schema, core type, or dependency change.
