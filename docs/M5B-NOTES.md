# M5b native calls — implementation and verification

**2026-09-14. Native implementation and independent local checks are complete.**
The signed app is installed and launched on Nicholas's iPhone. The iPhone
simulator passed 30 tests, and the iPad passed both full UI flows. Direct native
media and quick reconnect passed against two synthetic peers. This is not full
physical calling acceptance: CallKit-integrated media, PiP, routes, and Apple
push still need the checks below. Cloud preparation is checked locally but has
not run in Cloud. See [M5a notes](M5A-NOTES.md) for earlier text/media acceptance;
those results do not prove calling.

## Current result

| Area | Status and evidence boundary |
| --- | --- |
| Server invitation lifecycle and VoIP transport | **Verified by server lane; deployed.** See [server notes](M5B-SERVER-NOTES.md). Apple delivery remains unconfigured. |
| Final generated native API snapshot | **Verified locally.** SHA-256 matches the server's final artifact below. |
| Native tests | **30/30 iPhone tests and the full 2/2 iPad UI suite passed**, with zero failures/skips. The iPhone run includes all 28 unit tests. All new VoIP and search-navigation tests failed their intended mutants first and passed after exact restoration. |
| Signed physical app | **Built, installed, and launched.** Den 0.3.0 (1), bundle `app.denchat.ios`, on the connected iOS 27 iPhone. This is not an actual-app call/media acceptance result. |
| CallKit/audio startup | **Matched physical iOS 27 probes passed real activation/deactivation**, including a legitimately pending call. The iOS 26.5 simulator pending-call probe fails with End 55; its surviving connected control never activates audio. Actual Den phone/media acceptance remains pending. |
| Native two-peer audio/video | **Direct native media and quick reconnect passed.** Both cameras and the screen share decoded; other-account audio produced nonzero PCM; own microphone stayed excluded; leaving preserved the exact peer connections. This probe manually activated audio, not CallKit. |
| Native PiP | **Implemented; isolated compile passed.** Actual start/restore/background behavior on the iPhone is not verified. |
| Native outgoing screen broadcast | **Not implemented.** No ReplayKit Broadcast Upload Extension or app-group setup has been added. Receiving remote shares is a separate implemented path. |
| Production APNs/PushKit | **Blocked on configuration.** Production reports push delivery disabled; closed/locked-phone ringing has not passed. |
| Xcode Cloud | **Prepared and locally checked; zero Cloud runs.** Sign-in, team, and **denchat** TestFlight record **6811985501** are verified. Cloud/source setup and used/remaining hours remain unverified. |

## Server and schema baseline

The native checkout reached **`8e9d2e6`**, which records the completed M5b
deployment/public smoke evidence. The [deployment receipt](M5B-SERVER-NOTES.md#production)
identifies the actual released main revision as `e596399`, containing server
implementation `5f0d2b6` merged through `5303d3a`. This distinction avoids treating
the later notes commit as a different deployed binary. The shared checkout has
since pulled **`8b2adfc`**, which adds unrelated web dictation work; the deployed
M5b server revision above is unchanged.

The server lane reports 48 passing workspace tests, denied-access and race
coverage, restart/expiry/join-grace checks, HTTP/2 APNs transport checks, and
isolated plus public media smokes. These are server/client smoke results, not
native iPhone CallKit or audible-media acceptance.

Final pretty-printed OpenAPI artifact, also used by
`apps/ios/Packages/DenAPI/Sources/DenAPI/openapi.json`:

```text
c7d2f818ec3bf4f54d11dfd1008a5512d2717eceb95fd992f6930bc4a1b94d84
```

The server's production receipt records parsed JSON equality with that artifact;
the compact HTTP body has a different byte hash, documented in the server notes.
The provisional earlier schema hash is not the native baseline.

## Native implementation present

- **LiveKit 2.17.0 media session:** hangout/DM join and leave, microphone mute,
  output mute, camera on/off and flip, available input/speaker route choices,
  publication/subscription reconciliation, and reconnect/disconnect state.
- **Call UI:** a persistent dock keeps chat available while a sheet displays
  participants, camera tiles, every remote screen-share tile, call controls,
  status, and errors. Dismissing the sheet does not leave the call.
- **Same-account devices:** participant presentation groups connection identities
  by account. Additional devices join with microphone and sound off. When sound
  is enabled, the same account's remote microphone remains suppressed while its
  screen-share audio is eligible. Leaving is connection-scoped, not an
  account-wide disconnect.
- **CallKit controller:** outgoing/start/answer/mute/end actions, stable system
  IDs per invitation, one-shot action completion, timeout/reset cleanup, and an
  audio-activation owner that survives until its matching deactivation.
- **Authenticated invitation signaling:** canonical state events and foreground/
  reconnect reconciliation; accept, decline, cancel, and caller-only end use the
  invitation generation. Another group recipient's acceptance does not cancel
  this device's unanswered ringing deadline.
- **PushKit:** separate VoIP token registration with purpose, environment, and
  installation identity; ordered token rotation/invalidation/logout cleanup;
  CallKit reporting before credential restoration or network reconciliation.
  Ticket redemption uses the selected canonical origin without an account
  bearer, then falls back to authenticated reconciliation when appropriate.
- **PiP:** a stable dock thumbnail supplies the video-call content source; a
  separate LiveKit sample-buffer renderer fills the PiP controller. The first
  subscribed, unmuted remote video is selected. Restoration opens the call
  sheet and acknowledges its appearance. End/disconnect/session invalidation
  stops PiP, clears its content source, and releases video references.

These descriptions were checked against the native source. They describe
implementation, not successful execution of every behavior on a phone.

## Decisions and rationale

1. **CallKit owns audio activation.** Prepare `.playAndRecord` / `.voiceChat`
   with Bluetooth HFP before creating the provider, and configure again before
   fulfilling start/answer. Do not call `setActive` from the media layer. The
   early preparation follows the observed immediate-end probe failure. Matched
   physical probes now deliver activation/deactivation while the tested
   simulator's pending-call path fails independently of Den or LiveKit. Do not
   ship a false connected report, manual activation, or a simulator bypass to
   conceal that difference. The audio lease's tested policy keeps an old call's
   deactivation from being assigned to its successor.
2. **Use generated contracts and one canonical state event.** Native consumes
   `call_invitation_state`, not duplicate legacy invitation events. Account and
   request generations prevent late responses from restoring a previous session
   or changing a replacement call.
3. **Invitation end is not a global media disconnect.** Accepted/connected peers
   stay connected when invitation signaling ends. A failed recipient join does
   not issue caller-only global end; the server owns abandoned-answer cleanup.
4. **Keep the normal camera background pause.** No private camera API or changed
   LiveKit background-capture setting is used. Remote-video PiP does not prove
   background local-camera capture, locked-phone video, or interruption recovery.
5. **Do not substitute an in-app view for outgoing screen broadcasting.** The
   ReplayKit extension, app group, system consent, and cross-app/audio checks are
   still missing. No native outgoing share is presented as complete.
6. **Keep Cloud reproducible and isolated.** Commit discoverable XcodeGen output
   and package locks; compare a temporary regenerated project instead of changing
   Cloud's discovered project. Each test worker builds an exact bundled Rust
   source snapshot and creates its own loopback fixture. No production accounts,
   private credentials, or live media fixture are used by that bootstrap.
7. **Keep Search's field inside its dedicated screen.** On the tested iPad,
   leaving SwiftUI's system search presentation opened the result but lost all
   top-level tabs. Ordinary room navigation passed the same tab checks. Search
   dismissal, toolbar preferences, search-field placement, a root-owned search
   tab, and removing the call dock did not fix it. An inline native TextField
   with Search submission and a clear button passed. This preserves server
   search and its scope picker without relying on a timed dismissal workaround.

## Verification receipts to retain/finalize

### Native checks

- Final signed build: `~/.local/share/den-ios-tools/runs/aqua-ug4ylok0`,
  **BUILD SUCCEEDED / exit 0**. The build emitted two device-version discovery
  warnings about an empty build number; installation and launch succeeded.
  The built plist verifies **0.3.0 (1)**, iOS 26 minimum, sandbox APNs, and
  remote-notification/audio/VoIP background modes. MCP installed that exact
  bundle on device `6013C890-8F56-57F9-B447-5A23057BC488` and launched
  `app.denchat.ios` as process **1751**. Receipts:
  `/tmp/den-ios-m5b-device-proof.json` and
  `/tmp/den-ios-m5b-device-mcp-proof.json`. The diagnostic CallKit probe is not
  the installed Den app.

- Earlier full `DenTests` result: **24 passed, zero failed/skipped**:
  `~/Library/Developer/XcodeBuildMCP/workspaces/den-65f821c5653d/result-bundles/test_sim_2026-09-14T16-28-52-915Z_pid99391_432dbd87.xcresult`.
  Superseded for unit-suite count by the later 28-test baseline below.
- Four generated-client transport tests were mutation-proved: anonymous
  same-origin ticket redemption, rejecting mismatched invitation IDs, expired
  ticket fallback, and delayed-response/session-generation isolation. Receipt:
  `/tmp/den-ios-call-api-proof/README.md`. Its notification request-stream race
  was fixed by reading until EOF rather than treating `hasBytesAvailable=false`
  as EOF; no assertions were weakened.
- Alert registration's purpose/environment/stable-installation contract was
  also checked with a deliberate mutant and restored green. Receipt:
  `/tmp/den-ios-alert-contract-proof/README.md`. These transport stubs do not
  prove delivery through Apple.
- `CallControllerPolicyTests` now has five tests. The newly added
  `signalingEndDismissesPendingCallsWithoutDisconnectingAcceptedOrConnectedPeers` test
  was observed failing under a deliberate policy mutant and passing after
  restoration in the isolated harness. Logs under that MCP workspace's `logs/`:
  `swift_package_test_2026-09-14T16-28-17-821Z_pid32758_a11eab7c.log` (red) and
  `swift_package_test_2026-09-14T16-28-32-982Z_pid32758_2bf37a2e.log` (green).
  The restored isolated run passed eight tests: five controller-policy and three
  account/subscription-policy tests. Earlier group-recipient deadline mutation
  evidence is `swift_package_test_2026-09-14T16-22-55-501Z_pid32758_4ff2a202.log`.
  Restored policy SHA-256:
  `166d24c267d17bdf9970b8644bcba3ae80550138296de8b800af9d34bb344f37`.
- Later combined iPad result: **25 native unit tests passed**, while the UI test
  failed at its iPad tab selector. Result:
  `~/Library/Developer/XcodeBuildMCP/workspaces/den-65f821c5653d/result-bundles/test_sim_2026-09-14T16-49-27-189Z_pid86141_78f446ca.xcresult`.
  The selector was adapted to the exact tab label without requiring a TabBar
  ancestor; selected/hittable assertions were retained. The second UI run at
  `16:54:53`, result suffix `777659ab`, passed login/session restoration,
  send/edit/reply/react/delete, then failed keyboard dismissal. Resource pressure
  was present, but the cause is not yet established. No keyboard fix is claimed.
- Exclusive iPad reruns passed keyboard dismissal without an editor change.
  iPad's collapsed Search button needed an explicit test navigation step, with
  all search/result assertions retained. The next run exposed a real app issue:
  opening a search result removes the floating tab controls, visually and in AX.
  A new short regression reproduced it in 27.85 seconds, result
  `18-01-10-832Z_pid86141_500fb8c1`. Binding-only and descendant-dismiss-action
  experiments both remained red; neither was retained. Further visibility,
  placement, root-hosting, and dock-removal experiments also remained red.
  A direct ordinary-room control passed the same target-message and usable-tab
  checks (`18-27-23-261Z_pid86141_ae599d50`). The inline native search field then
  passed the original search-result flow:
  `18-30-20-198Z_pid86141_5883710c`. Its selector was adapted from a system SearchField
  to the explicit `search-query` TextField identifier, retaining existence,
  hittable, real result/message, selected Rooms, and usable Settings assertions.
  No timing delay, search-host override, or dock removal remains. Suppressing only
  result navigation then failed the final test's target-message assertion at
  line 174 (`18-32-21-587Z_pid86141_df2806ca`). Source and test hashes were checked
  after exact restoration. The full **2/2 iPad UI suite passed**, zero failures/skips,
  including text/session restoration, search, appearance, and logout:
  `~/Library/Developer/XcodeBuildMCP/workspaces/den-65f821c5653d/result-bundles/test_sim_2026-09-14T18-34-09-576Z_pid86141_52d8475d.xcresult`.
  Restored search source SHA-256:
  `7013fcd6ff6b2a3c0d26685083616cbe83ed2504034d5c2e8d7bb7cadae88d24`.
  Final UI test-file SHA-256:
  `9d0c51ac912716c58ca1e681e05b4479a0d9a6025106ea2e6155792cf6e76cbd`.
  The inspected [search results](shots/m5b-ipad-search-results.png) and
  [opened result](shots/m5b-ipad-search-navigation.png) show the real field,
  message, and preserved top-level tabs in the iPad's dark appearance.
  The final combined **30/30 iPhone run passed**, zero failures/skips:
  `~/Library/Developer/XcodeBuildMCP/workspaces/den-65f821c5653d/result-bundles/test_sim_2026-09-14T18-38-09-014Z_pid86141_a4cafaeb.xcresult`.
  This includes all 28 unit tests plus both real UI flows, on the dedicated
  Den iOS QA simulator. Receipt:
  `/tmp/den-ios-search-navigation-proof/README.md`.
- Three new `VoIPPushControllerTests` exercise the real queue with controlled
  registration continuations and a fake call reporter, without starting PushKit
  or CallKit/audio. They cover invalidation before replacement registration,
  failed-logout cached-token retry, and foreground/late-token callbacks during
  and after successful logout. A source fix keeps logout suspension until
  explicit login because alert deletion/server logout may still be awaiting
  responses. The full **28/28 DenTests baseline passed** before mutation:
  `~/Library/Developer/XcodeBuildMCP/workspaces/den-65f821c5653d/result-bundles/test_sim_2026-09-14T17-19-51-162Z_pid86141_ab38b40a.xcresult`.
  All three deliberate mutants then failed their intended test, with exactly
  two other VoIP tests passing in each run. Queue-order red result:
  `17-29-17-360Z_pid86141_9b7aa7bb`; failed-logout retry red:
  `17-30-46-712Z_pid86141_b22a9ed5`; foreground suspension red:
  `17-32-16-320Z_pid86141_0728d4da`. Exact restoration then passed the full
  **28 tests, zero failures/skips**:
  `~/Library/Developer/XcodeBuildMCP/workspaces/den-65f821c5653d/result-bundles/test_sim_2026-09-14T17-33-52-048Z_pid86141_4ac052f1.xcresult`.
  Restored source SHA-256:
  `0d857cd4302730c246538eef8629b620121b9f2f970eb21cb513c74d809a3ec0`.
  Test-file SHA-256:
  `714481cb9b640e711d9c2b564b3d395e129d1f0fe2c3b9d10f00a944a490a182`.
  Both hashes were checked again when these notes were updated. No mutant
  remains; no test assertions were edited or weakened. Full receipts:
  `/tmp/den-ios-voip-queue-proof/README.md` and `results.json`.
- CallKit probe receipt: `/tmp/den-ios-callkit-probe/README.md`, with exact
  traces under `evidence/`. A minimal CallKit-only binary reproduces the tested
  iOS 26.5 simulator's unsolicited `CXEndCallAction` / generic reason 55 without
  LiveKit, credentials, or networking. With early audio configuration, provider
  refresh, and immediate Start fulfillment, a pending call still ended after
  **62 ms**. Its synchronously connected control survived eight seconds, but
  **neither simulator variant received `didActivate`**. Reporting a false
  connection would hide the symptom without proving audio, so no such bypass
  or manual-activation workaround was added to Den.
- The same pure-probe pattern was tested on the paired **iOS 27.0 physical
  iPhone**. Both pending-connecting and immediate-connected control runs stayed
  alive through the eight-second observation and received real `didActivate`
  and `didDeactivate`. The pending call activated at **1.461 s** and deactivated
  at **9.837 s** after its own cleanup; the connected control activated at
  **1.365 s** and deactivated at **9.737 s**. Traces:
  `evidence/probe-preconfigure-refresh-connecting-device1.log` and
  `evidence/probe-preconfigure-refresh-device1.log`. These probes did not record
  audio, use LiveKit/network credentials, or require microphone permission.
  They prove physical CallKit lifecycle behavior, not decoded/audible media or
  complete Den phone acceptance. Device/runtime versions differ, so the result
  is limited to these tested environments rather than every simulator/release.
  The exact diagnostic app **`app.denchat.diagnostics.callkit`** was stopped
  after cleanup and uninstalled; `evidence/device-uninstall.json` records it.
  The Den app was not replaced or removed.
- Native LiveKit media decoding initially failed on the Tailnet fixture's ICE
  path. The isolated loopback TCP setup below subsequently passed real decoding;
  the earlier quiet-join/subscription checks alone were not accepted as proof.
- PiP's isolated Swift 6 complete-concurrency framework build passed for arm64
  and x86_64 simulator architectures with LiveKit 2.17.0. Local proof:
  `/tmp/den-ios-pip-compile/{build.log,compile-proof.json}`. The source/project
  were retained; obsolete compiled products were removed to recover disk space.
  This is compile evidence only, not a simulator or phone PiP run.

### Direct native media receipt

The isolated simulator probe uses the actual `CallSession`, delegate, policy,
PiP, and `CallMediaView` source with LiveKit **2.17.0**, plus read-only measurement
hooks. The SDK was restored byte-for-byte before this run. It connected to the
real isolated Den/LiveKit fixture and two synthetic Mac browser peers, not a
mock room. No ambient microphone, camera, or cross-app screen was captured.

- Both native peer connections established in **1.97 seconds**, selecting TCP.
- Quiet join detected the existing same-account device. Microphone and output
  started off; no remote audio was subscribed, while both cameras and the
  screen-share video delivered decoded frames.
- Probe-only manual audio activation respected those quiet controls. Enabling
  sound then produced **7,200 PCM frames / 3,284 nonzero samples** from the other
  account. This account's remote microphone remained unsubscribed.
- Leaving cleared local media state. A separate, no-subscription observer
  confirmed the exact two remote connection identities were unchanged, not just
  that the same account names appeared again.
- The inspected [native media screenshot](shots/m5b-native-media-fixture.jpg)
  shows the actual call view, all three video tiles, two people, and quiet-join
  controls. Its diagnostic banner states the manual-audio/CallKit boundary.

Report: `/tmp/den-ios-media-integration-probe/report-loopback-tcp-passed.json`.
This does not prove real microphone/camera publication, CallKit-integrated media,
locked-phone ringing, Bluetooth, PiP, or a Wi-Fi/cellular handoff.

The follow-up used LiveKit 2.17.0's public
`Room.debug_simulate(scenario: .quickReconnect)` on only the probe's own room.
The actual `CallSession` delegate observed `reconnecting` and then `reconnected`,
with matching session phases. All three video counters advanced by another
**16 / 19 / 18 frames**, and other-account nonzero PCM samples advanced from
**6,640 to 80,304**. Local connection identity, quiet microphone/camera choices,
output choice, and own-account microphone exclusion survived. Leaving again
preserved the exact two sibling connection identities.

Omitting just the reconnect trigger failed the unchanged callback/media
assertions with no reconnect callbacks. Exact source restoration then passed.
Reports are `report-reconnect-omission-red.json` and
`report-reconnect-restored-green.json` under the same probe directory; source
hashes and cleanup are in `reconnect-mutation-receipt.json`. The SDK remained
clean and unchanged. Both synthetic browser contexts stopped cleanly, and the
probe simulator was shut down. This proves the SDK's quick signaling reconnect,
not a physical network handoff or interruption-recovery test.

The original Tailnet fixture route failed because the native network sockets
were bound to the Mac's LAN address. A normal OS socket reached the Tailnet TCP
port; a LAN-bound socket did not. A Mac browser client independently received
all three videos over the original UDP fixture, ruling out missing server tracks.
The native proof used stock fixture-only TCP/NAT settings with an owned SSH
forward bound only to Mac loopback. No SDK patch or production networking change
was made. Guarded-listener, exact-config rollback, and forward-cleanup receipts
are in `/tmp/den-ios-fixture-transport-diagnosis.md`. Final cleanup completed at
**18:23 UTC**: restored the original config byte-for-byte, verified the temporary
TCP listener absent before removing only its four tagged guards, stopped and
removed the exact call fixture/container, closed both owned tunnels, and deleted
its credentials and private config backups. The old text fixture remained
unchanged and healthy for the final UI tests. Receipt:
`/tmp/den-ios-call-fixture-cleanup.md`, with durable redacted proof at
`~/.local/share/den-ios-tools/fixture-fwd/esg-557247c662/cleanup-proof.json`.

### Cloud preparation checked locally

Baseline: Xcode **26.6**, Apple Swift **6.3.3**, XcodeGen **2.45.4**. Swift 6
language mode is distinct from compiler version; DenAPI declares tools 6.2 and
LiveKit 2.17.0 declares tools 6.1. See [native README](../apps/ios/README.md) for
bootstrap commands, official Apple references, and the modest proposed matrix.

- `CI_XCODE_CLOUD=TRUE ci_post_clone.sh` passed locally against the tracked/staged
  project, shared scheme, workspace, package locks, and refreshed source archive.
- The pinned generator's documented plugin-trust command is now guarded by both
  Cloud environment and build ID. Three real post-clone probe cases passed with
  only `defaults` intercepted: normal local, local Cloud verifier, and Cloud.
  Omitting the required command failed the Cloud case; exact restoration passed.
  This Mac's Xcode preferences were not changed. Receipt:
  `/tmp/den-ios-cloud-plugin-proof/receipt.json`; rationale and official references
  are in the native README.
- The M5b archive contains **158 tracked files / 162,278 bytes**. Its hash is
  `bebff84b367df73f1925b061de1bee8e0651738a395670e2b9fa7582d596f6de`.
  The stale pre-merge archive failed the drift check; the refreshed archive
  matched under both local Python and system Python 3.9.6.
- Rust **1.98.1** built that archive with `--locked` in **54.41 seconds**, using
  its SQLx offline cache and an independent temporary build target.
- The fixture smoke verified real password login for two disposable users,
  authorized text/DM access, and a persisted second-user seed message.
  Credentials/receipt/log were `0600`, its private directory was `0700`, and
  cleanup stopped the exact process and removed its private files.
- Redacted local receipt: `/tmp/den-ios-cloud-m5b-proof.json`; build log:
  `/tmp/den-ios-cloud-m5b-build.log`. No native media fixture was stopped or
  modified by this check. The Cloud fixture does not configure LiveKit or APNs.

There have been **zero Xcode Cloud runs**. A fresh Chrome check superseded the
earlier login blocker. Nicholas is signed in to App Store Connect and Apple
Developer under the matching Individual team **`UH434K44A3`**; its signing App ID
**`app.denchat.ios`** exists. Apple rejected the first app-record name **Den** as
already used. Nicholas then selected **denchat**; its TestFlight record was
created successfully as **Apple ID `6811985501`**. A fresh App Information read
confirmed bundle ID and SKU **`app.denchat.ios`**, English (U.S.), and the saved
name. The native installed display name remains **Den**. TestFlight says
**Submit a build to start testing**. No App Store review or public release was
submitted; no pricing or distribution settings were edited.

The Cloud account page presents **Get started in Xcode**, without a plan/usage
meter or linked product. A fresh signed-in recheck at **18:20 UTC** confirmed the
same state on both the app and account Cloud pages, with no usage or billing-period
values. This is not a sign-in blocker. Den has used **zero Cloud compute hours**;
all its simulator tests so far ran locally. Apple documents **25 included compute hours/month**;
that is not evidence of this account's current remaining balance. Used/remaining
hours and Den's GitHub source authorization are still unverified. No source
permissions, workflow, run, schedule, or paid subscription were changed. Next:
commit/push the native project, complete source/onboarding, inspect the actual
usage meter, and then use the authorized modest manual workflow for TestFlight.
Public App Store release is outside this task. See
[the native README](../apps/ios/README.md#xcode-cloud-preparation) and redacted
receipts `/tmp/den-ios-cloud-account-proof.json` and
`/tmp/den-ios-cloud-usage-recheck.json`. Local bootstrap success is not
Cloud execution.

## Remaining acceptance

| Check | Current status / next evidence |
| --- | --- |
| Final app build and full native tests | **Passed.** Final iPhone 30/30, full iPad UI 2/2, signed build, physical install and launch. Deliberate mutants were removed and exact restoration verified. |
| Native two-peer call | Direct native video, nonzero decoded audio, and public SDK quick reconnect passed, with exact sibling-preserving leave. Physical capture and CallKit-integrated media remain separate. |
| Actual-app CallKit start/answer/end | Physical pure probes passed activation/deactivation; the tested simulator pending-call path fails independently. Full Den phone call acceptance is still required, without false connected reports or a bypass. |
| Incoming DM foreground/background/locked | Pending real APNs/VoIP configuration plus device checks for accept, decline, cancel, expiry, duplicate delivery, and answered elsewhere. |
| Speaker/Bluetooth/interruption | Needs real phone/accessories: verify route selection, interruption/deactivation/reactivation, and no stale audio owner. |
| Same account on multiple devices | Direct native person grouping, quiet join, own-microphone exclusion, and exact sibling-preserving leave passed. Screen-share audio remains policy-tested only because these synthetic peers publish no share-audio track. |
| Wi-Fi/cellular and reconnect | Needs physical transition during a two-peer call, with media recovery and cleanup observed. |
| PiP and background behavior | Needs phone start/automatic entry, restore, remote-video updates, paused local camera, interruption, and no PiP after leave/logout. |
| Outgoing ReplayKit broadcast | Not implemented; extension/app group/system-consent/audio work remains. |
| Cloud iPhone/iPad and TestFlight | Sign-in/team and denchat TestFlight record verified; complete source onboarding and inspect actual usage meter before a run. No recurring schedule or paid upgrade. |

Do not replace any pending row with a compile result, server/web smoke, or the
earlier M5a phone acceptance. The native lead owns the final evidence update.
