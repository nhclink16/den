# Profiles

Profiles are shared account data. Migration `0014_profiles.sql` adds `banner_url`,
`bio`, `accent`, and JSON `status` to users; the existing `avatar_url` is populated
by image uploads. There is no pronouns field.

## Profile patches

`PATCH /users/me/profile` returns the full `User`. Its generated `ProfilePatch`
type accepts any subset of:

- `display_name`: the existing 1–100 UTF-8 byte name rule, with nonblank text.
- `bio`: up to 190 Unicode characters, stored as literal plain text. Clients
  must not render it as Markdown or HTML.
- `accent`: six-digit hex, normalized to lowercase, such as `#aa12bf`.
- `status`: `{emoji,text,expires_at}`. Text has at most 60 Unicode characters.
  Emoji must contain exactly one extended grapheme cluster. This accepts flags,
  skin tones and joined emoji while rejecting two separate emoji. Expiry is an
  optional Unix timestamp in seconds.

Omitted fields stay unchanged. Explicit `null` clears bio, accent or status.
A supplied status replaces the whole status. Expired statuses are absent in
responses and are cleared lazily when the user or user list is read.

Every patch, selected profile-image change, and lazy status clearance emits
`user_updated` containing the full user. All authenticated instance members can
see users through `/users`, so they receive these events. The event has no
channel restriction. Unauthenticated clients cannot open that event stream.
Cookie writes require Origin and CSRF, just like other account writes.

## Profile images

`PUT /users/me/avatar` and `/users/me/banner` accept raw PNG, JPEG, WebP or GIF,
with a matching content type. Avatar bodies are limited to 4 MiB and banner
bodies to 8 MiB. The shared thumbnail decoder enforces 8192 pixels per axis,
16 million total pixels, and a 64 MiB decoder allocation cap. Avatars also must
be square and at most 4096 pixels per side; cropping happens in the client.

Original bytes are kept. Still-image previews are PNG: avatars are 256 square,
and banners fit within 1200 pixels wide and 8192 high while preserving aspect.
GIFs are served byte-for-byte without a derivative, preserving animation timing
and all frames. Upload and delete responses contain the full updated `User`.

`GET /users/{id}/avatar` and `/users/{id}/banner` require authentication and are
readable by any member. The URLs in `User` include a hash of the served content
as `?v=<hash>`, so replacements use a new cache key. Responses have a strong
content-hash ETag, `Cache-Control: private, max-age=86400`, and
`Vary: Authorization, Cookie`. Matching `If-None-Match` returns 304. Unset images
return 404; unauthenticated requests return 401.

`DELETE /users/me/avatar` and `/users/me/banner` remove the original and preview,
clear the corresponding URL, and broadcast the changed user. Repeating deletion
is harmless. Bot owners can upload their bot's avatar through
`PUT /users/{id}/avatar`; other users, including unrelated admins, cannot.

Files live in `DEN_UPLOADS/profiles/<user id>/` as `avatar`, `avatar.png`, `banner`,
and `banner.png`. Offline export/import includes originals and derivatives with
manifest hashes. Archive handling accepts only these named files under valid
user IDs, rejects symlinks, and excludes incomplete `.part` files.
