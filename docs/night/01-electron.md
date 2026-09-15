# Track 1: replace Tauri with Electron

Branch `desktop-electron`. Read `docs/night/README.md` first, then `docs/M4-NOTES.md` for what the current desktop app does, and `docs/M4-DESKTOP-BRIEF.md` for its full behaviour spec.

Nicholas decided to move the desktop app from Tauri to Electron. The reason is not cosmetic: Tauri uses each platform's own webview, and Linux's WebKitGTK cannot do WebRTC, so **calls are broken on Linux today**. Electron bundles Chromium, so one engine serves all three platforms and the media stack matches the browser we already test against. The cost is bundle size, and that is accepted.

## Scope
Replace `apps/desktop` with an Electron project that keeps **every** behaviour the Tauri app has today. Do not drop features in the port. From `docs/M4-NOTES.md` that means: native sessions in the OS keychain keyed by origin, the single-use WebSocket ticket flow, the multi-server switcher with merged inbox and cross-server palette, global push-to-talk registered only during a call, native notifications whose click focuses the window and opens the right room and server, dock badge on macOS and taskbar overlay on Windows, tray with mute and deafen, hide-on-close on Windows and Linux, `den://` deep links with single-instance forwarding, and a signed auto-updater.

Keep the same web client. It already detects Tauri via `window.__TAURI__`; introduce a neutral capability check the client can use for either shell, and leave the Tauri detection working until the Electron app ships, so nothing breaks mid-flight. Coordinate with Fable before changing anything under `apps/web/src/ui/`; you may add a small `apps/web/src/lib/desktop.ts` shim.

Use `electron` with `electron-builder`, both pinned. Context isolation on, node integration off, a preload script exposing a narrow typed bridge. Keychain via `keytar` or Electron's `safeStorage`, your call, documented. Reuse the existing Doorway D icon. Targets: macOS universal dmg, Windows nsis and msi, Linux AppImage and deb.

## What must be proven, and this is the point of the exercise
**A Linux call must work.** Build the AppImage, run it on codexbox, join the hangout against the dev server, and show camera and audio flowing. That single result is why we are doing this. If it still fails, say so loudly rather than burying it.

Also verify on Windows over `ssh pc` if that machine is awake, and on the iMac over `ssh imac`: install, log in, switch servers, receive a native notification, open the tray.

## Release
Extend `.github/workflows/release.yml` to build the Electron targets. Do **not** cut a release or deploy; open the PR and let Nicholas look first.

Open a PR titled "Desktop: move from Tauri to Electron" with a table of what was verified on which platform, screenshots of the app window on each OS you could reach, and a short screen capture of a Linux call working. Post `[astra-electron] PR open` to fable.
