# Electron desktop acceptance

2026-09-14 overnight. Branch `desktop-electron`.

## Linux call result

**The built Linux AppImage completed a two-participant call.** Both Electron and a
separate Chromium peer decoded incoming video and received non-silent audio from
the other participant. Electron's audio element was playing, unmuted, readyState 4.
The old installed Tauri AppImage reported LiveKit's unsupported-browser error on
the same private Den server, and its renderer had no `RTCPeerConnection`.

[Before](shots/pr/desktop-electron/linux-before.png),
[after](shots/pr/desktop-electron/linux-after.png),
[silent call recording](shots/pr/desktop-electron/linux-call.mp4),
[RTP evidence](shots/pr/desktop-electron/linux-call-stats.json).

The comparison uses `#general`, the light Den theme, and a 1200 by 800 client
viewport. Tests used generated Chromium microphone/camera devices. Codexbox has
no camera device, so this is transport and playback-state evidence, not a physical
webcam or human listening acceptance. At the recorded sample, Electron had decoded
13 remote frames with audio energy 0.1173; the peer had decoded 15 frames with audio
energy 0.1162. See the JSON for bytes sent/received and renderer isolation.

The shared server on port 7000 has no voice configuration and was left untouched.
Two private Den processes on 17010 and 17011 use their own databases and uploads
under `/mnt/storage/den-electron-acceptance`. They use the already-running local
LiveKit at 127.0.0.1:7880, with credentials loaded from
`~/.config/den/livekit.env`. The briefly created test LiveKit container was removed.

## Platform acceptance

| Behavior | Linux, codexbox XFCE/X11 | Windows, `ssh pc` | macOS, `ssh imac` |
| --- | --- | --- | --- |
| Package | AppImage and deb built | NSIS and MSI built; NSIS installed, exit 0 | Universal dmg and zip built unsigned; both x86_64 and arm64 verified |
| Launch | Real AppImage launched using its extract-and-run mode | Installed `Den.exe` launched in the interactive session | Installed separate app launched and rendered login through Chromium CDP |
| Native login and encrypted sessions | Pass, GNOME Secret Service | Pass, DPAPI | Not attempted while login keychain/console locked |
| Add server, switch, merged Inbox, cross-server palette | Pass with two private servers | Pass with two private servers | Not verified while locked |
| Ticket-authenticated sockets and reload restoration | Pass, two origins | Pass, two origins | Not verified |
| Call camera/audio | Two-way decoded generated video and non-silent audio; active playback | Not exercised in this pass | Not exercised while locked |
| Global PTT | Press and release observed while Tauri had focus; no further events after leaving the call | Not exercised in a game | Requires unlocked session and Accessibility permission |
| Native notifications | Implementation present; OS click not yet accepted | Real native toast captured; click focused Electron, selected Electron lab, and opened the correct room | Not verified while locked |
| Tray | Real XFCE menu captured with Open Den, mute, deafen and Quit | Den icon verified in the native overflow; menu interaction not yet accepted | Not verified while locked |
| Signed update | Valid signature accepted; changed manifest and wrong signing key rejected | Same verifier covered by tests | Same verifier; actual self-update also requires a signed Mac app |

Linux native results: [JSON](shots/pr/desktop-electron/linux-native.json),
[Inbox](shots/pr/desktop-electron/linux-inbox.png),
[switcher](shots/pr/desktop-electron/linux-switcher.png),
[tray](shots/pr/desktop-electron/linux-tray.png).

Windows: [before](shots/pr/desktop-electron/windows-before.png),
[after](shots/pr/desktop-electron/windows-after.png),
[native window](shots/pr/desktop-electron/windows-window.png),
[Inbox](shots/pr/desktop-electron/windows-inbox.png),
[switcher](shots/pr/desktop-electron/windows-switcher.png),
[native notification](shots/pr/desktop-electron/windows-notification.png),
[notification navigation](shots/pr/desktop-electron/windows-notification-open.png),
[results](shots/pr/desktop-electron/windows-native.json).
The paired before/after captures use the same room, light theme and 1200 by 800
viewport. The additional OS window capture follows Windows' dark system theme.

macOS: [Electron login renderer](shots/pr/desktop-electron/macos-after-renderer.png).
This is a screenshot from the real installed Electron renderer, not a WindowServer
capture or evidence that the locked console was usable. A matching new Tauri
before capture and native notifications/tray/login could not be accepted while
locked. No signing keys, Xcode configuration, or lock state were changed.

## Implementation and limits

The shared web client detects either shell through `lib/desktop.ts`. The existing
Tauri bridge continues to work. No `apps/web/src/ui` files were changed. Electron
uses a sandboxed, isolated renderer, an allowlisted preload, main-frame/origin
checks for IPC, native authenticated HTTP, and streaming authenticated media.

Sessions are encrypted with Electron safeStorage and keyed by canonical origin.
The main process rejects Linux's `basic_text` fallback. The SSH environment first
selected that fallback; using the unlocked XFCE session's DBus address and
`--password-store=gnome-libsecret` enabled encrypted storage. A FUSE launch stalled
under host load; running the same AppImage via `APPIMAGE_EXTRACT_AND_RUN=1` completed
acceptance. This is the packaged AppImage, not `electron .` or a browser stub.

Native PTT uses `uiohook-napi` for key release as well as press. The hook is active
only in a PTT call. It stops on call exit, renderer replacement, and app exit;
lock/suspend release the held state until unlock/resume. Wayland global PTT remains
unsupported by this hook and reports a clear error. Windows/macOS game-focused,
human-audible PTT remains an attended check.

Electron profiles do not automatically import Tauri credentials. The first
Electron launch requires signing in and adding remembered servers again. Existing
Tauri credentials remain intact. The new bridge does not store bearer tokens in
web storage.

The signed updater verifies the existing Den Minisign key against a platform
manifest before electron-updater receives a version or artifact checksum. Linux
manifests include AppImage and deb, Windows uses NSIS, and macOS uses zip. Release
CI signs manifests with the existing update key. A new PR-only workflow builds
unsigned installers on all three platforms without releasing them. No release,
deployment, or public server change was made for this lane.

An actual newer-version download/install/restart was not performed: there is no
published Electron update and this task forbids cutting a release. Apple Developer
ID/notarization and Windows Authenticode signing are also not acceptance passes.
See [desktop README](../apps/desktop/README.md) for build commands and signing secrets.

## Morning handoff

Windows remains powered on as explicitly requested. The installed Electron
executable is `C:\Users\caron\AppData\Local\Programs\Den\Den.exe`. Review installers
are under `C:\Users\caron\DenElectronNight\apps\desktop\dist`. The running acceptance
profile is `C:\Users\caron\DenElectronNight\electron-profile`; the ordinary installed
shortcut starts the default profile. The private test servers are accessed through
SSH reverse tunnels from codexbox on ports 17010 and 17011. The app can also add
real HTTPS servers normally.

macOS has `~/Applications/Den Electron Night.app`; the existing `Den.app` was
preserved. Universal review packages are under
`~/DenElectronNight/apps/desktop/dist`. It is unsigned apart from the linker ad-hoc
signature. Unlock the console before accepting login, native notifications, tray,
camera, microphone, or screen sharing.
