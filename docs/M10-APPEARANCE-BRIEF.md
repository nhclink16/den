# M10 brief: appearance, server half

Owner: a dedicated lane. Fable owns every client file in `apps/web/src/ui/` and the theme runtime, and is editing them in parallel. **Do not touch `apps/web/src/ui/*` or `apps/web/src/lib/theme*.ts`.** You own `den-core`, `den-server`, migrations, OpenAPI, and the regenerated `apps/web/src/lib/schema.d.ts`.

Read `docs/M9-NOTES.md`, `docs/THEMES.md`, `crates/den-core/src/appearance.rs`, and `AGENTS.md` first.

## 1. Decouple light and dark selection
Today `Appearance { mode, theme, custom_themes }` locks both halves to one family. Nicholas wants what T3 Code does: pick the light half from one theme and the dark half from another.

```
Appearance { mode, light_theme: String, dark_theme: String, custom_themes: Vec<Theme>, background: Option<Background> }
```

`Theme` is unchanged; it still carries both halves. Migration `0013_split_theme_choice.sql` sets both `light_theme` and `dark_theme` to the existing `theme` value for every row, defaulting to `den`. Validate that each id resolves to a built-in or one of the user's custom themes; reject otherwise. Update `docs/THEMES.md` including the native-client mapping note, since the iOS app reads this.

## 2. Backgrounds
New optional field on `Appearance`:

```
Background {
  source: BackgroundSource,     // builtin { name } | upload { id }
  blur: u8,                     // 0..=40, pixels
  dim: u8,                      // 0..=80, percent of a scrim over the image
  saturate: u8,                 // 50..=150, percent
  scope: BackgroundScope,       // app | sidebar | chat
  fit: BackgroundFit,           // cover | contain | tile
}
```

Clamp every numeric field server-side rather than rejecting. Built-in names are a fixed list you define as an enum of exactly: `aurora`, `dunes`, `harbor`, `ember-sky`, `slate-mist`, `grain`. The client renders those as CSS gradients from the active theme's own colors, so the server stores only the name and ships no image assets for them.

User images need a per-user store, separate from channel uploads, because a background belongs to an account and not a room:
- `PUT /users/me/background/image` accepts a single image body, `image/png`, `image/jpeg`, `image/webp`, or `image/gif`, max 8 MiB, and replaces whatever was there. Returns `{ id, content_type, size, width, height }`. Reject anything that does not decode as an image; reuse the M2 thumbnail decoder's limits and its pixel and allocation caps.
- `GET /users/me/background/image` returns it to the owning user only, with an ETag and `Cache-Control: private, max-age=3600`. 404 when unset.
- `DELETE /users/me/background/image`.
- Store under `DEN_UPLOADS/backgrounds/<user id>`; one file per user, overwritten. Include it in `den-server export` and `import`.
- Setting `background.source` to an upload whose file is missing must not break appearance loading; treat it as no background and say so in the response.

`appearance_updated` continues to go only to the owning user's sessions and now carries the new shape.

## 3. Interface contrast
Add `contrast: u8` to `Appearance`, 80..=120, default 100. The client multiplies text and border contrast by it. Server only stores and clamps it.

## Tests
Human-scale. One integration test for the split selection, covering migration of an existing row, rejecting an unknown theme id, and the event reaching only the owner. One for backgrounds, covering upload, private fetch by a second user returning 404, oversize rejection, a non-image body, delete, and export and import round-tripping the file. That is enough.

## Finish
Regenerate OpenAPI and `schema.d.ts`, keep the web client compiling by leaving Fable's files alone and only changing generated types, run the workspace suite, then **stop without deploying**. Fable releases once the client half is ready. Post `[astra-appearance] server half merged` to fable.
