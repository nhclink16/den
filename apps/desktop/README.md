# Den desktop

Electron bundles the existing Svelte client. Node integration is off, context
isolation and renderer sandboxing are on. Only the main frame of `den://app`
receives the allowlisted IPC commands in `electron/preload.cjs`.

```sh
npm ci --prefix apps/web
npm ci --prefix apps/desktop
npm --prefix apps/desktop run build
```

For development, start Vite separately and run
`DEN_DESKTOP_URL=http://localhost:5173 npm --prefix apps/desktop run dev`.
Without that variable, `dev` loads the existing `apps/web/dist`.
`DEN_DESKTOP_DATA` selects an isolated desktop profile for acceptance tests.
Packaged builds never load a development URL.

## Credentials and media

Electron safeStorage encrypts each origin's bearer session. The encrypted blobs
and remembered origins live in the Electron user-data directory, in `native.json`
with mode 600. macOS uses Keychain, Windows uses DPAPI, and Linux uses its desktop
secret service or KWallet. Linux's `basic_text` fallback is rejected. An SSH-launched
Linux test must inherit the unlocked desktop's DBus address and desktop variables.
For XFCE with GNOME Keyring, `--password-store=gnome-libsecret` selects Secret Service.

The main process adds bearer headers and refuses HTTP redirects and origin-changing
paths. The renderer keeps tokens out of web storage. Upload media streams through
`den-media://app` with authenticated range requests. Each instance retains the
existing single-use WebSocket ticket flow.

Tauri's client bridge still works. The new Electron profile does not import Tauri's
saved sessions or local window preferences; sign in and add your servers once.
The old keychain entries remain available to an installed Tauri client.

## Native features

`uiohook-napi` supplies native key-down and key-up events because Electron's
`globalShortcut` only reports activation. Its N-API prebuilds are bundled without
rebuilding against a specific Electron ABI. The hook starts only for PTT calls,
filters to the selected key, retains no other input, and stops on call exit,
renderer navigation/crash, or app quit. Lock and suspend release the microphone;
key presses remain ignored until unlock or resume. macOS requires Accessibility
permission. Global PTT on Wayland is not supported by this hook; the client reports
that limitation and its focused-window PTT remains available.

Screen sharing uses the OS picker where available. Otherwise a native dialog asks
which screen or window to share. Windows can include loopback sound. Linux's fallback
picker shares video only. The existing call pop-outs retain their media nodes.

## Signed updates

`electron-updater` installs updates, using a custom provider that verifies a Minisign
manifest before exposing its artifact URL, version, size, and SHA512 hash to the
downloader. The pinned public key is Den's existing Tauri update key. The manifest
is named `electron-<platform>-<arch>.json`; its detached `.sig` uses Tauri's base64
signature format. Legacy Tauri `latest.json` is never offered an Electron installer.

Checks run at startup and every six hours. Auto-install on quit is disabled; only
the existing Update ready / Restart action installs a verified download. No new
release is published by a build command. A macOS update also requires a signed app.
The PR build is unsigned, so a macOS self-update is not an acceptance pass.

Release CI reuses `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` to sign manifests. The Tauri CLI remains a
build-only signing dependency; there is no Tauri runtime or Rust desktop project.
Optional Apple signing and notarization use `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID`.
Optional Windows Authenticode uses `WINDOWS_CERTIFICATE` and
`WINDOWS_CERTIFICATE_PASSWORD`. Missing platform certificates produce unsigned
installers; missing update signing credentials fail the release pipeline.

To rotate the update key, first ship a verifier that trusts the new public key
alongside the old one, then change the signing secret and retire the old key in a
later release. Never replace the only trusted key in the same release that first
uses it. Apple/Windows certificates can be renewed through the corresponding
repository secrets; keep publisher identity consistent for Windows update checks.

## Verification

`npm --prefix apps/desktop test` covers origin escape/redirect refusal, header
filtering, keychain failures and isolation, and signed-manifest tamper rejection.
`node scripts/electron-call-smoke.mjs` connects to a running acceptance AppImage
at CDP port 19226 and verifies two-way RTP and decoded media against a browser peer.
`node scripts/electron-native-smoke.mjs` checks the real native bridge, multi-server
UI, Inbox, palette, saved sessions, and ticket sockets. Both require private test
credentials and servers. See `docs/ELECTRON-NOTES.md` for real-machine results.
