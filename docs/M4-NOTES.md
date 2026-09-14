# M4 desktop delivery — 2026-09-14

Desktop releases are published, the installed Windows and Linux clients upgraded
through the signed updater, and the paired-theme server is deployed. Full human
acceptance remains open for audible push-to-talk in a game. The macOS follow-up
passed several first-session checks. Later blank captures were taken while macOS
was locked, so unlocked relaunch acceptance is still required. See the diagnostic
correction below; those captures do not establish a Den rendering defect.
Associated HTTPS links also remain an implementation follow-up; `den://` links
are implemented and verified. These are not recorded as passes.

## Releases and installed versions

- [v0.2.0](https://github.com/nhclink16/den/releases/tag/v0.2.0), source
  `d6aaa1e`: first desktop release, old appearance schema. CI run
  [34802715164](https://github.com/nhclink16/den/actions/runs/34802715164) passed.
- [v0.2.1](https://github.com/nhclink16/den/releases/tag/v0.2.1), source
  `698f9f8`: paired themes, sidebar collapse controls, serialized incoming events
  so a newly created DM can resolve its channel before showing its notification.
  CI run [34803837359](https://github.com/nhclink16/den/actions/runs/34803837359)
  passed all three desktop and three server/CLI builds, then publication.
- Both releases contain a universal macOS DMG and application archive, Windows
  MSI and NSIS EXE, Linux AppImage and deb, updater signatures, `latest.json`,
  source and license archives, and `SHA256SUMS`.
- macOS packages are **not Developer ID signed or notarized**. Windows installers
  are **not Authenticode signed**; SmartScreen may warn. Updater signatures are
  present and are a separate signing mechanism.

| Machine | Installed artifact and location | Current version |
| --- | --- | --- |
| Windows PC, `pc` | Initially MSI at `C:\Program Files\Den`; updater launched NSIS, current executable is `C:\Users\caron\AppData\Local\Den\den-desktop.exe` | 0.2.1, confirmed through the running Tauri app |
| codexbox, Debian 12/XFCE | `~/Applications/Den.AppImage` | 0.2.1, updater replaced the file; SHA256 matches published artifact |
| iMac, `imac` | `~/Applications/Den.app`, installed from universal DMG | 0.2.1, manually replaced after 0.2.0; plist version and running process verified |

The v0.2.1 AppImage hash is
`b5e426385db7a2601d80d25e03403203b3e13daf1cdca5f4acc5093329a552bf`.
The macOS v0.2.1 DMG matched `SHA256SUMS`; `hdiutil` verified its image checksums.

One late correction, `78565fa`, is on main but **not in the published v0.2.1
binaries**: native Settings now constructs copied invite URLs from the active
server origin rather than the bundled webview origin. Until the next release,
create/copy invites in the web client. Existing invite URLs can be added in the
desktop client.

## Implemented behavior

`apps/desktop` bundles the Svelte client in Tauri 2, with Doorway D icons, a
900×600 minimum, saved window position/size, native Windows/Linux title bars,
and a macOS overlay title bar with sidebar clearance. The dependency versions
are pinned. Two additions beyond the requested plugins are justified in
`Cargo.toml`: single-instance forwards Windows/Linux deep links into the existing
window; notify-rust supplies desktop notification click callbacks that the
notification plugin does not expose.

Native credentials live in the OS keychain, keyed by canonical origin. Rust
performs authenticated HTTP and media requests; tokens are not put in URLs or
web storage. Native media uses `den-media` URLs and forwards range requests.
Cross-origin redirects are refused. Browser cookie authentication and origin
checks are unchanged.

The server provides public `/instance` metadata and authenticated
`POST /auth/ws-ticket`. Tickets expire after 30 seconds, are stored as SHA256
digests, consumed once, and checked against the originating session's continued
validity. Cookie ticket minting retains CSRF protection. Shared types originate
in den-core and the client schema is generated from OpenAPI.

Each remembered origin has its own store and ticket-authenticated WebSocket.
The wordmark switcher, background attention indicator, unread/@ counts, merged
Inbox, cross-server palette, and Ctrl/Cmd+Shift+1…9 / ] shortcuts use these stores.
Calls retain their originating store when switching; logging out of that origin
leaves its call. Theme caches and in-flight saves retain their origin, including
the M9b paired schema.

Native features include global PTT registration during calls, native DM alerts
with room navigation, Windows unread overlay/macOS badge support, tray controls,
hide-on-close, scheme deep links, and launch/six-hour update checks. A verified
download exposes the quiet “Update ready · Restart” control.

The shared web/desktop sidebar button uses the existing preference and shortcut.
Collapsed rooms, Inbox, Search and Settings expose the restore button first in
their headers. Narrow layouts keep their existing menu control.

## Acceptance results

“Needs Nicholas” means no pass is claimed. The browser stub is separate from
the real-machine evidence below.

| Check | Windows / WebView2 | Linux / WebKitGTK | macOS / WKWebView |
| --- | --- | --- | --- |
| Install and launch real release | Pass, MSI exit 0 | Pass, AppImage | Pass first-session launch and real window capture; subsequent relaunch capture invalidated by locked macOS session |
| Public login, OS credential persistence | Pass, retained after update | Pass, Secret Service; retained after update | Pass public login; persistence after restart awaits unlocked acceptance |
| Add second server, switch and restore | Pass, public + private test server | Pass, public + two private servers | Pass add/switch public + private server; restart restoration awaits unlocked acceptance |
| Native DM notification | Pass, OS toast captured | Pass, OS notification captured after update | Blocked by locked session; private test DM sent, no native toast verified |
| Click notification to focus/switch/open room | Pass, returned to public server and requested room | Native notification action rendered; click navigation not separately accepted | Not verified; notification delivery blocked |
| Tray/menu and app window | Captured and inspected | Captured and inspected | Pass, real app and menu-bar menu captured and inspected; Quit exited the process |
| Call and microphone | Pass, joined public LiveKit room | Failed: LiveKit reports unsupported browser | Pass public call join and local mic control toggles; no remote audio confirmation |
| Global PTT with another app focused | Press and release observed with Notepad foreground; audible/game check needs Nicholas | Global shortcut press/release event observed, but call unavailable | Not tested; audible/game check still needs Nicholas |
| System screen picker and simultaneous shares | Pass, two active screen-share tiles captured | `getDisplayMedia({video:true})` rejected with `OverconstrainedError: Invalid constraint` before picker | Pass real system picker; no active share confirmed and no multiple-share pass |
| Call remains attached across server switch | Pass, dock retained `hangout · Den` | Blocked by unsupported calls | Not exercised in this pass |
| Native upload and authenticated thumbnail | Pass, file uploaded in UI; `den-media.localhost` thumbnail loaded with nonzero natural width | Basic native HTTP/media path exercised; upload not separately accepted on release | Not exercised in this pass |
| `den://join` routing | Pass through registered OS scheme | Pass through repeat app launch/forwarding | Pass registered OS URL opened the private-server preview/login flow |
| Signed 0.2.0 → 0.2.1 updater | Pass, signed download, restart, running version 0.2.1 and both sessions retained | Pass, restart, release hash, all three sessions retained | Installed 0.2.1 matches latest manifest; updater installation remains unverified |
| Paired Appearance against deployed server | Pass, eight families; selected Tide in native UI and read `theme=tide` back; restored original preference | Paired private-server themes rendered after update | Per-account themes rendered across origins; editor/save not exercised |

The Windows update crossed from a machine-wide MSI to the per-user NSIS install.
It displayed an MSI removal confirmation and an elevation prompt. Subsequent
inspection found the updated app running from the per-user directory. Removal
of the old MSI registration was not separately verified; check Installed Apps
for an obsolete second Den entry during attended acceptance.

Linux reports: “LiveKit doesn't seem to be supported on this browser. Try to
update your browser and make sure no browser extensions are disabling webRTC.”
This is an actual runtime failure, not a missing automation permission. Use the
browser for calls on this machine. The AppImage notification currently shows a
generic/missing icon in XFCE; notification delivery itself works.

The initial unattended iMac pass could not capture the displays. The follow-up
below supersedes that capture limitation. The earlier signing probe found zero
valid identities; signing/keychain setup was not inspected or changed during the
follow-up because Nicholas was working on it separately.

### macOS follow-up, 2026-09-14 00:15–00:48 EDT

Tested the installed, unsigned `~/Applications/Den.app` v0.2.1 on
`Nicholas-Work.local`, macOS 26.6.1. No Xcode, keychain sign-in, signing identity,
or system signing configuration was changed. No quarantine attribute was present
on the app at the final check, so no quarantine removal was needed.

Direct SSH `screencapture` failed even with the displays awake;
`CGPreflightScreenCaptureAccess()` returned false. Nicholas enabled Screen &
System Audio Recording for Terminal. Driving `screencapture` through a dedicated
GUI Terminal window then worked. The app/menu images use real WindowServer IDs
with `screencapture -x -o -l`; the picker uses `-R680,183,1200,800` around Den's
window. These are actual captures, not reconstructed UI or browser substitutes.
The dedicated capture window excluded Nicholas's Xcode work.

The first running app session successfully logged into denchat.app as Nicholas.
`open 'den://join?url=http%3A%2F%2F127.0.0.1%3A17900'` opened the registered scheme,
previewed First Den, and accepted the private test account login. A temporary SSH
reverse tunnel connected that URL to the existing disposable M4 database on
codexbox. The switcher showed both servers, and selecting each changed the
account, room contents and theme. Input used macOS accessibility controls and,
where text events were needed, keyboard events addressed to Den's PID.

The public hangout joined with one participant and a live call dock. The UI
reached `Mute microphone`, then responded to mute with `Unmute microphone`.
That establishes local control behavior, not audible reception by another person.
The menu-bar menu displayed Open Den, checked Mute microphone, Deafen and Quit.
Quit later exited the application process.

Share screen opened macOS's Shared Content Picker Controls. A real capture shows
`Share This Window` over Den. Selection did not produce a confirmed active share
or screen tile. The session subsequently stopped exposing its main window through
accessibility, while WindowServer could still capture its last rendered content.
The sequence does not establish that the picker caused the later failure.

**Historical observation, superseded by the lock-state diagnosis below.** Quitting and reopening v0.2.1 produced a persistent blank
window. Repeating through GUI Terminal, both with `open` and with the bundle's
executable directly, also produced a blank window. Screenshots record both paths.
The native event loop remained alive; a one-second process sample did not show it
blocked in a keychain operation. Unified logs included these WebKit child errors:

```text
WebContent[10671] Failed to look up the port for "com.apple.windowserver.active" (1)
WebContent[10671] failed to do a bootstrap look-up: xpc_error=[1: Operation not permitted]
WebContent[11377] Failed to look up the port for "com.apple.windowserver.active" (1)
```

These logs alone did not establish a root cause. The later lock-state diagnosis
below invalidates the visual failure verdict. Repeat relaunch, restart persistence
and updater acceptance with the console session unlocked.

A private DM was sent successfully with HTTP 200 after relaunch, but no native
notification was verified while the app was blank. There is therefore no macOS
notification screenshot or notification-click pass. A substitute system-script
notification was not used. The installed bundle reports 0.2.1; the GitHub manifest
retrieved from the iMac also reports 0.2.1 with both Darwin architectures. No update
chip appeared in the working first session, but this is not proof of an updater
check succeeding. Download verification, installation and restart through the
macOS updater remain unverified.

The blank app, dedicated capture Terminal window, temporary reverse tunnel,
disposable server and capture job were stopped. Test scripts and temporary
credential input were removed. The installed app and its remembered server data
remain. First Den is offline until the disposable server/tunnel are restarted.

### Locked-session diagnosis, 2026-09-14 00:53–01:10 EDT

The earlier blank-window verdict was premature. `ioreg -n Root -d1` reported
`CGSSessionScreenIsLocked=Yes` and `CGSSessionScreenLockedTime=1789360621`, which
is **00:37:01 EDT**, matching the start of the failed visual checks. The existing
`caffeinate -u -dmi -t 7200` process did not prevent the session from locking.
Terminal, Finder and Den all exposed zero accessibility windows while locked.

The unchanged installed v0.2.1 was launched from GUI Terminal with `RUST_LOG=debug`
and stderr redirected outside the repo. It produced no stderr. App and WebKit
unified logs were collected with `log stream --level debug --predicate`. A process
sample showed the release's main event loop idle. A separate temporary debug
bundle enabled webview devtools and logged page-load events, JavaScript errors,
keychain boundaries and the DOM text at five seconds. No debug code was added to
the installed app or the repository's application sources.

The debug build loaded `tauri://localhost` successfully. With a fresh private
origin, missing keychain reads returned, `/settings` returned 200, `/users/me`
returned 401, and the five-second DOM report contained the complete login form.
Its real `screencapture` image was nevertheless blank. The WKWebView was attached,
visible and 1200×800. Removing all Den plugins and rendering a static green page
also produced a blank capture. A separate minimal Objective-C WKWebView app
launched through Finder reproduced the same inaccessible window behavior. This
rules out restored sessions, theme cache and Den's media protocol as necessary
causes of the locked-session visual symptom.

The debug bundle's reads of existing credentials waited inside
`SecKeychainFindGenericPassword`; missing entries returned immediately. This was
measured while locked and with a different executable signature. It is not proof
that the installed release stalls reading its own credentials while unlocked.
The original keychain entries were preserved. Temporary remembered-origin changes
were restored to the public server plus the existing private First Den server.
Xcode and keychain sign-in were not changed. The separate diagnostic build used
an updated Rust toolchain and an ad-hoc signature without a signing identity.

The new `scripts/m4-smoke.mjs` regression starts a cold browser context with two
pre-seeded OS-keychain bridge sessions. It requires the active server's room link
to become visible within five seconds, without a login form or JavaScript errors.
It passed in **435 ms**, followed by the full native stub smoke and verified
cleanup. This checks application startup; it does not substitute for an unlocked
WKWebView capture or a real keychain/updater test.

**Pending:** unlock the iMac, then repeat the unchanged release first. No v0.2.2
root-cause fix or macOS updater pass is claimed from locked-session evidence.
The requested release and installed-app acceptance remain open.

### Evidence

- Windows: [app](shots/m4-windows-app.png),
  [switcher](shots/m4-windows-switcher.png),
  [notification](shots/m4-windows-notification.png),
  [tray](shots/m4-windows-tray.png),
  [two screen shares](shots/m4-windows-shares.png),
  [update chip](shots/m4-windows-update.png),
  [retained servers after update](shots/m4-windows-updated.png),
  [paired themes](shots/m4-windows-paired-themes.png),
  [collapsed sidebar](shots/m4-windows-collapsed.png).
- Linux: [updated app](shots/m4-linux-app.png),
  [switcher](shots/m4-linux-switcher.png),
  [notification](shots/m4-linux-notification.png),
  [tray](shots/m4-linux-tray.png),
  [call failure](shots/m4-linux-call-limit.png),
  [update chip](shots/m4-linux-update.png),
  [deep link](shots/m4-linux-deeplink.png).
- macOS: [app](shots/m4-macos-app.png),
  [switcher](shots/m4-macos-switcher.png),
  [call](shots/m4-macos-call.png),
  [menu-bar menu](shots/m4-macos-tray.png),
  [system picker](shots/m4-macos-picker.png),
  [blank relaunch](shots/m4-macos-restarted.png),
  [blank GUI relaunch](shots/m4-macos-restarted-gui.png).
- Browser: [public collapsed sidebar](shots/m4-public-sidebar-collapsed.png).
  Stub-only evidence: [switcher](shots/m4-native-stub-switcher.png),
  [Inbox](shots/m4-native-stub-inbox.png),
  [collapsed header](shots/m4-sidebar-collapsed.png).

Windows app captures come from the installed WebView2; toast/tray images capture
the native screen region. Linux images capture the actual XFCE display. Screenshots
were visually inspected before commit `5408b2a`.

## Automated verification and public rollout

- Server integration suite: 25 passed, including bearer-authenticated uploads,
  objects and terminal permission paths. Ticket reuse, revocation and cookie CSRF
  behavior passed again after the paired-schema merge. The ticket expiry unit test
  checks the 30-second boundary without wall-clock sleeps.
- Workspace Cargo check, native Cargo check/build and native Clippy with warnings
  denied passed. Local Cargo used `CARGO_TARGET_DIR=/mnt/storage/den-m4-target`.
- Svelte check: zero errors and warnings. Production web build passed. The final
  one-line invite-origin correction also passed Svelte check.
- `node scripts/m4-smoke.mjs` passed against two private servers: native login,
  bearer-only HTTP, two real ticket WebSockets, switcher/shortcuts, per-origin
  appearance, merged Inbox, cross-server palette, restored sessions, keychain
  logout and all four sidebar headers. The Tauri bridge is stubbed in this test.
- Public web login and sidebar collapse/restore passed in room, `/inbox`,
  `/find?q=M4` and `/settings`, with zero page errors.
- M9b deployed the exact v0.2.1 archive after the explicit desktop
  ready-to-deploy handoff. Its backup, migration-8 and public theme smoke evidence
  is in [M9 notes](M9-NOTES.md). Native paired-theme acceptance above ran afterward.

Private smoke databases/credentials are outside Git at
`/mnt/storage/den-m4-smoke`. To repeat the stub run, start the two den-server
processes on `127.0.0.1:17900` and `:17901` using its `one` and `two` databases,
and Vite on `localhost:17902`, then run `node scripts/m4-smoke.mjs`. The script
documents the credential/bootstrap setup. The disposable servers, Vite and native
automation driver were stopped after acceptance; remembered test origins will be
offline until those servers restart. The existing development server was left alone.

## Signing prerequisites and rotation

The updater private key is on codexbox at
`/home/nicholas/.local/share/den-desktop/updater.key`, mode 0600, outside the repo.
Its public counterpart is `updater.key.pub`; the public key is embedded in
`apps/desktop/tauri.conf.json`. GitHub Actions uses
`TAURI_SIGNING_PRIVATE_KEY` and optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
Keep a private backup. For planned rotation, first ship a release trusted by the
old key that embeds the new public key. Confirm every installed client has taken
that bridge release before changing the CI private key for later releases. Changing only CI would strand existing clients. If the old key is lost,
provide a separately verified manual installer.

Nicholas must supply Apple signing before a Developer ID release:

1. In Keychain Access, use Certificate Assistant → Request a Certificate From a
   Certificate Authority; save the CSR to disk.
2. In Apple Developer → Certificates, Identifiers & Profiles → Certificates → +,
   choose **Developer ID Application**, upload the CSR, download the certificate,
   and import it into the same Mac keychain containing the private key.
3. Export that certificate and private key as a password-protected `.p12`.
   Put its base64 content in GitHub secret `APPLE_CERTIFICATE`; set
   `APPLE_CERTIFICATE_PASSWORD` and `APPLE_SIGNING_IDENTITY` to the matching
   `Developer ID Application: … (TEAMID)` identity.
4. Set `APPLE_TEAM_ID` and `APPLE_ID`. At account.apple.com → Sign-In and Security
   → App-Specific Passwords, generate a notarization password and store it as
   `APPLE_PASSWORD`.
5. Build the next tag; verify the signing/notarization job, download in Safari,
   and confirm Gatekeeper accepts it. Rotate by replacing the certificate/P12
   secrets or revoking/reissuing the app-specific password, then verify a new
   release before retiring working credentials.

The workflow already selects signing/notarization when `APPLE_CERTIFICATE` exists.
See [Tauri's signing instructions](https://tauri.app/distribute/sign/macos/) and
[Apple's distribution signing guide](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/).

## Remaining attended and implementation checks

On the iMac, first unlock the console session and recheck the installed release.
Then repeat restart persistence, native notification delivery/click navigation and
updater installation. Diagnose an application defect only if it reproduces unlocked. Verify actual screen-share publication and two simultaneous shares,
not only the system picker. Global PTT still needs a second participant to confirm
speech only while held, including with another application/game focused. These
human-dependent checks are not claimed. Gatekeeper/notarization acceptance waits
for Nicholas's signing setup and a signed release.

On Windows, repeat the audible PTT check while an actual game has focus. Inspect
Installed Apps for any obsolete MSI entry from the first release. The updated
per-user installation is the accepted one.

The requested associated HTTPS domain is **not implemented** in these packages.
The login page offers the working `den://` scheme. macOS still needs a signing
identity/provisioning setup, the associated-domains entitlement, and an AASA file
for the real team/app identifier. Windows website association requires package
identity (MSIX or a sparse package), which these unsigned MSI/NSIS packages do not
provide. This needs packaging work and then real OS activation tests; it is not
equivalent to custom scheme registration. References:
[Apple associated domains](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.associated-domains),
[Microsoft app URI handlers](https://learn.microsoft.com/en-us/windows/apps/develop/launch/web-to-app-linking).

## Windows shutdown

Windows acceptance was recorded before shutdown. Seven temporary `Den-M4-*`
scheduled tasks were removed. At **2026-09-14 04:08:00 UTC**, the requested safe
command was issued over SSH with a 60-second delay and the cancellation notice;
Git Bash used `MSYS_NO_PATHCONV=1` to preserve Windows slash arguments:

```text
shutdown.exe /s /t 60 /c "Den desktop acceptance finished. Cancel with shutdown /a."
```

At **04:09:23 UTC**, `tailscale status --json` reported the Windows peer
`nicholas` with `Online: false`: 83 seconds after scheduling, within three minutes.
No `/f` and no smart-plug power action were used.
