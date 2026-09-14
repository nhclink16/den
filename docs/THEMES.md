# Appearance and themes

Den's appearance is account data. Web, desktop, and native clients use the same
JSON from `GET /users/me/appearance`. Replace it with `PUT /users/me/appearance`.
Cookie writes require the normal CSRF token and Origin; bearer writes use the
normal token authentication. An `appearance_updated` event carries `user_id` and
`appearance` only to that user's sessions. Reconnects refetch the preference.

## Shared schema

Rust definitions live in `crates/den-core/src/appearance.rs`. OpenAPI and the web
TypeScript declarations derive from those definitions. Built-in data lives in
`crates/den-core/src/themes.json` and is consumed by both the server and web build.

```json
{
  "mode": "system",
  "light_theme": "den-light",
  "dark_theme": "den",
  "custom_themes": []
}
```

`mode` is `light`, `dark`, or `system`. System mode retains one selection for each
appearance and resolves it against the current device's preference. Both selected
cards show a check. A theme has this shape:

```json
{
  "id": "sea-glass",
  "name": "Sea glass",
  "appearance": "dark",
  "colors": {
    "bg": "#0f1518", "bg2": "#151d21", "bg3": "#1c272c",
    "line": "#27353c", "ink": "#dfe8ec", "ink2": "#98a9b1",
    "ink3": "#63737b", "accent": "#78dcca",
    "success": "#7fd39a", "danger": "#e26d5c"
  },
  "fonts": { "display": "Sora", "body": "Inter", "mono": "IBM Plex Mono" },
  "radius": "soft",
  "density": "comfortable"
}
```

Colors require `#RRGGBB`. IDs, names, and font families require 1–40 Unicode letters,
numbers, spaces, hyphens, or underscores, with at least one nonspace character.
Unknown fields, unknown enum values, duplicate IDs, built-in ID replacement, missing
themes, and light/dark mismatches are rejected. At most 12 custom themes may be
stored, and their compact UTF-8 JSON array must be at most 16 KiB. File imports also
have a 16 KiB limit before parsing. Importing an existing ID gives the imported copy
a new UUID; every other field survives unchanged. Deleting a selected custom theme
restores `den-light` or `den` for that appearance.

## Color roles and derived values

`bg` is the room, `bg2` panels, `bg3` raised controls and composers, `line` separators,
`ink` primary text, `ink2` secondary text, and `ink3` subdued metadata. `success` and
`danger` carry their named states. The accent is for what is live: mentions, unread
activity, connected state, focus and progress. Theme selection itself stays neutral.

Compute these values when applying a theme, never store them in authored JSON:

- `accent_dim`: mix 55% accent and 45% bg in sRGB. For each normalized RGB channel,
  `dim = 0.55 * accent + 0.45 * bg`. Web uses `color-mix(in srgb, accent 55%, bg)`.
- `accent_glow`: accent RGB with exactly 0.18 alpha, independent of its background.
- `selection`: `accent_dim`.
- Mention background: `accent_glow`.

The web maps colors to `--bg`, `--bg2`, etc. and keeps the old `--bg-2`, `--ink-2`,
`--lamp`, `--moss`, and `--ember` names as aliases. Overlay, scrim, hover, shadow,
and grid colors derive from these roles. `color-scheme` matches the active theme.
One `applyTheme` function handles runtime changes and the synchronous inline
first-paint script generated into `index.html`. The inline script reads the
`den.appearance` localStorage cache before the application modules run. Cache
failure falls back to the defaults. The server remains authoritative after login.

| Setting | Values |
| --- | --- |
| Sharp radius | 2px / 6px |
| Soft radius | 6px / 12px |
| Round radius | 10px / 18px |
| Comfortable density | 1.0 |
| Compact density | 0.8 |

Density scales message spacing, sidebar row padding, and composer padding.
The editor previews colors, fonts, radius and density live. `Save as new theme`
keeps the changes in the account; Reset or leaving Appearance discards the preview.
An export includes the current preview. Custom cards have keyboard-accessible delete
buttons, also visible on touch screens.

Only the active theme's font families are requested through one Google Fonts CSS
link. IBM Plex Mono is bundled locally and is the mono fallback. The curated list
contains all built-in families plus Inter, Instrument Sans, Space Grotesk, Nunito,
Lora, Fraunces and Commit Mono. Options declare their own family without preloading
inactive fonts. A free-text family uses the same loading path. A failed request or
three-second timeout switches to fallback and shows `Couldn't load that font`.

## Built-in themes

| id | name | appearance | bg | bg2 | bg3 | line | ink | ink2 | ink3 | accent | success | danger | display / body / mono |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| den | Den | dark | #1b1916 | #232019 | #2c2821 | #3a3429 | #ece5d8 | #a89f8f | #6f6759 | #e8a44a | #8da874 | #d2623e | Zilla Slab / Atkinson Hyperlegible / IBM Plex Mono |
| den-light | Den Light | light | #f4efe6 | #ebe5da | #e2dbcd | #d3cabb | #26221c | #5e564a | #8a8173 | #c77d1f | #5f7f4a | #b8462a | Zilla Slab / Atkinson Hyperlegible / IBM Plex Mono |
| moss | Moss | dark | #151a15 | #1b221b | #232c23 | #2f3a2f | #e4ead9 | #a3ad98 | #6c7566 | #9ccf6f | #7fbf8a | #d9694f | Gabarito / Atkinson Hyperlegible / JetBrains Mono |
| tide | Tide | dark | #0f1518 | #151d21 | #1c272c | #27353c | #dfe8ec | #98a9b1 | #63737b | #5fd3c6 | #7fd39a | #e26d5c | Sora / Inter / IBM Plex Mono |
| ember | Ember | dark | #191313 | #211919 | #2b2020 | #3a2b2b | #f0e4e0 | #b09c97 | #7a6864 | #f4845f | #8fc48a | #ff4d6d | Bricolage Grotesque / Atkinson Hyperlegible / Fira Code |
| iris | Iris | dark | #14121b | #1a1724 | #231f2f | #302a40 | #ebe6f5 | #a89fbd | #726985 | #b48cff | #8fd39a | #ff6b8a | Manrope / Manrope / Fira Code |
| paper | Paper | light | #fafaf7 | #f2f1ec | #e9e8e1 | #dad9d1 | #1f1f1d | #5c5c57 | #8b8b84 | #2f6fed | #2f8f5b | #d6453d | Source Serif 4 / Source Sans 3 / Source Code Pro |
| slate | Slate | dark | #111214 | #17181b | #1f2024 | #2a2c31 | #e6e7ea | #9a9ca3 | #64666d | #8b93ff | #6fcf97 | #f26b6b | Geist / Geist / Geist Mono |
| terminal | Terminal | dark | #000000 | #0a0a0a | #141414 | #262626 | #d0d0d0 | #8a8a8a | #555555 | #33ff66 | #5fd38d | #ff5555 | JetBrains Mono / JetBrains Mono / JetBrains Mono |

All use soft radius and comfortable density, except Terminal uses sharp radius and
Slate uses compact density. `scripts/m9-contrast.mjs` runs before every web build and
requires 4.5:1 for ink/bg and ink2/bg2 in every built-in palette. Custom colors are
user-authored and are not silently changed to meet a contrast threshold.

## Native clients

Native apps, including iOS, read the same JSON and map each role to their own
components. Use the same IDs, light/dark resolution, sRGB mixing, radii and density
scale. Native font loading may use locally installed or bundled family equivalents;
the server schema remains family names. Do not create a second palette format.
