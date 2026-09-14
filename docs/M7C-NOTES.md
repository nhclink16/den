# M7c — call layouts, multiple shares, quality

Implementation and local M7c smoke are complete. Public deployment and smoke
are in progress; this note will be updated with their results.

## Layouts

The expanded call has Auto, Even and Focus presets on a local 12-column grid.
Auto places each share in its own equal large slot, then the active canvas or
terminal, then cameras. Focus enlarges all pinned tiles and retains every other
tile below. With no pins, Focus enlarges the shared content, or cameras if there
is none. The compact strip keeps its existing horizontal layout.

Drag a tile's name or empty area, or resize from its bottom-right corner.
Colliding neighbours move to free cells. Either interaction selects Custom.
Pin and Pop out appear on hover or keyboard focus. With the tile itself focused,
arrows move, Shift+arrows resize, P pins, O pops out, R resets, and Escape releases
focus. Nested terminal controls retain their own keyboard input. Reset layout
is also in the command palette and the expanded toolbar's overflow.

Layouts use `den.call-layout:<room id>` in localStorage. Departed tiles do not
occupy space, but their saved positions remain available on return. LiveKit
rotates connection identities on rejoin, so vacant saved slots for the same
account are reused, reserving exact matches first to keep simultaneous devices
separate. Clearing browser data clears layouts. If several devices on one account
all reconnect together, their otherwise indistinguishable vacant slots are
assigned in identity order.

Below 900px, a fresh layout defaults to Focus; pins and presets remain available,
and drag, resize and pop-out are disabled. A saved Custom desktop layout is
shown as Focus on mobile without overwriting its coordinates. Tile movement uses
140ms ease and stops animating with reduced-motion preferences.

## Floating tiles

Chromium's [Document Picture-in-Picture API](https://developer.chrome.com/docs/web-platform/document-picture-in-picture)
opens the floating window; browsers without it use a regular popup. A blocked
popup produces an error in Den. The grid keeps the `popped out · bring back`
placeholder. Closing the window or choosing Bring back restores the same mounted
tile. Moving the component's root preserves its event listeners, media element
and plugin instance; leaving the call closes its floating windows. Styles,
including subsequently loaded plugin CSS, are copied into the child document.

## Multiple shares and quality

Each person can publish up to three independent captures, named `screen-<n>`.
The 20px plus beside the main sharing toggle opens the picker again. Each local
share's pill has its source label (28 characters maximum), an individual stop
control, and Smooth/Sharp options. The main toggle stops all shares. Captured
audio is published in the same stream group as its corresponding video and is
unpublished with that share, including when the browser ends capture.

The first share retains LiveKit 2.15.6's existing default: up to 1920×1080 capture
at 30 fps, with a 2.5 Mbps/15 fps publishing cap. Additional shares capture at
up to 1920×1080/15 fps with a 750 kbps/15 fps publishing cap. All start balanced.
Smooth uses maintain-framerate and raises the capture and sender frame-rate caps
to 30; Sharp uses maintain-resolution and 15 fps. Neither changes that share's
bitrate cap. These are maximums, not promised output rates. LiveKit simulcast,
dynacast, adaptive stream and WebRTC congestion control do the adaptation.

Local video pills sample actual sender statistics every two seconds and display
the highest currently sending layer's height and frame rate. An amber dot marks
bandwidth or CPU limitation with the specified tooltip. An idle/unavailable
sender shows a dash. Participant connection quality appears as three small bars,
with the poor-connection tooltip distinguishing local from remote quality.

The call token now allows updating the caller's own LiveKit attributes to carry
source labels to current and late-joining viewers. Data publishing and room
administration remain disabled. Display names come from Den's user directory,
not the caller-editable LiveKit name. No database migration or shared API type
change was needed.

## Verification so far

- `node --experimental-strip-types --test scripts/m7c-layout.test.ts`: three
  tests pass for equal share allocation, collision packing, multi-pin Focus,
  mobile placement and returning/multiple-device identity slots.
- `CARGO_TARGET_DIR=/mnt/storage/den-m8-target cargo test -p den-server --test api calls::`:
  three call authorization/webhook/multiple-device tests pass, including the
  new own-attribute permission and continued denial of data publishing.
- `npm --prefix apps/web run check`: zero errors and warnings. Production web
  build passes; the existing large-chunk advisory remains.
- `node scripts/m7c-smoke.mjs` against the isolated localhost:7001 server:
  three fake-camera contexts, two equal large shares with decoded frames,
  real pointer drag and corner resize with stored coordinates, keyboard move
  and resize, multiple pins and Focus, leave/rejoin with a restored share size,
  native Document PiP playback, same-element return on window close, working
  Smooth control inside PiP with a 30 fps sender cap, window.open fallback,
  Reset, multiple labeled shares from one person, cap at three, individual stop,
  remaining video delivery, sender readouts, viewer bars and mobile Focus pass.
  Fake display capture is real Chromium getDisplayMedia; the test gives its
  otherwise identical fake sources distinct labels. No network shaping.

Local verification uses a private SQLite snapshot under
`/mnt/storage/den-m7c-dev`, separate from the other engineer's running dev server.
Builds and private logs use `/mnt/storage/den-m8-target` and
`/mnt/storage/den-m7c-*.log`. Existing design directories are untouched.
