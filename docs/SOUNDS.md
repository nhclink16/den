# Sound packs

Open **Settings → Sounds** to audition, replace or silence Den’s eleven sounds.
Master and event volumes run from 0–100%. Preferences belong to your account on
that server and synchronize to other sessions.

Resolution is per event: your event choice → your selected pack → the server
pack → Den. `silent` ends that search. Unknown names and missing files fall
through. **Den · built-in** ignores the server pack. **Use server defaults** clears
your selected pack and event choices. Admins can save a named pack and choose
**Use selected pack for this server**; personal choices still win.

## Author without a checkout

Put this `pack.json` alongside your audio files:

```json
{
  "id": "andy-bells",
  "name": "Andy’s bells",
  "sounds": {
    "message": { "type": "upload", "id": "message.wav" },
    "mention": { "type": "upload", "id": "mention.ogg" },
    "call_join": { "type": "builtin", "name": "call_join" },
    "error": { "type": "silent" }
  }
}
```

ZIP the files directly, without a containing folder, as `Andy-bells.den-sounds.zip`.
IDs/file names use 1–64 ASCII letters, digits, dots, hyphens or underscores,
beginning with a letter or digit. Pack names use 1–60 characters.
Each uploaded reference needs a matching file in the ZIP.

Supported: PCM WAV, MP3 and Ogg Vorbis; mono/stereo, at most 192 kHz,
**512 KiB and five seconds per file**. Ogg Opus and other codecs are rejected.
A ZIP contains up to eleven audio files and one `pack.json` (16 KiB maximum),
with a 6 MiB total cap. Every audio entry, including unused entries, must decode
before anything is stored. Nested paths, duplicates, symlinks, special files and
oversized expansion are rejected.

| Event | When it plays |
| --- | --- |
| `message` | A message arrives in a followed room |
| `mention` | Someone mentions you |
| `dm` | A direct message arrives |
| `call_join` | You connect to a call |
| `call_leave` | You leave or disconnect |
| `someone_joined` | Another person joins your call |
| `someone_left` | Another person leaves your call |
| `screen_share_started` | A screen share starts in your call |
| `terminal_bell` | An open live terminal rings |
| `upload_complete` | Your upload finishes |
| `error` | A request or call action fails |

Built-in sound names use these same keys. The original call tones and matching
notification tones are rendered once by `scripts/render-sounds.py`. No oscillator
runs in the client.

## Share and install

Send a pack or supported sound using the normal channel upload button. Its card
names the poster. **Play** only auditions; **Add to my sounds** saves the pack and
layers its authored events over your existing choices. Omitted events keep their
choices. A single sound’s **Use for…** changes just one event. Private attachments
retain the conversation’s access checks. No download/reupload is needed.

Keep up to twelve named packs. Removing a pack does not remove separate event
overrides. **Export pack** flattens effective choices and includes referenced
files. Volumes remain account preferences, outside the shared pack.

## API and storage

Authoritative types: `den-core/src/sounds.rs`. OpenAPI and TypeScript are generated.

- `GET/PUT /users/me/sounds`: resolved account state / replace preferences.
- `PUT /users/me/sounds/{id}`: raw audio, immutable ID; identical retry succeeds.
- `GET /users/me/sounds/{id}`: authenticated account audio.
- `GET/PUT /settings/sounds`: server pack / admin update, with server-owned files.
- `GET /settings/sounds/files/{id}`: authenticated server audio.
- `PUT /users/me/sounds/import`: raw ZIP, validate then install.
- `GET /users/me/sounds/export`: effective `.den-sounds.zip`.
- `GET /uploads/{id}/sounds`: validated card metadata, without installation.
- `GET /uploads/{id}/sounds/{sound}`: validated preview audio.
- `POST /uploads/{id}/sounds`: install, with `{ "event": null }` for a pack or an
  event key for a single sound.

Cookie writes require CSRF and Origin. WebSocket clients must explicitly connect
to `/ws?sounds=true` to receive `sounds_updated`; omitted or false leaves legacy
Rust streams unchanged. A user ID targets only that account; server changes
broadcast an invalidation to opted-in sockets. Native clients retain the flag
alongside `&ticket=...`. Reconnect refetches.
Files live under `DEN_UPLOADS/sounds/<user-id>/<sound-id>` or `sounds/server/`.
Offline `den-server export` and `import` include the database metadata and files.

Audio is fetched and decode-cached on first playback, with a bounded per-origin
cache. Browser autoplay may require a click/key first. Focused-room suppression
uses the existing notification check. Reduced motion stops the previous sound
before the next starts. **Test all** waits for each sound to finish, then one second.
