# Direction

Decisions from a strategy pass on 2026-09-15, after an outside review of the
integrated tree. `DESIGN.md` describes what Den is; this describes what it is
becoming and, more usefully, **why** — so a decision already made does not get
quietly relitigated or accidentally reversed.

## What Den is competing on

"Open-source Discord" is occupied territory. Stoat reported a million registered
users in March 2026; Fluxer and Daccord both advertise self-hosting, LiveKit
calls and plugins. Self-hosting, voice and a plugin system are table stakes, not
differentiators.

The thing nobody else has is **live shared objects that people and agents both
act on** — the canvas and terminal cards, where an agent's work appears as a
thing in the room rather than a wall of log messages. That is the centre of
gravity. Features that strengthen it come first.

## Ambition, stated plainly

Den is an open-source project intended for other people to self-host, **not a
business**. Revenue is not a goal. The only commercial consideration that
matters now is not foreclosing options later, which means keeping the licence
posture clean. Growth tactics, pricing and hosted-tenant plans are deliberately
out of scope; do not build for them.

## Threads: adopted, and deliberately not Discord's

`DESIGN.md` previously excluded threads. That is reversed. Four properties,
and the reasoning matters more than the list:

- **Resolve, never archive.** Discord archives because it carries millions of
  channels cheaply. Den does not have that problem. A thread stays open until
  someone resolves it, which matches what a side conversation is: a question
  with an outcome.
- **Zero-friction creation.** Replying to a message *is* the thread. No dialog,
  no naming step; the title is inferred from the parent and can be changed later.
- **Visible in the room.** Open threads appear as a collapsed strip in the
  channel, not hidden behind an icon you have to remember to check.
- **Agent activity threads itself.** A long agent task collapses into a thread
  instead of flooding the room, and a thread can contain objects — the terminal
  session where the bug was chased, living inside the thread about the bug.

Build resolve-not-archive and agent-auto-threading first; those are the two
Discord cannot easily copy. This is not theoretical: Nicholas ran an agent in
Discord threads and stopped using threads entirely because of exactly this
friction.

## Custom emoji: adopted

Also previously excluded. It is cheap and it is how a group makes a space feel
like theirs, which is the whole pitch.

## Canvas: tldraw stays, pinned, and Excalidraw joins it

**tldraw is pinned at 3.15.6 deliberately. Do not upgrade it without reading the
licence.** That version permits commercial and production use free provided the
watermark is retained. The licence on tldraw's `main` branch — which the bundled
`LICENSE.md` links to — prohibits production use without a paid licence. A
routine dependency bump therefore moves denchat.app from compliant to not, with
no visible signal.

Excalidraw (MIT) is added as a **separate object kind**, not a second renderer
behind the same kind, so a board created in one never fails to open in the other.
Users choose. This doubles as the real test of the plugin surface: if a second
canvas cannot slot in cleanly, the plugin system is not finished.

## YouTube playback is an explicit admin toggle, off by default

Today music silently fails unless `yt-dlp` happens to be installed. That is an
accident of packaging, not a position. It becomes a setting an admin turns on,
stating what it requires and that operating it is their responsibility.

The difference matters: "Den can do this" and "Den does this" are not the same
claim, and only the first is defensible if anyone ever hosts Den for others.

## Plugins stay in-tree

Separate repositories cost atomic changes and CI simplicity before there is an
ecosystem to justify them. The exception is capability with a legal posture worth
keeping at arm's length — the YouTube resolver — and that is already out-of-band
as an external binary.

## Android is a known limit, not a gap

Everyone using Den today is on iPhone. Android is genuinely absent from the
roadmap, and for a product with outside users that would be the largest adoption
hole there is. It is not one for this group. Record it as a limit; do not
schedule it against work that helps the people actually using Den.

## Not shipped, and say so honestly

There is **no end-to-end encryption**. Self-hosting means choosing who operates
the server; it does not mean the operator cannot read messages. Do not describe
Den as equivalent to an end-to-end-encrypted messenger.

## What was considered and rejected

Federation, threads-as-Zulip-topics, enterprise administration, per-member
pricing, and growth mechanics aimed at converting invited members into new
organisers. Den is a place for a group of friends. Scope that grows it into a
workplace product is a different product.
