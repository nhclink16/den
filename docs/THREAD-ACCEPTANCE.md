# Thread acceptance runners

Two manual runners. They are acceptance tools, not production behaviour, and
they are deliberately **not** in CI: both need a real server, a real fixture and,
for the Electron one, a real X display and a Secret Service. CI runs the focused
node tests instead.

    scripts/thread-web-ui-smoke.mjs      real Chromium, two clients, live events
    scripts/thread-electron-smoke.mjs    real Electron window, natively resized

## What they need

Both talk to an **isolated** server — never the shared one on :7000, and never
`vite dev` against it. Build the client and point a spare-port server at the
build:

    npm --prefix apps/web run build
    DEN_DB=<scratch>/den.db DEN_UPLOADS=<scratch>/uploads \
    DEN_BOOTSTRAP_FILE=<scratch>/bootstrap.txt \
    DEN_WEB_DIR=apps/web/dist \
    DEN_ORIGIN=http://localhost:17061 DEN_BIND=127.0.0.1:17061 den-server

`DEN_SMOKE_URL` must match `DEN_ORIGIN` exactly, host included. Pointing the
runner at `127.0.0.1` while the server's origin says `localhost` leaves reads
working and rejects every write, which looks exactly like a broken composer.

    DEN_SMOKE_URL        server base URL, identical to DEN_ORIGIN
    DEN_SMOKE_PASSWORD   password for the fixture's `nicholas` account
    DEN_SMOKE_COMMIT     optional, recorded in the results provenance

The browser runner also exercises a terminal object inside a conversation. That
step needs a `den-host` connected to the *same* isolated server
(`DEN_HOST_CONFIG_DIR=<scratch>/hostcfg den-host run`). Without one it records
`terminalInConversation: {ran: false, why: ...}` and does not silently pass.

## Fixtures are per-run and independent

Each runner creates its own channel, roots and conversation. Neither depends on
the other having run first, and neither reuses whatever an earlier run left
behind — the Electron runner used to adopt the newest `workshop-*` channel,
which had grown to 73 replies and made every run a different test.

## Baseline mode

`BASELINE=1 node scripts/thread-web-ui-smoke.mjs` captures the control against
an **unchanged** client: it records the three viewport shapes, asserts that no
thread strip or panel exists, and stops there. Everything after that point
asserts behaviour an unchanged client cannot have, so it does not run — against
the activated client the same flag fails immediately on `strip == 0`, which is
the assertion doing its job. The full assertion set requires the companion
activation change. Do not weaken an assertion to make a runner pass against a
client that does not have the feature yet — a passing runner on a pre-feature
client proves nothing, and the combined tree is what establishes the result.

## The Electron runner

It **launches** Electron itself through Playwright's Electron launcher rather
than attaching over CDP, because Electron's zoom is a `webContents` property and
only the main process can reach it. Attaching over CDP is what produced an
earlier zoom result that measured nothing: `Emulation.setPageScaleFactor`
returned without throwing while devicePixelRatio and CSS width never moved.

    DEN_ELECTRON_BIN   electron binary
    DEN_APP_DIR        the desktop app directory (contains electron/main.cjs)
    DEN_PROFILE        a throwaway user-data directory
    DISPLAY            an X display; the window is resized with xdotool

It asserts, rather than assumes:

- exactly one window, since it mixes main-process and renderer measurements
- the app is actually talking to the isolated origin, checked by a real request
  **before** any credential is typed — a fresh profile defaults to production
- every native resize actually arrived, or the run is reported blocked
- zoom changed: `getZoomFactor()` 1 → 2, DPR 1 → 2, CSS width halved, with the
  native window bounds unchanged so the comparison means zoom and not a resize
- panel controls sit inside the viewport on **both** axes; a header that wraps
  under zoom can push a control below the fold rather than past the edge

The Secret Service it needs must be disposable: a private `dbus-run-session`
with private XDG directories, whose ownership of `org.freedesktop.secrets` is
confirmed by PID before use. Never unlock the real keyring and never reuse a
personal profile. Keep that root path short — the control socket is an
`AF_UNIX` path and silently truncates past 108 bytes, which surfaces as
`Address already in use` on a directory you just created.
