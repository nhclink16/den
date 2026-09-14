# Appearance and themes

Appearance is account data, shared by web and native clients through
`GET/PUT /users/me/appearance`. Cookie writes require CSRF and Origin; bearer
writes use token authentication. `appearance_updated` reaches only the owning
user's sessions. Reconnects refetch the preference.

## Shared schema

Rust types live in `crates/den-core/src/appearance.rs`. OpenAPI and TypeScript
are generated from them. `crates/den-core/src/themes.json` is the single source
for built-in families. A family contains both appearances:

```json
{"mode":"system","theme":"den","custom_themes":[]}
```

Mode is `light`, `dark`, or `system`; System follows the device. The selected
family stays the same as Mode changes. A theme has this shape:

```json
{
  "id": "sea-glass",
  "name": "Sea glass",
  "light": {
    "bg": "#f1f6f7",
    "bg2": "#e6eef0",
    "bg3": "#dae5e8",
    "line": "#c6d4d8",
    "ink": "#15252b",
    "ink2": "#4d6068",
    "ink3": "#7a8b92",
    "accent": "#1e8f85",
    "success": "#2f8f5b",
    "danger": "#d0533f"
  },
  "dark": {
    "bg": "#0f1518",
    "bg2": "#151d21",
    "bg3": "#1c272c",
    "line": "#27353c",
    "ink": "#dfe8ec",
    "ink2": "#98a9b1",
    "ink3": "#63737b",
    "accent": "#5fd3c6",
    "success": "#7fd39a",
    "danger": "#e26d5c"
  },
  "fonts": {
    "display": "Sora",
    "body": "Inter",
    "mono": "IBM Plex Mono"
  },
  "radius": "soft",
  "density": "comfortable"
}
```

Each half may additionally contain `generated: true`. This marks a derived half
for the editor hint. Editing a color clears that half's generated marker. Built-in
palettes are authored and never generated.

IDs, names, and font families require 1–40 Unicode letters, numbers, spaces,
hyphens or underscores, with at least one nonspace character. Colors require
`#RRGGBB`. Unknown fields, invalid enums, duplicate IDs, replacement of built-ins
and missing selected families are rejected. The custom-theme array is limited
to 12 families and 16 KiB of compact UTF-8 JSON. Import files also have a 16 KiB
limit. An imported ID collision receives a new UUID. Deleting the selected custom
family selects Den.

## Migration and generated halves

Migration `0008_theme_pairs.sql` replaces the previous light/dark selections with
`theme`, preferring `dark_theme`, then `light_theme`, then `den`. The old built-in
`den-light` maps to `den`. Existing custom palettes keep their colors, IDs, names,
fonts, radius and density. The missing half starts as null in SQL, then the Rust
startup companion fills it in one transaction before accepting requests. Nulls
are a restart-safe marker; retrying after a crash completes only missing halves.
No authored colors are replaced. The old first-paint cache is migrated in memory
before painting, and native caches remain scoped to the server origin.

The editor's Copy from dark/light action and imports containing one half use the
same derivation as the server migration. Old M9 `.den-theme.json` exports with
`appearance` and `colors` are accepted by the importer.

The conversion uses [Ottosson's OKLab matrices](https://bottosson.github.io/posts/oklab/).
For bg/ink, bg2/ink2 and bg3/ink3, exchange the OKLCH lightness values while
retaining each role's hue and chroma. Place line 0.12 lightness units toward the
opposite appearance from the new bg, keeping line's hue and chroma. For accent,
success and danger, step lightness toward the opposite extreme by 0.001 until
the quantized color passes 4.5:1 against the new bg. Conversion to six-digit sRGB
clips out-of-gamut RGB channels and rounds to bytes. If no chromatic candidate
passes, use the black or white endpoint that does. Generated colors are a starting
point for custom themes, never a substitute for authored built-ins.

Migrating a very large legacy custom array may expand it beyond 16 KiB. Migration
preserves it for reading; subsequent saves still enforce the limit. Remove or
export some custom families before saving if the account reaches that limit.

## Runtime and editor

The root applies the selected family's active half through one `applyTheme`
function, also embedded synchronously before application modules. Browser cache
uses `den.appearance`; native cache uses `den.appearance:<origin>`. The server is
authoritative after login. Native pending saves retain their original server.

The grid has eight split previews. Light occupies the top-left and Dark the
bottom-right, separated by a thin line. Explicit Light or Dark mode dims the other
half to 70%; System leaves both undimmed. Only the selected family is checked.
Names use each family's display font. Custom cards keep their custom label
and keyboard/touch-accessible delete control.

The editor's Light/Dark control selects which half to edit. Changes affect the
screen when that half is active. Changing Mode retains the unsaved draft and
previews its other half. Save as new theme saves both halves; Reset or leaving
Appearance discards the draft. Export includes the current draft.

## Color roles and derived values

bg is the room, bg2 panels, bg3 raised controls, line separators, ink primary
text, ink2 secondary text, and ink3 subdued metadata. Accent marks live activity,
mentions, unread state, focus and progress. Success and danger retain their roles.

Derived values are not stored in palette JSON:

- accent_dim is 55% accent plus 45% bg in sRGB.
- accent_glow is accent at 0.18 alpha.
- selection uses accent_dim; mention background uses accent_glow.

CSS variables use `--bg`, `--bg2`, etc., with the existing `--bg-2`, `--ink-2`,
`--lamp`, `--moss` and `--ember` aliases. Overlay, scrim, hover, shadow and grid
colors derive from the same roles. color-scheme follows the resolved half.

Sharp radius is 2px/6px, Soft 6px/12px, Round 10px/18px. Comfortable density is
1.0 and Compact 0.8; density scales messages, sidebar rows and composer padding.
Active font families load through the existing Google Fonts link. Appearance also
loads the gallery display fonts while mounted. IBM Plex Mono is bundled offline.
Font loading has a three-second fallback and the existing failure message.

## Authored families

Nicholas confirmed eight families and sixteen palettes for M9b. Fonts, radius
and density are unchanged from M9.

| Family | Half | bg | bg2 | bg3 | line | ink | ink2 | ink3 | accent | success | danger |
|---|---|---|---|---|---|---|---|---|---|---|---|
| den | light | #f4efe6 | #ebe5da | #e2dbcd | #d3cabb | #26221c | #5e564a | #8a8173 | #c77d1f | #5f7f4a | #b8462a |
| den | dark | #1b1916 | #232019 | #2c2821 | #3a3429 | #ece5d8 | #a89f8f | #6f6759 | #e8a44a | #8da874 | #d2623e |
| moss | light | #f3f6ef | #e9eee3 | #dfe6d6 | #cdd6c3 | #1f261d | #55604f | #7f8a78 | #4f8a2a | #3f8a4a | #c2552f |
| moss | dark | #151a15 | #1b221b | #232c23 | #2f3a2f | #e4ead9 | #a3ad98 | #6c7566 | #9ccf6f | #7fbf8a | #d9694f |
| tide | light | #f1f6f7 | #e6eef0 | #dae5e8 | #c6d4d8 | #15252b | #4d6068 | #7a8b92 | #1e8f85 | #2f8f5b | #d0533f |
| tide | dark | #0f1518 | #151d21 | #1c272c | #27353c | #dfe8ec | #98a9b1 | #63737b | #5fd3c6 | #7fd39a | #e26d5c |
| ember | light | #faf3f1 | #f3e9e6 | #ebdedb | #dcc9c4 | #2a1c19 | #6a544f | #957f79 | #d9552c | #4f8f4a | #c8284a |
| ember | dark | #191313 | #211919 | #2b2020 | #3a2b2b | #f0e4e0 | #b09c97 | #7a6864 | #f4845f | #8fc48a | #ff4d6d |
| iris | light | #f6f4fb | #eeeaf7 | #e4dff1 | #d2cbe4 | #1f1a2e | #5b5470 | #857d9a | #6d4de0 | #3f8f5a | #d0405f |
| iris | dark | #14121b | #1a1724 | #231f2f | #302a40 | #ebe6f5 | #a89fbd | #726985 | #b48cff | #8fd39a | #ff6b8a |
| paper | light | #fafaf7 | #f2f1ec | #e9e8e1 | #dad9d1 | #1f1f1d | #5c5c57 | #8b8b84 | #2f6fed | #2f8f5b | #d6453d |
| paper | dark | #1a1a19 | #222221 | #2b2b29 | #393937 | #eeeeea | #a6a6a0 | #6f6f69 | #6f95ff | #5fbf85 | #f0655c |
| slate | light | #f6f7f9 | #eef0f3 | #e4e7ec | #d3d7de | #16171b | #565a63 | #868a93 | #4f57e6 | #2e9a62 | #d84545 |
| slate | dark | #111214 | #17181b | #1f2024 | #2a2c31 | #e6e7ea | #9a9ca3 | #64666d | #8b93ff | #6fcf97 | #f26b6b |
| terminal | light | #ffffff | #f4f4f4 | #e8e8e8 | #cfcfcf | #111111 | #555555 | #8a8a8a | #0a8f2f | #1f8a4c | #d11a1a |
| terminal | dark | #000000 | #0a0a0a | #141414 | #262626 | #d0d0d0 | #8a8a8a | #555555 | #33ff66 | #5fd38d | #ff5555 |

| Family | Display / body / mono | Radius | Density |
|---|---|---|---|
| Den | Zilla Slab / Atkinson Hyperlegible / IBM Plex Mono | soft | comfortable |
| Moss | Gabarito / Atkinson Hyperlegible / JetBrains Mono | soft | comfortable |
| Tide | Sora / Inter / IBM Plex Mono | soft | comfortable |
| Ember | Bricolage Grotesque / Atkinson Hyperlegible / Fira Code | soft | comfortable |
| Iris | Manrope / Manrope / Fira Code | soft | comfortable |
| Paper | Source Serif 4 / Source Sans 3 / Source Code Pro | soft | comfortable |
| Slate | Geist / Geist / Geist Mono | soft | compact |
| Terminal | JetBrains Mono / JetBrains Mono / JetBrains Mono | sharp | comfortable |

`scripts/m9-contrast.mjs` runs before web builds and checks ink/bg and ink2/bg2
at 4.5:1 for both halves of every built-in family. User-authored colors retain
M9's validation policy and are not silently modified to meet a contrast target.

## Native clients

Consume the same family JSON, Mode resolution, token derivations, radii and
density scale. Font families may map to locally installed or bundled equivalents.
Do not add a second palette format. Clients built against the retired M9 schema
need an updated appearance implementation before using this server contract.
