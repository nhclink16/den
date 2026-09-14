# M5a native iOS text

## September 14 implementation update

The native SwiftUI client now exists in `apps/ios`. It builds with Swift 6 strict
concurrency for iOS 26, installs with automatic signing, and has launched on
Nicholas's iPhone. This replaces the disposable signing probe. **M5a is not yet
fully accepted:** closed-app APNs delivery still needs the server key, and the
remaining device checks below are separate from simulator results.

### Implemented

- Generated public Swift client from the exact server OpenAPI; canonical per-origin
  bearer sessions in Keychain, one-use WebSocket tickets, redirect rejection,
  reconnect/refetch, and protected cached text. No API credential goes in a URL,
  screenshot, fixture output, or repository file.
- Native Rooms/Inbox/Search/Settings, categorized channels, DMs/group DMs,
  author grouping and day/unread separators, Markdown/mentions/code, replies,
  reaction toggle, own-message edit, own/admin delete, typing and presence.
- Chunked Photos/Files uploads, progress/retry/removal, authenticated ranged
  AVPlayer playback and seeking, image thumbnails and protected file sharing.
  Upload completion is based on `complete`, not the existence of an upload ID.
  Confirmed sends clear attachments before any fallible history refresh.
- Eight shared paired theme families, all 15 licensed font families, light/dark/
  system appearance, native tab/toolbars, iPad split navigation, and read-only
  Machines/Access. Canvas and terminal messages remain explicit M5c placeholders.
- APNs permission, device registration/removal before logout, retry after transient
  registration failures, badge updates and message deep links after reauthentication.
  Server device/push implementation and deployment are recorded separately in
  [server notes](M5-SERVER-NOTES.md).

Nicholas tested the physical app and confirmed **video scrubbing works**. His
composer feedback is implemented: one uniform rounded surface, no glass treatment
in the composer, a smaller send control with a 44-point hit target, Return to send,
and Shift-Return for a newline. Multiline paste and input-method composition are
preserved. The other tab/toolbars retain the brief's native iOS treatment.

### Reproduction and versions

Generate with XcodeGen **2.45.4** using `xcodegen generate --spec apps/ios/project.yml`.
The shared scheme is `Den`, bundle `app.denchat.ios`, team `UH434K44A3`.
OpenAPI Generator **1.13.1**, runtime **1.12.1**, URLSession transport **1.3.1**,
and HTTPTypes **1.8.0** are exact package requirements with a committed package lock.
The checked snapshot builds offline; `apps/ios/scripts/regenerate-api.sh` refreshes
it explicitly. Public `https://denchat.app/openapi.json` was independently checked
for parsed equality after server deployment. Snapshot SHA-256:
`b2fd728d912bbcdae29020f28f501a2a628d6051ce47386614dcc1a1dfa5f480`.

Resource source URLs, licenses, font commit and hashes are in the bundled font
manifest. `python3 apps/ios/scripts/sync-resources.py --check` checks shared themes,
fonts and assets. Fonts are bundled, not fetched by the running app.

### Evidence so far

- Real signed application build, install and launch on Nicholas's iPhone, not the
  earlier probe. Aqua build receipts are under
  `~/.local/share/den-ios-tools/runs/aqua-dzbmvbjz/` (exit 0).
  The subsequent composer build is `runs/aqua-2o66c3cr/` (exit 0).
- Thirteen Swift tests passed, including real AVPlayer seek against an authenticated
  range stub, lost chunk-response offset recovery, origin/Markdown/grouping logic,
  and the confirmed-send boundary. Every new test was deliberately broken, failed,
  restored and passed. Notification retry and queued-tap recovery are included in that passing suite.
- The real-server UI test has exercised login, Keychain restoration, message
  send/edit/reply/reactions/delete, DM, Inbox, search and server-persisted Tide/dark
  appearance. The final combined suite passed **14/14** (13 Swift tests plus the full real-server UI flow), zero warnings.
  It found and fixed binary typing frames closing the socket, cancelled view
  requests appearing as offline errors, and keyboard-obscured tab navigation.
  Its sign-out confirmation selector was adapted to the actual sheet rather than
  the equally labelled background row; logout assertions remain intact.
- Test data is on the isolated Debian fixture, reached by a loopback SSH forward;
  no friends' conversations/settings were used. Credentials stay in private files.
  The native fixture hook exists only in debug simulator builds, requires an
  explicit launch argument and loopback origin, and still uses the real login UI.

Final combined result: `test_sim_2026-09-14T15-47-03-281Z_pid86141_44f9caaa.xcresult`.
The UI test deliberately failed when Return stopped calling send, then passed after
exact source restoration. Layout captures in [shots](shots/) show the real simulator
composer, Inbox and appearance; they are not phone screenshots.

Local xcresults are under
`~/Library/Developer/XcodeBuildMCP/workspaces/den-65f821c5653d/result-bundles/`.
Mutation receipts: `/tmp/den-ios-unit-proof.md`,
`/tmp/den-ios-media-mutation-proof/README.md`, and
`/tmp/den-ios-send-boundary-proof/README.md`,
`/tmp/den-ios-notification-proof/README.md`, and
`/tmp/den-ios-ui-mutation-proof/README.md`. These temporary local artifacts are
not durable Cloud run links. No Xcode Cloud run is claimed here.

### Photos, Files and iPad follow-up

The real simulator Photos picker selected a four-second synthetic video. Den
uploaded and sent it; its authenticated download matched the original 1,557
bytes and SHA-256 `c92042cbf78c77a55010fedd2e639eef16cb3cc8bc5c46df019081d050f952bc`.
The native AVPlayer scrubber moved from one second elapsed to three seconds.
Save to Files, reimport through the Files picker, and send produced the same bytes.
These were temporary UI automation probes, not additional retained regression
tests. Evidence: `/tmp/den-ios-photos-files-proof/README.md`; final result
`test_sim_2026-09-14T16-10-25-279Z_pid99391_9a98d157.xcresult`.
[Actual video controls](shots/m5a-iphone-video.png) show the simulator, not the phone.

An iPad cold launch with the owned SSH fixture forward closed restored cached
rooms and messages without a modal error. Reopening that forward automatically
reconnected and displayed a message created during the outage, without tapping
refresh. The Offline indicator disappeared. The server itself was never stopped.
Evidence: `/tmp/den-ios-offline-proof.json`, simulator process 50535, semantic
snapshots 11 and 12. [Cached chat](shots/m5a-ipad-offline.jpg).

Largest accessibility text initially crowded the split sidebar. At those sizes,
Rooms now uses full-width navigation; author and timestamp stack, and decorative
icons/avatar initials stay inside their bounds. Normal iPad split navigation is
preserved. The edited app built and ran successfully in
`build_run_sim_2026-09-14T16-13-08-182Z_pid86141_c9112448.log`.
Inspected captures: [large rooms](shots/m5a-ipad-accessibility-rooms.jpg),
[large chat](shots/m5a-ipad-accessibility-chat.jpg),
[normal light split view](shots/m5a-ipad-light.jpg). The simulator's text setting
was restored to its original Large value. This is visual/semantic inspection,
not a completed VoiceOver hardware audit.

### Device acceptance still to finish

| Check | Result |
| --- | --- |
| Signed real Den install and launch | Pass, native device tools |
| Physical video scrubbing | Pass, Nicholas's report |
| Revised composer appearance and Return behavior | Updated signed install/launch; simulator Return-to-send and layout pass |
| Offline cold cache and reconnect | Pass, iPad cold launch with fixture forward closed, cached chat visible without modal; automatic reconnect received the message sent during outage |
| iPad, Dynamic Type, light/dark captures | Pass, iPad Pro 11-inch M5 iOS 26.5, normal light/dark and largest accessibility text; full-width navigation at accessibility sizes |
| Closed-app mention/DM push and tap | Blocked: APNs key/config missing |
| Notification permission denial and Settings recovery | Native tests pass; hardware follow-up remains |
| Calls, mic/camera/route/background/lock | M5b, not text acceptance |

For push, supply the private `.p8`, Key ID and Team ID to the server configuration
as described in server notes, using the signing-appropriate APNs environment.
Then close Den, send a disposable-account mention/DM from a second client, verify
the alert/badge, and tap into the exact message. Do not treat registration success
or a simulated payload as this acceptance check.

## Historical signing investigation

The following notes describe the earlier probe and resolved signing blocker, not
the current native implementation status.

# M5a iOS signing setup, 2026-09-14

## Signing gate cleared at 10:18 Eastern

Nicholas selected his developer team, connected the phone, enabled Developer
Mode and approved signing access. A new run on the iMac then verified:

- Team `UH434K44A3`, with a valid Apple Development signing identity.
- An automatically created iOS development provisioning profile.
- iPhone 17 Pro Max available and paired, Developer Mode enabled and developer
  disk image services available. The phone reports iOS 27.0.
- `xcodebuild` completed the signing probe with `BUILD SUCCEEDED`.
- `xcrun devicectl device install app` installed `app.denchat.ios` on the phone.
- `xcrun devicectl device process launch` launched it successfully, process 705.

The installed app is still the disposable **DenSigningProbe**, displaying
`Den signing check`. It is not the Den client. No chat or M5a feature acceptance
is claimed. The implementation and APNs work listed below remain outstanding.

### Remote signing execution context

Direct SSH builds still saw the default keychain as locked even after Nicholas
unlocked it in the desktop session. Repeating the build in the logged-in Aqua
session succeeded. No password was copied, stored or passed in a command.

The verified method used a temporary LaunchAgent named
`app.denchat.m5a-signing-check`, bootstrapped into `gui/502` over SSH. Its plist
set `LimitLoadToSessionType` to `Aqua`, `RunAtLoad` to true, and invoked
`/bin/bash /tmp/den-m5a-signing/gui-build.sh`. The script ran the same
`xcodebuild` command documented below, with
`-allowProvisioningDeviceRegistration` added. It wrote a log and exit code.
The LaunchAgent was unloaded after the successful build; no background build
service remains installed. Future remote builds should use this GUI-session
execution path while Nicholas is logged in and the keychain is unlocked.

The diagnostic scripts and evidence remain on the iMac under
`/tmp/den-m5a-signing`: `gui-build.plist`, `gui-build.sh`, `gui-build.log`,
`gui-build.exit`, `install.json` and `launch.json`. Both device operations
reported `outcome: success`. These temporary files are not committed.

The checkout remains `/Users/nicholascaron/Projects/personal/den` on `main`.
The earlier signing and device blockers below are historical, resolved by
this attended setup. The APNs key is a separate prerequisite.

## Original overnight result

M5a is **not implemented or accepted**. This run stopped at signing setup.
There is no Den iOS app to install yet, and the server device endpoints and
APNs sender have not been added.

The brief says: "If signing fails because no team is selectable, stop and say
exactly what is missing." A development team could not be selected through
the available remote controls. The automatic-signing probe fails because its
target has no development team. A saved Apple account entry exists, but this
run could not verify its team membership or obtain a selectable team from
Xcode. This does **not** establish that Nicholas lacks a developer membership.

## Verified on the iMac

- Tailscale ping succeeded, and `ssh imac` identified `Nicholas-Work.local`,
  user `nicholascaron`.
- Xcode is 26.6, build 17F113. XcodeGen 2.45.4 was already installed.
- Created the clean checkout at
  `/Users/nicholascaron/Projects/personal/den`, on `main`, starting at `86c4cc7`.
  All Xcode commands ran on that Mac over SSH.
- `security find-identity -v -p codesigning` reported
  `0 valid identities found`, including after the provisioning attempt.
- A disposable SwiftUI signing probe lives at `/tmp/den-m5a-signing` on the
  iMac. It uses `app.denchat.ios`, automatic signing, iOS 26, Swift 6 and strict
  concurrency. It is a signing diagnostic, not the M5a application, and is not
  committed. Its `build.log` contains the failed provisioning attempt.

The exact build command, run inside that temporary directory, was:

```sh
xcodebuild -project DenSigningProbe.xcodeproj -scheme DenSigningProbe \
  -destination 'generic/platform=iOS' -allowProvisioningUpdates \
  -derivedDataPath /tmp/den-m5a-signing/build build
```

It exited 65 with:

```text
Signing for "DenSigningProbe" requires a development team. Select a development team in the Signing & Capabilities editor.
```

The probe did not pass `DEVELOPMENT_TEAM`, because no verified team ID was
available. This error proves the missing target setting; it does not prove
that a configured team would fail automatic provisioning. Xcode Settings
opened to Apple Accounts, but its account controls exposed no readable team
through accessibility inspection. Project navigation also failed to expose a
usable Team selector remotely. No account credentials were changed or exported.

`xcrun devicectl list devices` twice reported the paired iPhone 17 Pro Max as
`unavailable`, identifier `6013C890-8F56-57F9-B447-5A23057BC488`. No device
installation or launch was possible. This Xcode's `devicectl device` help has
no screenshot subcommand. Mac display capture also failed with
`could not create image from display`; no M5a screenshots are claimed.

## Needs Nicholas

1. Wake and unlock the iMac. In Xcode, open Apple Accounts and confirm that
   the signed-in account shows the intended developer team. Complete any
   account or agreement prompts that Xcode presents.
2. Open `/tmp/den-m5a-signing/DenSigningProbe.xcodeproj`, select the application
   target, then Signing & Capabilities. Select that team with automatic signing
   enabled. The next lane can read the resulting `DEVELOPMENT_TEAM` from this
   disposable project and rerun the command above. If the temporary directory
   has been cleared, create an equivalent disposable SwiftUI signing probe.
3. Connect and unlock the iPhone, complete any trust or Developer Mode prompts,
   and confirm `xcrun devicectl list devices` reports it available.

No wait for Nicholas or repeated phone polling remains running.

## Push and remaining work

Push is **blocked on APNs key**. The expected codexbox file
`~/.config/den/apns.p8` was absent. Nicholas still needs to supply that key,
Key ID and Team ID for `DEN_APNS_KEY_PATH`, `DEN_APNS_KEY_ID` and
`DEN_APNS_TEAM_ID` in `/etc/den/den.env`. This run did not change deployment
configuration. Device registration and APNs delivery code are also still
unimplemented; supplying the key alone will not enable push.

Resume from [the iOS brief](M5A-IOS-BRIEF.md) after the signing gate. Every
functional acceptance item remains open: generated OpenAPI client, native
session and Keychain storage, rooms and DMs, markdown/replies/reactions,
Photos and Files uploads, inline video scrubbing, typing/presence, inbox/search,
shared themes and bundled fonts, settings, offline cache, device endpoints,
notification registration, push delivery and tap navigation. Calls and live
object placeholders also remain to be built as specified in the brief.

After implementation, login, notification permission, Photos selection, any
Face ID prompts and the closed-app push tap require Nicholas's attended check.
Record real device logs and simulator layout screenshots where device capture
is unavailable. Do not treat the signing probe as application acceptance.

Only this notes file is committed by the iOS lane. Other lanes' scripts,
screenshots and design files in the shared checkout are left untouched.
