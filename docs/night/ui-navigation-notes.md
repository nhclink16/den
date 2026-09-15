# UI review follow-up: navigation

Rechecked against main `43531a9`, after PRs #1–#7. This PR covers “Worth fixing”
items **14, 15, 18, 19, 20, 22, 25, 34, 35, 37**.

- Palette and mobile room list use native modal dialogs. Focus enters the dialog,
  Tab stays inside, Escape dismisses, and closing returns focus to the opener.
  The palette exposes a combobox/listbox with stable selected-option IDs.
- Mobile settings centers the selected link by scrolling only the tab strip.
  Room links, Inbox, Settings, and settings sections expose their current state.
- Avatar presence has an accessible image role; offline people also have a visible
  label. The blocked-audio banner explains why the call is silent.
- “Room list” is consistent across settings, palette, and both toggle tooltips.
  DM search labels describe a conversation, and mobile search gives the query its
  own row.
- Reduced motion limits animation iteration count and disables smooth scrolling.

## Review triage

The rest is split into appearance/settings (21, 23, 26–28, 30–33) and conversation
layout (17, 24, 29, 36), rather than bundled into this change.

**Skipped stale item 16:** PR #7 already centers the expanded call canvas when its
rows do not fill the viewport (`margin-block: auto`) and adds portrait geometry.
Reapplying the old row-height proposal would overwrite the new layout behavior.

**Partially stale item 25:** PR #1 already guards the reconnect animation behind
`prefers-reduced-motion: no-preference`; that dot needs no change. The global reset
still omitted iteration count and scroll behavior, which this PR fixes.

Items 1–13 and the “Nice to have” list are outside this assignment.

## Verification

`scripts/ui-navigation-smoke.mjs` runs against disposable local data and records
paired 1440×900 desktop and 390×844 mobile screenshots plus silent keyboard videos
under `docs/shots/pr/fix/ui-navigation/`. Run with `BEFORE=1` on the base tree.

The before run reproduced escaped focus, missing restoration, an off-screen Account
tab, a drawer that ignored Escape, a 91px search query, and “Search #Just you”. The
after run verifies modal state, active-option IDs, focus containment and restoration,
visible/current settings navigation, no page-level horizontal scroll, a 290px query,
DM wording, presence roles, the audio explanation, and reduced-motion iteration count.
The offline person and blocked-audio state are explicit UI fixtures; no LiveKit call
is needed for those rendering checks.

Svelte/TypeScript checks and the production build pass. Current main has one existing
unused `.small` selector warning in Login; it is unrelated to this scope.

Not verified: physical iOS/Android devices, native desktop keyboard integration,
and an actual screen reader. Browser roles and names are checked with Playwright.
