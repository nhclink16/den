# M5a native iOS text

## September 14 implementation update

The native SwiftUI client now exists in `apps/ios`. It builds with Swift 6 strict
concurrency for iOS 26, installs with automatic signing, and has launched on
Nicholas's iPhone. This replaces the disposable signing probe. **M5a is not yet
fully accepted:** closed-app APNs delivery still needs the server key, and the
remaining device checks below are separate from simulator results.

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
