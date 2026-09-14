# M9 brief: appearance and themes

Owner: Astra. Fable authored the palettes below; use them exactly. Read `apps/web/src/app.css` (the token system this replaces), `Settings.svelte`, `store.svelte.ts`, `docs/M8-NOTES.md`, and `AGENTS.md`. Design rule that survives every theme: the accent is for what is live, and only that.

Done when: Settings, Appearance shows nine built-in themes as live mini previews, switching is instant with no flash, light and dark can follow the system, fonts and density change live, a custom theme made in the editor survives a reload on another device because it synced through the server, export and import round-trip a JSON file, and the public site runs it.

## Schema (in `den-core`, shared with every future client)
```
Theme { id, name, appearance: "light" | "dark", colors: ThemeColors, fonts: ThemeFonts, radius: "sharp" | "soft" | "round", density: "compact" | "comfortable" }
ThemeColors { bg, bg2, bg3, line, ink, ink2, ink3, accent, success, danger }   // hex
ThemeFonts  { display, body, mono }                                             // font family names
```
Derived at apply time, never authored: `accent_dim` (accent mixed 45% toward bg), `accent_glow` (accent at 18% alpha), `selection` (accent_dim), mention background (accent_glow). Validate hex and 1–40 char names; reject anything else.

User preference, stored server-side per user and cached in `localStorage` for first paint: `Appearance { mode: "light" | "dark" | "system", light_theme: id, dark_theme: id, custom_themes: Theme[] (max 12, 16 KiB total) }` via `GET/PUT /users/me/appearance`, with an `appearance_updated` event so other open sessions follow. Migration `0007_appearance.sql`.

## Built-in themes (exact values)
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

All built-ins use radius `soft` and density `comfortable` except `terminal` (radius `sharp`) and `slate` (density `compact`). Defaults: mode `system`, light `den-light`, dark `den`. Check every pair of ink on bg and ink2 on bg2 for at least 4.5:1 contrast at build time with a small script and fail the build if one slips.

## Applying a theme
- One function sets CSS custom properties on `:root` from a `Theme`, including the derived values, and sets `color-scheme`. Every color in `app.css` and every component must come from those properties; grep for stray hex values and remove them. Radius maps to `--r`/`--r-lg` (sharp 2/6, soft 6/12, round 10/18). Density maps to a `--density` scale that the message list, sidebar rows, and composer padding read.
- Fonts: the curated list is every family in the table above plus Inter, Instrument Sans, Space Grotesk, Nunito, Lora, Fraunces, Commit Mono. Load only the families the active theme uses, through a single dynamically built Google Fonts link; keep `IBM Plex Mono` bundled locally as the fallback mono. A free-text font field accepts any Google Fonts family name and loads it the same way; if it fails to load in 3 seconds, fall back and show `Couldn't load that font`.
- First paint: read the cached appearance from `localStorage` in an inline script in `index.html` before the app boots so there is no flash of the wrong theme.

## Settings, Appearance (new section at the top of the list)
- **Mode**: segmented `Light · Dark · System`.
- **Theme grid**: cards 180px wide, each a live mini wireframe of Den drawn with that theme's colors (sidebar, a message row with an avatar and an accent mention, a composer), the name underneath in the theme's display font, a check on the selected one. Two rows: light themes and dark themes. In System mode both a light and a dark selection are shown as chosen.
- **Fonts**: three selects (Display, Body, Mono) from the curated list with each option rendered in its own family, plus a `Use any Google Font` text field per role.
- **Radius** and **Density** segmented controls.
- **Customize**: opens an editor panel beside the grid: the ten color roles as rows with a swatch, a native color input, and a hex field; every change applies live to the whole app. Buttons: `Save as new theme` (prompts for a name), `Export` (downloads `<name>.den-theme.json`), `Import` (file picker, validates, adds to custom), `Reset`. Custom themes appear in the grid with a small `custom` eyebrow and a delete on hover.
- **Copy**: section title `Appearance`. Hints: `Follows your device`, `Live preview`, `Themes sync to your account`.

## Other clients
Write `docs/THEMES.md`: the schema, the derived values and how to compute them, the built-in table, and the rule that native apps read the same JSON and map roles to their own components. iOS will use this.

## Verification
`scripts/m9-smoke.mjs`: log in, switch to `tide`, assert `--accent` on `:root` equals its hex, reload and assert it persisted with no default-theme flash (screenshot at 50 ms after navigation), set mode to system and assert the right theme under `prefers-color-scheme: light` and `dark` emulation, create a custom theme by changing `accent`, reload in a second context as the same user and assert the custom theme is present, export then import it and assert the round trip. Screenshots of the Appearance section and of the main chat in `den`, `paper`, `tide`, and `terminal` at 1440×900, and the Appearance section at 390×844, to `docs/shots/m9-*.png`. Look at them; the contrast script must pass. Release, rerun the smoke against `https://denchat.app`, write `docs/M9-NOTES.md`, stop.
