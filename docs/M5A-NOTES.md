# M5a native iOS text

## September 15 physical-phone update and Offline diagnosis

Nicholas returned to the office and reported Offline in the mobile app. The
paired iPhone was available on iOS 27.0. A fresh pinned XcodeBuildMCP 2.7.0 stdio
session listed both simulators and devices; device inspection confirmed the
installed app was 0.3.0 (1).

Production health returned HTTP 200. Its current appearance contract requires
`light_theme` and `dark_theme`, while build 1 requires the removed `theme` field.
This is a verified incompatibility, not a captured phone-side decoding error.
Appearance participates in the all-or-nothing native refresh; restore and
reconnect classify its decoding failure as Offline. The integrated native
schema matches production's bootstrap response schemas and checked chat/auth
paths. No native source change or rebuild was needed for this update.

Installed the already-built `Den-integrated.xcarchive` application over the
existing app, without uninstalling or clearing data. Its strict code signature
passed and its provisioning profile includes Nicholas's phone. XcodeBuildMCP
reported install and launch success (PID 5495); independent device inspection
confirmed **0.3.0 (2)**. The production-origin chat cache was freshly written at
2026-09-15 17:47:42 UTC, evidence that the updated app read the server after
launch. No chat contents or credentials were copied out. The original Offline
banner and its disappearance have not been visually verified: iPhone Mirroring
requested the Mac login. Nicholas was asked to confirm the banner and dictate
two sentences separated by a pause, then tap the checkmark. He completed the
check and reported: "dictation animation is PERFECT" and "it works great."
This accepts the revised dictation interaction and spoken recognition on his
physical iOS 27 phone. It is user-reported device acceptance, separate from the
automated install, cache, simulator UI, and recorded-audio evidence.

Evidence: `/tmp/den-phone-app-status.json`,
`/tmp/den-phone-app-after-install.json`, `/tmp/den-phone-cache-files.json`,
`/tmp/den-ios-offline-production-schema.json`, and the existing integrated
archive receipt. Binary SHA256 remains
`53478fdb7cd7da79eb2952bad8219dd688c6961a1d5709be730f16bae073ab07`.

TestFlight has not been updated by this installation. A new Aqua-session
keychain check succeeded, but one fresh distribution export still failed with
`No Accounts` and `No signing certificate "iOS Distribution" found`, exit 70.
Evidence is `~/.local/share/den-ios-tools/runs/aqua-p0lwh1uz/`. No repeated export,
credential changes, distribution upload, or public App Store release occurred.
Build 1 should not be treated as compatible with the current server for Andrew's
testing; the updated native build must reach TestFlight first.

## September 14 implementation update

The native SwiftUI client now exists in `apps/ios`. It builds with Swift 6 strict
concurrency for iOS 26, installs with automatic signing, and has launched on
Nicholas's iPhone. This replaces the disposable signing probe. **M5a is not yet
fully accepted:** closed-app APNs delivery still needs the server key, and the
remaining device checks below are separate from simulator results.

### Dictation UX revision, September 14

Nicholas confirmed that build 1 no longer crashes, then supplied a recording
showing the extra Done toolbar, pulsing mic, cramped multiline draft, and text
being replaced after pauses. The recording's audio track is silent; the visible
transcribed complaints and UI were the acceptance reference. His newer request
to follow T3 Code supersedes the original streaming/pulsing UX in the brief.

- Record first into an app-private mono Int16 CAF file; transcribe the completed
  file on-device after Finish. Insert the complete result once, never send it.
  Preserve the selected caret/draft ownership, reject late results after Cancel,
  and delete temporary audio on finish, failure or cancellation. The local legacy
  fallback also reads a completed file and still requires on-device recognition.
- Replace the mic with Cancel, a real input-level waveform and elapsed time, and
  a checkmark. Preparation/transcription stay in that same row. No pulsing mic,
  extra listening line, or keyboard Done toolbar. Motion respects Reduce Motion.
- Keep the existing keyboard and editor mounted; the draft is read-only during
  recording. Expanded/multiline text occupies the full width above the controls.
  The input still supports Return to send, Shift-Return for a newline, and outside
  taps to dismiss the keyboard. Interactive scrolling remains enabled.
- Recording is capped at five minutes; transcription has a 60-second deadline.
  Real background, navigation and call preparation cancel. The permission sheet's
  inactive state does not cancel preparation. Only microphone permission is asked.
  Den still uses installed Apple assets only and is hidden during calls.

This is a native implementation of the interaction in T3 Code commit
[`5ea6439`](https://github.com/pingdotgg/t3code/tree/5ea6439816470288d3f2b6b43635fea41fbbb101/apps/mobile/src/features/voice-input),
not a React Native port. It omits T3's one-off 3D row flip; there is no ornamental
transition. Context7 and the installed iPhoneOS 26.5 SDK were used for the
completed-file SpeechAnalyzer and AVAudioFile APIs. No dependency was added.

Verification so far: all **39 native tests passed**, with the separate opt-in
permission test skipped; the real fresh-permission path is exercised by UI tests.
The recorded-file test checks every one of 240,000 samples, including a three-second
silent interval, meter behavior and file removal. Its first failure exposed a
bad test assumption: AVAudioFile returned 239,616 frames and the last 384 on the
next read. The test now loops over valid frameLength values and still checks all
samples byte-for-byte. It does not weaken the expected audio content.

Deliberately dropping file writes failed that test's length/sample assertions.
Deliberately allowing edits during recording failed the UI test with the changed
draft. Both production files were restored byte-exact, with a receipt retained.
An initial method-only Swift Testing selector executed zero tests and is excluded
from the evidence; the suite-level negative run caught the intended failure.

Actual Apple spoken-file recognition could not be validated here: the iOS 27
simulator reports the analyzer unavailable. A separate signed local Mac probe
reports supported but uninstalled speech assets; Apple's test-only installation
request failed with `Foundation._GenericObjCError.nilError`. No model download
code was added to Den. The unexecutable spoken-file test is retained only in the
private evidence directory, not presented as passing coverage or committed as an
untested test. Nicholas's next TestFlight check must include speaking across a
pause and tapping Finish; simulator PCM/UI proof is not physical recognition proof.

Evidence is under `/tmp/den-ios-dictation-ux/`: `unit-final.xcresult`,
`ui-green-1/first-run.xcresult`, `ui-readonly-red/first-run.xcresult`,
`file-write-red-suite.xcresult`, `mutations-restored.json`, and the visual captures.
A full text-flow run caught a real dismissal gap after removing Done: an
interactive scroll gesture did not reliably hide the custom editor keyboard.
The outside-tap target now covers the whole timeline, including its blank area.
The test uses that visible tap and retains its keyboard-hidden, tab-hittable and
tab-selected assertions. The initial failed run is `ui-final/first-run.xcresult`.
File transcription also finalizes through the explicit last sample time returned
by `analyzeSequence(from:)`, following Apple's completed-file API contract; it
does not wait for a future live stream to close.

Final iOS 27 run: **42 passed, 0 failed, 1 skipped**, including all three real
text UI tests and a new privacy reset. `all-final.xcresult` is authoritative.
The skipped opt-in unit permission test is covered by the fresh system-prompt UI
flow. Visual captures: [recording](shots/dictation-ios-recording.png) and
[multiline draft](shots/dictation-ios-multiline.png). The project/scheme/package-pin
and portable fixture checks passed via `ci_post_clone.sh`.

The signed Release archive is **0.3.0 (2)** in `Den-final.xcarchive`;
`Archive-final.xcresult` passed and its signature/privacy manifest were verified.
An earlier preview archive predates the final keyboard/finalization changes and
is not the delivery artifact. Distribution export then failed with **No Accounts**
and **No signing certificate "iOS Distribution" found**, through the same Aqua
helper/options that successfully uploaded build 1. App Store Connect's browser
session is still signed in, but that does not prove Xcode's account credentials.
Evidence: `archive-verification.json`, `export-helper.log`, Aqua runs
`aqua-vih01q7y` (archive) and `aqua-om6xsmej` (failed export). Build 2 has not yet
been uploaded; no new TestFlight availability or physical recognition is claimed.
The canceled phone watcher remains off, and no Xcode Cloud run was started.

### Concurrent appearance-contract integration

Before the dictation push, main advanced to `6893973` with the M10 appearance
schema. That removed `Appearance.theme`, breaking native compilation; the first
42-test receipt above predates that integration. Native now resolves `light_theme`
or `dark_theme` after Mode and edits only the currently displayed half. The
other half, custom themes, contrast and background values are preserved on save.
This is compatibility work required by the merged contract, not a claim to have
implemented the new background editor or contrast rendering on iOS. The portable
Cloud fixture source bundle was refreshed and its server rebuilt locally.

The new resolution test was deliberately broken by always choosing the light
family. It failed both dark-mode cases (`appearance-red.xcresult`); the production
file was restored byte-exact before the final current-main run. The existing
text UI assertions are retained; their fixture now matches the merged server.

At the integration check, production still advertised the old required `theme`
field. The new schema/build therefore needs coordination with the M10 deployment;
Fable was notified. The pre-M10 signed archive remains available for the old
contract, but neither archive has been distributed as build 2. Xcode's actual
`DVTDeveloperAccountManagerAppleIDLists` preference contains an empty
`IDE.Identifiers.Prod` array; its provisioning-team metadata remains cached. Aqua
can access one development identity, but no distribution identity. The browser
App Store Connect session remains signed in. No account/credential preference
was edited or reconstructed, no signing credential was exported, and no extra
Cloud run or public release was started.

Fable subsequently identified a locked login keychain for non-console sessions:
`security show-keychain-info` reports `User interaction is not allowed`, while a
valid development identity and cached profiles remain present. The next signing
step is to retry after Nicholas unlocks the iMac/login keychain, not to request
another browser sign-in. Den has not asked for, stored, or reconstructed a keychain
password. The `No Accounts` export result alone does not establish an account fault.

The first current-main UI run failed because Apple Swift OpenAPI Generator 1.13.1
skipped `Appearance.background`, whose schema used `oneOf` with a standalone null
branch. Its strict decoder then rejected even `background: null` during bootstrap.
The newer profile schema had the same unsupported representation for `User.status`
and `ProfilePatch.status`. Short branch `ios-background-schema`, merged with a note
in `5afff96`, derives these object fields from the original shared types and changes
only their OpenAPI nullability representation to `type: [object, null]`. There is
no runtime JSON change, duplicated client contract, new migration, or deployment.
Fable was notified before the merge. The first inline-only annotation was insufficient
and was replaced before the commit; it is not presented as successful evidence.

All **40 server API tests passed**, including appearance and profile behaviors.
The native snapshot comes from that actual branch server binary. Its SHA256 is
`80cd028cf1364d434aceaf6b761d93bbe91ffb7cea8af3b83d0b5732467a978b`.
Apple generation now reports zero diagnostics and includes the nullable fields.
The portable fixture was regenerated from the merged Rust source, and the Cloud
checkout/project/package checks pass. The text UI test now seeds a non-null account
background and contrast 110, then checks both survive changes to each theme half
and Mode. Earlier theme/mode expectations were adapted to the new field contract,
not removed or weakened. `current-main-final.xcresult` is the failed integration
run; the post-fix result is recorded separately below.

Final integrated iOS 27 result: **43 passed, 0 failed, 1 skipped** in
`integrated-final.xcresult` (302.4 seconds). All three real text UI tests passed:
fresh privacy/first PCM 42.485 seconds, full text/session/appearance 164.080 seconds,
and search/tab navigation 28.893 seconds. The unit permission probe is the one
skip; the real reset-privacy permission UI test ran and passed. The final generation
and fixture evidence are `final-schema-diagnostics.yaml`,
`schema-server-tests-latest.log`, and `ci-post-clone-integrated.log`.
The linked recording and multiline screenshots were refreshed from this final run
and visually inspected. They show the compact recording row, no Done toolbar, and
the full-width draft above its controls.

A new read-only keychain-info check through Aqua succeeded (`aqua-wuv7m27q`, exit 0),
so distribution export was retried once through that same desktop session. It still
failed with **No Accounts** and **No signing certificate "iOS Distribution" found**
(`aqua-wq8tflq7`, exit 70; `export-keychain-retry.log`). This shows that reading
keychain settings does not prove Xcode can obtain distribution signing credentials.
There will be no repeated export attempts without a change in signing access.
The production contract recheck still has the old `theme` field and pre-profile
User. Current-main distribution must be coordinated with the M10 server rollout;
the already verified pre-M10 `Den-final.xcarchive` matches the presently live API.
No upload, server deployment, public release, or Cloud run occurred in this retry.

The final current-main **0.3.0 (2)** Release archive also succeeded through Aqua
(`aqua-ze0q4wv0`, exit 0). `Den-integrated.xcarchive` contains the verified source
in `140f211`; `Archive-integrated.xcresult` and
`archive-integrated-verification.json` record the result. Deep/strict code-signature
verification passed and the app privacy manifest equals source. This is a normal
development-signed archive pending distribution re-signing, not a TestFlight IPA.
The earlier `Den-current-main.xcarchive` still has the broken nullable schema and
must not be distributed. The disposable local native fixture was stopped with its
identity-checking cleanup command after all tests passed; the simulator app was
stopped and the merged schema worktree removed. Test products, archives and logs
remain in the evidence directories. The phone watcher remains off.

### Remote TestFlight delivery, September 14

The first **internal-only TestFlight** upload, **0.3.0 (1)**, succeeded at
20:32 EDT. It contains the `ada6aa9` AnalyzerInput/first-permission dictation
repair and a first-party privacy manifest for Den's own UserDefaults storage
(`CA92.1`). XcodeGen adds that manifest to the app Resources phase. No data-use
label, core/server contract, Cloud workflow or public App Store submission was
changed. The separate missing framework dSYMs reported during upload are
nonfatal; Den's own dSYM is included.

Verification used the actual Release archive and exported IPA, not just source
settings: archive/export succeeded, the bundled manifest equals source,
`codesign --verify --deep --strict` passed, bundle/version is
`app.denchat.ios` / `0.3.0 (1)`, and re-signed entitlements have production APNs,
beta reports and no debugger attachment. The exported options retain
`testFlightInternalTestingOnly: true`. `ci_post_clone.sh` passed the generated
project, shared scheme, package-pin and fixture-source checks. This resource-only
packaging change adds no test; it does not replace the dictation regression
results below.

Evidence: `/tmp/den-ios-testflight-ada6aa9/` holds the final
`Den-privacy.xcarchive`, `Archive-privacy.xcresult`, `export/Den.ipa`,
`archive-privacy-verification.json`, `export-verification.json`, and
`upload-helper.log`. Aqua runs: `aqua-7i8j0fqy` (archive), `aqua-p8c1ty2o`
(distribution export), `aqua-nurjaaoa` (upload), all exit 0. Apple's uploader
reported **Uploaded package is processing** and **Upload succeeded**.

At **20:56 EDT**, after Nicholas restored browser sign-in, the actual App Store
Connect UI showed upload **Complete**, build **Testing**, and Nicholas Caron
**Invited** after a fresh reload. The new internal group `Nicholas` has exactly
one tester and build `0.3.0 (1)`. Automatic distribution is off: later releases
are added deliberately, without automatically exposing every uploaded build.
No external group or App Store release was created. Receipt:
`/tmp/den-ios-testflight-ada6aa9/testflight-ready.json`.

The remaining Missing Compliance gate was resolved with the build's actual
encryption classification: standard algorithms outside Apple's OS. The shipped
LiveKitWebRTC binary implements DTLS/SRTP cipher selection and includes BoringSSL,
libsrtp, AES-GCM and SRTP symbols/strings. Den's OS-backed URLSession API traffic
does not make the complete app OS-encryption-only. France distribution was
answered No within the authorized Nicholas-only internal testing scope. This
does not decide future public or French distribution; revisit it if scope changes.
No exemption flag was added to Info.plist. Apple's table distinguishes bundled
standard algorithms from OS-only encryption and scopes French documentation to
French App Store distribution.
[Apple encryption documentation requirements](https://developer.apple.com/help/app-store-connect/reference/app-information/export-compliance-documentation-for-encryption/)

No new phone install, physical spoken dictation acceptance or real push delivery
is claimed. The canceled phone watcher was not restarted. This local route used
no Xcode Cloud compute and does not require Nicholas's phone near the iMac. Setup notes are in
[the iOS README](../apps/ios/README.md#remote-iteration-with-testflight).

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

## Dictation

### September 14 physical crash follow-up

The first permission-callback repair (`e188e4e`) did not resolve physical
dictation. A fresh device report, `Den-2026-09-14-162122.ips`, records
**EXC_BREAKPOINT / SIGTRAP on thread 15**, queue
`RealtimeMessenger.mServiceQueue`. Its stack is
`AnalyzerInput.data(from:)` → `AnalyzerInput.init(buffer:)` → the Den audio tap.
The report's Den image UUID matches the signed binary installed by that repair.
This is the first microphone buffer, not the earlier speech-authorization
callback isolation failure. Private local evidence is under
`/tmp/den-ios-dictation-buffer-crash/`.

Installed iOS **27.0 (24A434)** beside 26.5 using the official platform download;
Xcode remains **26.6 (17F113)**. The exact same compiled PCM regression passed
all nine input cases on 26.5, then trapped on the first Float32 mono case on 27:
**“Audio sample data must be 16-bit signed integers.”** The failed iOS 27 run is
`test_sim_2026-09-14T20-38-54-000Z_pid96882_62a49a63.xcresult` (four other tests
passed, one optional permission integration skipped). Initial simulator startup
was slow, but XCTest did reach and execute this failing case.

After Int16 conversion, the stereo case exposed a second framework precondition:
**“Multi-channel audio is not supported.”** That failure is retained in
`test_sim_2026-09-14T20-53-07-545Z_pid7099_2a953eff.xcresult`. The repair transports
an owned raw PCM frame out of the tap, chooses only module-compatible **mono
Int16**, downmixes/converts, and constructs `AnalyzerInput` only afterward.
No Speech wrapper is used to carry unconverted input into AVAudioConverter.
Empty callbacks are ignored. Copying reads the buffer's valid `frameLength`, not
the unused capacity exposed by `mutableAudioBufferList`.

The new nine-case test was adapted, not left unchanged: it now includes unused
capacity, source-buffer reuse, mono/stereo, planar/interleaved and Float32/Int16.
An initial stereo assertion assumed arithmetic averaging; iOS 27 returned an
equal-power result for interleaved Int16. Apple's `AudioConverter.h` documents
a layout-dependent default mix map, not fixed gains. The revised assertion
checks isolated left/right contributions, silence and additivity; mono amplitude,
owned bytes, valid-frame counts and the real Speech constructor remain checked.
No custom DSP was added to force a test's undocumented gain assumption.
The final PCM assertions were deliberately challenged with downmixing disabled:
`test_sim_2026-09-14T21-23-17-963Z_pid25524_8c5721a7.xcresult` fails the new
buffer test while four other tests pass. The production source was restored
byte-for-byte before final verification.

The lead audit also found:

- Apple's [permission guide](https://developer.apple.com/documentation/speech/asking-permission-to-use-speech-recognition)
  scopes speech-recognition authorization to `SFSpeechRecognizer`, not the
  on-device `SpeechAnalyzer` transcriber. The user's newer one-microphone-prompt
  instruction supersedes the original brief's two-prompt requirement.
- `DenApp` already cancels on real `.background`, not `.inactive`; the audio
  observers likewise use `didEnterBackground`, interruptions and engine reset.
  **The first-run test found a separate focus race:** the editor lost focus
  before UIApplication changed from active to inactive. Composer treated this
  as Stop and cancelled preparation. The actual trace then reported
  `microphone=true, current=false`. Preparation now ignores that incidental
  focus loss. Explicit Stop, keyboard Done/Escape, People, timeline taps, navigation,
  call preparation and real background still stop/cancel. Microphone completion
  can also arrive while inactive; startup waits briefly for active before audio.
- T3 Code's record-then-transcribe implementation was examined at commit
  `549d182aaadbf0e1e195d1a8cdc1805200e6751e`. File transcription is a viable
  different interaction, but removes live partials and adds recording-file
  lifecycle work. The reproduced PCM-format failure supports repairing the
  live conversion boundary instead of adding that fallback for this bug.
- Authorized iPhone Mirroring stopped at its own Mac-login password gate. No
  password was entered, no unlock was requested, and no security setting changed.

With those changes, the iOS 27 first-run test passed after `reset all`: exactly
one mic prompt and one initial tap, preserved draft plus the processed-PCM marker,
Listening, Stop/edit, and unchanged server message IDs (no auto-send). The same
run passed all nine buffer cases: `test_sim_2026-09-14T21-20-23-473Z_pid24096_f58ef4d2.xcresult`.
It contained temporary simulator-only lifecycle tracing, which was then removed.
The final trace-free full iOS 27 suite passed **41 tests, zero failures**, with
one old opt-in permission integration skipped:
`test_sim_2026-09-14T21-29-21-783Z_pid27680_4eae7601.xcresult`.
The new first-use test was not skipped; it passed in 33.335 seconds.
Synthetic input is not a physical spoken-dictation claim.
Reintroducing the old focus-loss cancellation deliberately failed that same UI
test after Allow: the composer retained `Keep this draft` but never received the
PCM-driven transcript. The source was restored byte-for-byte and all final native
source hashes matched the passing trace-free build before subsequent checks.
Failure result: `test_sim_2026-09-14T21-34-03-893Z_pid29854_97b9b981.xcresult`.

The permanent local runner then passed that same fresh-permission UI test on
iOS **26.5**, using the exact trace-free test products from the iOS 27 run:
`/tmp/den-ios-dictation-buffer-crash/final-26-first-run-fixed-runner/first-run.xcresult`.
Two earlier runner invocations did not reach XCTest: a cold-boot privacy reset
timed out, and the next invocation exposed Xcode rejecting `-test-iterations 1`.
The simulator was opened and the invalid repetition argument removed. The final
runner used the normal single execution with no automatic retries, and reset
privacy successfully before the passing test. Cloud execution remains unverified.

The signed device build passed in
`~/.local/share/den-ios-tools/runs/aqua-xcf5u5yh/` (exit 0), and
`codesign --verify --deep --strict` passed. The new Den image UUID is
`1145C439-FD18-3273-B06F-400518A6E942`; its sources match the passing trace-free
test products. The app is at
`/tmp/den-ios-device/Build/Products/Debug-iphoneos/Den.app`.

**Installation is blocked, not complete.** The native device list reports
Nicholas's iOS 27 phone disconnected/unavailable. One installation attempt failed
with CoreDevice error **1011**, unable to locate the requested device. No launch
or physical spoken/offline retest is claimed, and no unlock was requested.
Evidence: `/tmp/den-ios-dictation-buffer-crash/device-{list-after-build,install}.json`.
Install this signed app when the phone is available, then have Nicholas speak;
the earlier installed build is not this fix. The isolated local fixture was
stopped with its ownership-checked helper and its private data removed. Both
owned QA simulators were shut down after testing; the iOS 27 runtime is retained.

Test harness corrections are explicit: Python and Foundation canonicalize `/tmp`
aliases differently, so both receipt paths now use the same comparison. iOS 27
permission sheets are matched by visible Den-specific titles, not assumed Alert
roles. Underlying-app phase assertions were adapted to same-tap PCM delivery and
Stop/Listening after dismissal; querying behind a system sheet was unreliable.
Failures return before dependent UI actions and deny only a remaining matching
Den permission sheet during cleanup. No permission is pregranted or implicitly
allowed by the test. The host resets privacy before every invocation and supplies
a fresh single-use, exact-device receipt. This test is part of the regular UI
suite, not an opt-in skipped permission check.

The native composer now has an on-device dictation control immediately before
Send, with the same 44-point plain-button treatment as Attach. Listening uses
the theme accent, a 1.2-second ease pulse (off under Reduce Motion), and a mono
`listening` label in place of typing status. Stop leaves text editable; recognition
never sends a message. Escape, outside taps, manual edits, navigation, background,
session changes, and call preparation stop or cancel the appropriate session.
Settings > Voice contains the platform language picker and a persisted
Punctuation preference where the selected engine supports it.

### Implementation decisions

- Prefer iOS 26 `SpeechAnalyzer` / `SpeechTranscriber` with already-installed
  assets. No model download or reservation API is called. If unavailable, use
  `SFSpeechRecognizer` only with both `supportsOnDeviceRecognition` and
  `requiresOnDeviceRecognition = true` and already-granted legacy authorization.
  Hide the button when neither eligible local engine supports the selected
  language. First use requests only the microphone; SpeechAnalyzer does not
  require legacy speech-service authorization. No new legacy speech prompt is
  introduced as a fallback. Ownership is rechecked before hardware startup.
- Xcode **26.6 (17F113)**, installed iPhoneOS **26.5** SDK, Swift **6.3.3**,
  Swift 6 strict concurrency, iOS 26 minimum. Context7 had no matching Apple
  Speech entry; exact APIs were checked against the installed SDK and Apple's
  [SpeechAnalyzer documentation](https://developer.apple.com/documentation/speech/speechanalyzer),
  [WWDC25 sample](https://developer.apple.com/videos/play/wwdc2025/277/), and
  [on-device request documentation](https://developer.apple.com/documentation/speech/sfspeechrecognitionrequest/requiresondevicerecognition).
  No dependency was added. SpeechTranscriber has no punctuation switch in this
  SDK, so that setting is shown only for the legacy local engine.
- Full-session transcript revisions replace only the dictation-owned span at
  the UTF-16 caret. Preserve surrounding text, including selected text: insert
  before a selection because selecting an existing draft is not a deletion
  request. Respect existing whitespace and punctuation, and remove owned
  separators for an empty transcript. Reject manual changes, including Unicode
  edits that look identical but change UTF-16 offsets.
- Capability discovery is shared and cached, including an empty result, across
  room recreation and new sessions. Settings explicitly refreshes it. The initial
  platform scan remains on MainActor; no measured background-thread performance
  improvement is claimed. Each selected driver still rechecks local readiness.
- Stop closes the tap, engine, and owned audio session synchronously, then
  permits at most two seconds for final words. Manual edits and context changes
  immediately invalidate late callbacks. Queued startup tasks are cancelled and
  carry an invalidation revision, including idle Stop and pre-permission races.
  Asynchronous cleanup never deactivates a later call's audio session.
- **Dictation is hidden throughout calls.** LiveKit 2.17.0 exposes a public PCM
  observer, but Den's quiet-join, mute, activation and route gates have not been
  validated with a speech consumer on the phone. This is a conservative scope
  decision, not a claim that microphone reuse was tested and failed. CallKit
  preparation synchronously revokes dictation and restores its audio category
  before a system transaction/report. Even a malformed mandatory incoming report
  holds a dictation-exclusion guard until its report fails or is explicitly ended.

### Verification

Nine focused tests passed at baseline. Deliberate changes then made **all nine
fail**: stale volatile text, network-capable fallback, old-generation callbacks,
capture left open during finalization, cumulative insertion, wrong UTF-16 caret,
external overwrite, unwanted separators, and accepted invalid selections.
All four mutated source files were restored byte-for-byte before the full suite.
The request/privacy test constructs a request but starts no recognizer; controller
tests use fake capture drivers. No ambient microphone capture was used for proof.

Swift 6.3.3's test macros generated immutable receivers for mutating insertion
calls. Those calls now execute into local variables before `#require`/`#expect`;
all original inputs and assertions were preserved. Static review also found and
fixed queued-start and malformed-report exclusion races; those observations are
not presented as physical CallKit test results.

Evidence: `/tmp/den-ios-dictation-proof/mutation-restored.json`; baseline result
`test_sim_2026-09-14T19-16-10-583Z_pid86141_5647252d.xcresult`; deliberate red result
`test_sim_2026-09-14T19-19-29-738Z_pid86141_33befea8.xcresult`.
After adding discovery caching, all nine passed again in
`test_sim_2026-09-14T19-34-05-006Z_pid53025_5a0975d6.xcresult`. Removing the cache
guard then failed the existing cancellation/discovery test's count assertions
(three other engine tests passed), in
`test_sim_2026-09-14T19-37-57-870Z_pid55826_30bf3d75.xcresult`. The latest controller
was restored byte-exact; receipt: `/tmp/den-ios-dictation-proof/cache-mutation-restored.json`.
An earlier method-level CLI selector matched zero tests and is excluded from
verification; suite-level selection supplied the real negative proof.
The final iPhone run passed **39 tests, zero failures and zero skips** in 252
seconds, including both existing text/navigation UI tests. Result:
`test_sim_2026-09-14T19-39-08-816Z_pid56724_b5a78ab9.xcresult`.
The iPad reused those exact build products and passed both UI tests with zero
failures or skips in 222 seconds. Result:
`test_sim_2026-09-14T19-44-55-607Z_pid61587_d13ec88a.xcresult`.
Receipts are `/tmp/den-ios-dictation-proof/final-{iphone,ipad}.json`.

The running iPhone simulator's Voice settings showed the language picker,
Punctuation switch and local-audio/call limitation copy without clipping.
[Voice settings screenshot](shots/dictation-simulator-voice.jpg) is a simulator
layout check only; it does not show physical dictation or listening. Resource
sync, generated-project/lock checks and the local post-clone hook passed.

The same source built for the physical iPhone through the documented Aqua
signing helper, **BUILD SUCCEEDED, exit 0**, in
`~/.local/share/den-ios-tools/runs/aqua-q4s9g5pz`. The pinned CLI installed
Den 0.3.0 build 1 on Nicholas's connected iOS 27.0 phone and launched process
**1956** successfully. Receipts:
`/tmp/den-ios-dictation-proof/device-{install,launch}.json`.
No speech signing entitlement or App ID change was needed. Existing microphone
and new speech usage descriptions remain in the generated Info.plist.

The first full run was incomplete, not green: the existing Inbox test lost its
destination because Den crashed at `ConversationView.swift:91`. The actual crash
stack reports an array subscript assertion. Lazy `ForEach` rows held an enumerated
snapshot but fetched their previous message from a changing live array. The
timeline now captures one immutable array for both operations. The navigation
assertions were not changed. Minimal crash evidence is
`/tmp/den-ios-dictation-proof/timeline-crash.json`; the original system report is
`~/Library/Logs/DiagnosticReports/Den-2026-09-14-152502.ips`.

Separately, local disk exhaustion interrupted the tool connection and left that
run's result bundle incomplete. Only verified completed lane-owned build/cache
products were removed; source, logs, result bundles and current build outputs were
preserved. The pinned XcodeBuildMCP **2.7.0 CLI** provides the same native tools
while this session's MCP stdio transport is closed. This remains local iMac work,
not Xcode Cloud usage.

After the final tests and layout check, the official identity-checking fixture
stop command removed the old isolated text server and its disposable data. Its
exact SSH forward and private local credential file were also removed. Receipt:
`/tmp/den-ios-text-fixture-cleanup.json`. The separate call fixture had already
been cleaned up; neither cleanup touched production conversations.

**Physical acceptance remains open:** dictate a sentence into the actual iPhone
composer, repeat offline with airplane mode and Wi-Fi off, check caret/manual-edit
behavior, and capture the listening screen as `docs/shots/dictation-iphone.png`.
No simulator image is substituted for that requested phone screenshot. Source
guards and fake-driver tests are not proof of real offline speech recognition.

### September 14 physical permission crash

Nicholas's first physical dictation attempt exposed a real crash missed by the
fake-driver tests. The phone report `Den-2026-09-14-160333.ips`, captured at
16:03:32 Eastern, traps in `_dispatch_assert_queue_fail` and Swift's isolation
check. The app frame is the speech-authorization completion inside
`DictationPlatform.authorize(isCurrent:)`, called by TCC on
`com.apple.root.default-qos`. Dictation had not started audio capture.
Redacted evidence: `/tmp/den-ios-dictation-crash/crash-receipt.json`.

The iPhoneOS 26.5 headers explicitly allow both microphone and speech permission
completions off the main thread, but these older Objective-C block declarations
are not marked Sendable. Our closures inherited MainActor isolation from their
enclosing type. This is the callback mismatch described in Swift's
[incremental adoption guide](https://www.swift.org/migration/documentation/swift-6-concurrency-migration-guide/incrementaladoption/#Unmarked-Sendable-Closures).
The earlier state tests injected an authorization result and never exercised
these actual completion closures. The regression needs that callback boundary,
not another fake-driver start test.

Both real completion literals now explicitly use `@Sendable` and only resume
their checked continuation. The async function resumes on MainActor for its
existing ownership and denial checks; those checks were not changed.

The retained regression calls the real platform authorization method. Enable it
by passing `DEN_TEST_DICTATION_PERMISSIONS=1` in XcodeBuildMCP's `testRunnerEnv`
and selecting `DenTests/DictationEngineTests`. It may show system permission
prompts on a fresh test device; it starts no microphone capture or recognizer.
It is opt-in so ordinary unattended CI does not hang waiting for those prompts.
The QA simulator's microphone permission was granted with simctl; its actual
speech permission prompt was allowed through the observed accessibility button.

Before the fix, this test **crashed with signal trap**, while the four existing
engine tests passed, with no skips. Result:
`test_sim_2026-09-14T20-11-14-760Z_pid75326_fd06755f.xcresult`, receipt
`/tmp/den-ios-dictation-crash/actual-permission-red.json`.
An earlier injected Swift-registrar experiment passed before the fix because it
changed the Objective-C callback conversion. That seam and its two misleading
tests were removed; their pass is explicitly excluded from regression proof.

After the two-line callback fix, the same actual-API regression and all existing
engine/insertion tests passed: **10 passed, zero failed, zero skipped** in 28
seconds. Result:
`test_sim_2026-09-14T20-13-57-981Z_pid76494_26492900.xcresult`, receipt
`/tmp/den-ios-dictation-crash/actual-permission-green.json`.
No existing assertion was weakened, no privacy request was bypassed in production,
and no audio was captured for either run.

The corrected app built and signed successfully through the Aqua helper,
**exit 0**, receipt `~/.local/share/den-ios-tools/runs/aqua-qjet5p0a`. It was
installed on Nicholas's connected iOS 27.0 phone and launched as process **2126**.
Receipts: `/tmp/den-ios-dictation-crash/device-{install,launch}.json`.
This verifies the crash fix's regression and delivery, not a new spoken-sentence
or offline-recognition pass; Nicholas's actual dictation retry remains needed.

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
