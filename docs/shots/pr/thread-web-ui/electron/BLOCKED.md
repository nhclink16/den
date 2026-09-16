# Real-Electron acceptance: OPEN. First attempts failed; my diagnosis of why was wrong.

Electron itself runs. What cannot be done here is signing in, so the thread
layout has NOT been exercised in a real Electron window. This is an open item,
not a closed one.

## What happened, in order

1. Electron v44.3.0 (the version apps/desktop pins) was launched against an
   isolated Den server on its own Xvfb display, its own user-data directory and
   its own CDP port. It loaded and rendered the native login.
2. **The first sign-in attempt was aimed at the wrong server.** A fresh Electron
   profile defaults to the production origin, and the smoke typed the throwaway
   account into it before setting the instance origin. That request went to
   denchat.app and failed as a bad credential. No credential is recorded here or
   anywhere in this evidence.
3. The ordering was then corrected: the profile is pointed at the isolated
   server, and the origin is verified, BEFORE anything is typed. The attempt was
   not repeated against production.
4. With the origin correct, login fails inside the app with:
   "Unlock your OS keychain to save or restore your Den session."
   That is apps/desktop/electron/session.cjs refusing to persist a session when
   safeStorage reports no real backend (`basic_text`). It is the app's own
   security posture, not a defect.
5. I then reported the cause as "the host has no usable Secret Service",
   citing `secret-tool store` and `secret-tool lookup` failing inside the same
   isolated D-Bus session. **That diagnosis was wrong, and the probe caused it.**

## Corrected diagnosis

`secret-tool` D-Bus-ACTIVATES `org.freedesktop.secrets`. Probing with it started
a second `gnome-keyring-daemon` that won the bus name before the private
`--unlock` daemon did; that private daemon then logged "another secret service is
running" and exited, and the activated instance had no unlocked collection. The
failure was a startup race introduced by the probe, not a missing capability.

The environment lane established this and the fix — poll with
`dbus-send ... ListNames`, which does not activate, until the private daemon owns
the name — with a sentinel-verified working Secret Service on this same box. See
/mnt/storage/den-thread-electron-environment-evidence/RESULT.md.

The original failure output above is kept exactly as recorded. Only the
explanation of it is corrected.

## Not done, deliberately

- The machine's real keyring was not unlocked or modified, and no private
  acceptance profile or its credentials were used.
- No test bypass was added to session.cjs; that is outside the one authorized
  Electron bridge change.
- **Chromium evidence is not Electron evidence.** The landscape, portrait,
  narrow and zoom cases in ../after/ were exercised in Chromium. They prove the
  responsive code path, which is the same single UI codebase Electron renders,
  and they are not labelled or counted as a real Electron window.

scripts/thread-electron-smoke.mjs is complete and runs up to the login step. It
needs a host with a usable Secret Service, or an already-signed-in profile that
is not the private acceptance profile.
