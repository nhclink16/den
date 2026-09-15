# Track 5: sound packs

Queued. Read `docs/night/README.md`, `docs/M9-NOTES.md` and `docs/THEMES.md` first: this is deliberately the same shape as the theme system, so it should reuse those patterns rather than invent new ones.

Nicholas's friend Andy wants to do sound design for Den. Today there are exactly two sounds, both synthesised in the browser with an oscillator, and one on-off switch. That is not something a person can design for.

## The model
A **sound pack** is a named set of sounds, one per event, exactly as a theme is a named set of colours.

- The **server** has a default pack, set by an admin. It gives an instance its character.
- A **user** can override it, per event or wholesale, and their choice follows their account to every device.
- A user can also silence any single event without silencing the rest.

Precedence, most specific first: the user's per-event choice, then the user's pack, then the server's pack, then the built-in pack.

## Events
Name them for what happened, not for what plays. Start with exactly these and no more:

`message` (a message in a room you follow), `mention`, `dm`, `call_join` (you joined), `call_leave` (you left), `someone_joined` (another person joined your call), `someone_left`, `screen_share_started`, `terminal_bell`, `upload_complete`, `error`.

The existing join and leave tones map to `call_join` and `call_leave`. Keep the current synthesised sounds as the built-in pack so nothing goes silent on upgrade, but render them once to files rather than keeping the oscillator code.

## Storage
- Pack metadata rides in the user's account like `custom_themes` does: a `SoundPack { id, name, sounds: { [event]: SoundRef } }` where a `SoundRef` is a built-in name, an uploaded id, or `silent`.
- Audio files use a per-account store like the M10 background and the M11 avatars: `PUT /users/me/sounds/{id}`, max 512 KiB each, `audio/ogg`, `audio/mpeg`, `audio/wav`. Reject anything that does not decode, and reject anything longer than 5 seconds; a notification sound that runs longer is a bug, not a choice.
- Server default pack lives on the instance settings, admin-only, same endpoint shape.
- Include both in `den-server export` and `import`.

## Authoring, which is the point
Andy has to be able to work on this without a Den checkout.
- **Export a pack** as a single `.den-sounds.zip`: a `pack.json` plus the audio files. **Import** the same.
- Settings gains a **Sounds** section listing every event as a row: its name, a plain sentence saying when it plays, the current sound, a play button to hear it in place, a replace button, and a silence toggle. Per-event volume is a slider; there is also one master volume.
- A **Test all** button plays the pack in order with a second between each, so a designer can hear the set as a set.
- Say plainly in the section that files are capped at 512 KiB and 5 seconds.

## Rules
- Nothing plays when the tab is focused and you are already looking at that room; the existing notify logic already knows this, reuse it rather than duplicating the check.
- Respect the operating system's reduce-motion equivalent for audio: if `prefers-reduced-motion` is set, still play sounds, but never stack more than one at a time.
- Sounds are never preloaded on page load; fetch on first use and cache.

## Tests
Human-scale. One integration test for pack storage and precedence (user over server over built-in, silencing one event, an unknown id falling back). One for uploads, covering the size and duration limits, a non-audio body, and export and import round-tripping a pack with files.

## Deliverable
Branch `feat/sounds`. PR titled "Sound packs: per-server defaults, per-user overrides". Before and after of Settings, and a short screen capture with audio of Test all playing a pack. Post `[astra-sounds] PR open` to fable.

## Confirmed by Nicholas, 2026-09-15
The eleven events above are right. Two additions, both from him:

**Admins set the server pack.** Already in this brief as the server default. It stands: an admin
changes the instance's sounds, and that is what a new member hears. A user's own choice still wins
over the server's. He asked for "the more room for customization the better", and a forced pack is
the one place where more room for the admin means less for the user, so the user keeps the override.

**Sounds are shareable in chat.** This is the part to get right, because it is how Andy will actually
hand his work around. A pack or a single sound posted to a channel renders as a card, not a file
attachment: the pack's name, how many events it covers, a play button that auditions it in place, and
an **Add to my sounds** button that installs it without a download-then-upload round trip. Posting is
just sending the `.den-sounds.zip` this brief already specifies, so the card is a renderer over an
attachment whose type Den recognises, not a new upload path. A single sound posts the same way and
offers **Use for...** with the event list.

Rules for the card: auditioning never installs anything; installing a pack does not silence events the
pack leaves out, it layers over the current choice; and the card says plainly who posted it. Treat an
imported pack as untrusted input — validate every file decodes and fits the 512 KiB and 5 second caps
before it is written, exactly as the upload endpoint does, because a zip is an easier place to hide a
malformed file than a file picker is.
