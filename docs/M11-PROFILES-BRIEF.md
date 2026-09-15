# M11 brief: profiles, server half

Owner: the appearance lane, **after** M10's server half is merged, so migrations do not collide. Fable owns every file under `apps/web/src/ui/` and the theme runtime and is editing them in parallel; do not touch those paths.

Today `User.avatar_url` exists and nothing ever sets it. Every avatar in Den is a coloured initial. Nicholas wants what Discord gives people, minus the paywall: everything here is free for everyone.

## Fields
Extend the user record, migration `0014_profiles.sql`:

```
User {
  ...existing,
  avatar_url: Option<String>,     // already present, start populating it
  banner_url: Option<String>,
  bio: Option<String>,            // <= 190 chars, plain text, no markdown
  accent: Option<String>,         // #rrggbb, tints that person's profile card
  status: Option<Status>,
}
Status { emoji: Option<String>, text: Option<String>, expires_at: Option<i64> }
```

`status.text` is at most 60 characters. `status.emoji` is a single grapheme cluster; reject longer input rather than truncating. `expires_at` supports "clear after an hour"; the server treats an expired status as absent when serving and clears it lazily.

`PATCH /users/me/profile` takes any subset of `display_name`, `bio`, `accent`, `status`. Broadcast `Event::UserUpdated { user }` to everyone who can see that user, so names, avatars and statuses change live everywhere without a refresh. Include the full `User` so clients can replace their cached copy wholesale.

## Images
Two per user, an avatar and a banner, stored like the M10 background:
- `PUT /users/me/avatar` and `PUT /users/me/banner`, raw image body, `image/png`, `image/jpeg`, `image/webp`, `image/gif`. Avatar max 4 MiB, banner max 8 MiB. Reject anything that does not decode.
- Store the original plus a square 256px avatar derivative and a 1200px-wide banner derivative, reusing the M2 thumbnail decoder and its pixel and allocation caps. **Animated GIFs keep their animation**: do not flatten them, serve the original and skip the derivative.
- `GET /users/{id}/avatar` and `/banner` are readable by any authenticated member, since these appear next to messages. Strong ETag from the content hash, `Cache-Control: private, max-age=86400`. 404 when unset.
- `DELETE` on both.
- `avatar_url` and `banner_url` on `User` become those paths with a content-hash query so a change busts caches immediately.
- Include both files in `den-server export` and `import`.

Bots get avatars too, set by their owner: `PUT /users/{id}/avatar` succeeds when the caller owns that bot.

## Cropping
The client sends an already-cropped square for avatars, so the server does not crop. It must still verify dimensions are sane, at most 4096 on a side.

## Tests
Human-scale. One integration test for the profile patch, covering length limits, a rejected multi-character emoji, an invalid accent hex, and the event reaching the right audience. One for images, covering upload, GIF animation preserved, oversize rejection, non-image body, another user reading an avatar, an outsider to the instance getting 401, delete, and export and import round-tripping.

## Finish
Regenerate OpenAPI and `schema.d.ts`. Do not deploy; Fable releases when the client is ready. Post `[astra-appearance] profiles server half merged` to fable.
