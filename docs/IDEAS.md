# Ideas and non-negotiables from Nicholas

Captured 2026-09-12. These shape later milestones; they are not scope for the current one.

## Watch every stream at once
Discord's worst call behavior: when two people share their screen you can only watch one and have to flip between them. Den must show every share and every cam at the same time. Multiple simultaneous screen shares are first-class in the grid, never a "pick one" switcher.

## One account, multiple devices

Requested 2026-09-13. Stay in the desktop call with a headset while another
device joins on the same account and shares its screen. Every device can share
independently. Mic and playback controls belong to each device. Do not replace
the desktop connection when the phone joins, and do not play the account's own
microphone back to its other devices. Screen-share audio should reach them.

The motivating example is watching Instagram reels shared from an iPhone while
staying ready for the next desktop game. Reliable cross-app screen and audio
capture on iPhone also needs the native ReplayKit broadcast path.

## Your call, your layout
People arrange streams and webcams however they like: drag tiles to reorder, resize them, pin one big and the rest small, pop a tile out into its own window, and have the layout remembered per room. Presets are fine (grid, focus, side-by-side) but they are starting points, not the only options.

## Plugins people can vibecode
Once the core works, Den gets a plugin system in the spirit of Herdr's: a small, documented surface that an agent can write a plugin against in one sitting. Likely shape: client-side plugins that can add panels, tile types, slash commands, and message renderers; server-side hooks over the existing REST and WebSocket API using the same tokens agents already use. The current `AGENTS.md` rule "no plugin systems" holds until the core is solid; until then the job is to keep the client's store and API surface clean enough that a plugin layer can sit on top without a rewrite.

## Open source
Den goes public once it works well. MIT is already set in the workspace. Keep secrets out of the repo, keep the docs honest, and keep the codebase readable by someone who did not write it.
