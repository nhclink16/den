# Track 6: multiplayer music

**Not queued yet.** Three decisions at the bottom are Nicholas's, and the first one changes
almost the whole design. Do not start until he has answered it.

Read `docs/night/README.md` first.

This is two separate features. Nicholas was explicit about that: "i think the yt one and 3 should
be separate things, i like both tbh." They share no code and should not share a branch.

---

# Part A — Den DJ, a YouTube queue

The thing he actually asked for: "a good multiplayer music experience", and his own read on it was
"i assume streaming wont sound as good as a proper music bot." He is right, and the reason is worth
stating because it decides the architecture.

If every client plays its own YouTube embed and the server just broadcasts a timestamp, five people
hear five slightly different copies. They drift, an ad on one person's account desyncs them
completely, and the phone cannot play it in the background at all. That is not a music bot, it is
five people pressing play at once.

So: **the server plays the audio and publishes it into the call as a participant.** That is what a
music bot is, it is why they sound right, and it is the only version that works on iOS.

## Shape
- A `music` module in `den-server`. One queue per voice room.
- `yt-dlp` resolves the URL to an audio stream; `ffmpeg` transcodes to Opus; the result is published
  to the LiveKit room as a bot participant named after the instance.
- Queue rows: `id, room_id, url, title, duration, thumbnail, added_by, position, state`. New migration.
- REST under `/rooms/{id}/music`: `POST /queue`, `DELETE /queue/{id}`, `POST /skip`, `/pause`,
  `/seek`, `PUT /queue/order`. Every mutation emits a WS event so all five UIs move together.
- Anyone can add, anyone can skip, anyone can reorder. Five friends. No DJ role, no vote-to-skip.

## Client
A queue panel in the call dock: now playing with artwork and a progress bar, the rest of the queue
below it, drag to reorder, who added each track shown next to it. Paste a YouTube URL into the
composer in a voice room and it offers to queue it rather than posting a link.

**Per-listener volume comes free.** PR #2 added per-person volume in calls. The DJ is a participant,
so the existing control already covers it — do not build a second volume system. Wire the music
track into the control that shipped in #2.

**Ducking.** When someone talks, drop music by about 12 dB and bring it back after a beat. On by
default, one toggle to turn it off.

## CLI, because agents are the point
`den music add <url>`, `den music skip`, `den music queue`. This is the cheapest possible proof of
the whole "agents are first class" idea — an agent that DJs is a good demo and costs almost nothing
once the REST endpoints exist.

## What will actually break
Not lawyers. Nobody is going to notice a five-person private instance, and Nicholas already asked
the right question — "but its our own thing, how would it get banned". The real failure mode is
`yt-dlp` breaking when YouTube changes something, which happens every few months. So: pin `yt-dlp`,
make it a separate updatable binary rather than a vendored library, and when resolution fails say
"could not load this track" in the queue instead of dropping the whole player. Assume it will break
and make it break politely.

## Tests
Human scale. One integration test for queue ordering and the skip/reorder events. One for a URL
that fails to resolve leaving the rest of the queue intact. Do not test yt-dlp itself.

---

# Part B — the Spotify Jam card

Nicholas: "all my homies have spotify premium i think so remote jams should be fine- we need a
sleek implementation of it if possible."

Sleek means it should not look like a pasted link. A Jam URL dropped in a room becomes a **card
pinned to the room header**, not a message that scrolls away: who started it, a Join button that
deep-links into Spotify, and the Den members who have joined.

**The one genuinely sleek part.** The host optionally connects their Spotify account to Den
(OAuth, scopes `user-read-playback-state` and `user-read-currently-playing`, read only, no playback
control and no writes). With that connected, the card shows the track that is actually playing right
now, with art, updating live. Without it the card is still a good Join button.

**A limitation to state plainly rather than discover later:** a Jam's state cannot be read from the
share link. Only the host's own playback can be read, and only with the host's token. So "who is
listening" is Den counting Join clicks, not Spotify telling us. Do not promise otherwise in the UI.

Store the token encrypted, per user, refreshable, and let a user disconnect in Settings and have it
actually deleted.

---

## Spotify app registered, 2026-09-15

The developer app exists, so Part B is no longer blocked on credentials.

- **Client ID:** `9efa4ca0d79a4be5a934a22c229ad656`. Client IDs are public and travel in the
  authorize URL; this one is safe to commit.
- **Client secret:** never printed, never committed. It lives on the iMac at
  `~/.config/den/spotify.env`, mode 600. **It is not on the VPS yet.** Whoever builds Part B
  has to move it to the server that will actually perform the token exchange, since the iMac
  is not where Den runs.
- **Redirect URIs registered:** `https://denchat.app/spotify/callback` and
  `http://127.0.0.1:5173/spotify/callback`.
- Scopes intended: `user-read-playback-state`, `user-read-currently-playing`. Read only.
  OAuth has not been exercised yet.

### Three things that will bite whoever implements this

**`localhost` is banned by Spotify; only explicit loopback IPs are allowed.** And `localhost`
and `127.0.0.1` are different origins to a browser, while Den checks `Origin` against
`DEN_ORIGIN` and scopes cookies per origin. So local testing must run the server with
`DEN_ORIGIN=http://127.0.0.1:5173` *and* browse to `127.0.0.1:5173`. Mixing the two silently
breaks the callback, the session cookie, or both. A portless loopback URI was attempted,
since our dev ports move around, and the dashboard rejected it as insecure.

**Refresh tokens expire after 180 days.** A host who connects and then does not host a Jam for
six months comes back to a dead token, so the UI has to handle re-authorisation as a normal
state rather than an error, and storing a refresh token is not a one-time setup.

**The app is in development mode: five authenticated users, owner needs Premium.** This is
not a constraint for the design, because only the Jam *host* authenticates so Den can read
what is playing; joiners deep-link into their own Spotify app and consume no slot. It would
only bind if we ever showed every participant's now-playing. No quota extension was requested
and none is needed.

## Answered by Nicholas, 2026-09-15

**Q2, server-side DJ: yes.** The server plays the audio and joins the call as a
participant. Build Part A on that.

**Q3, music lives inside the call: yes.** You have to be in the voice room to hear it.
One audio path, not two.

**Q4, Spotify developer app: yes, but he is registering it tomorrow.** So **Part B is
blocked** until the credentials exist. Do not start it. Part A has no such dependency
and is the whole of this brief's work for now.
