# Den for iOS

Native SwiftUI, iOS 26 minimum, Swift 6 strict concurrency. `project.yml` is the
project source; `Den.xcodeproj` is its committed, reproducible output for Xcode
Cloud discovery. The shared `Den` scheme includes `DenTests` and `DenUITests`.

## Local project setup

Use Xcode 26.6 (verified Apple Swift **6.3.3**) and XcodeGen **2.45.4** for the
current baseline. `SWIFT_VERSION: 6.0` selects Swift 6 language mode, not a Swift
6.0 compiler. DenAPI declares Swift tools 6.2; LiveKit **2.17.0** declares Swift
tools 6.1. The installed 6.3.3 compiler satisfies both package manifests. From
the repository root:

```sh
den_xcodegen=$(apps/ios/ci_scripts/install-xcodegen.sh)
"$den_xcodegen" generate --spec apps/ios/project.yml
xcodebuild -resolvePackageDependencies -project apps/ios/Den.xcodeproj -scheme Den
apps/ios/ci_scripts/ci_post_clone.sh
```

Review and stage explicit paths after generation. Keep these discovery inputs in
Git, excluding user settings and build products:

- `Den.xcodeproj/project.pbxproj`
- `Den.xcodeproj/xcshareddata/xcschemes/Den.xcscheme`
- `Den.xcodeproj/project.xcworkspace/contents.xcworkspacedata`
- `Den.xcodeproj/project.xcworkspace/xcshareddata/swiftpm/Package.resolved`
- `Packages/DenAPI/Package.resolved`

Cloud needs a consistently located project and shared scheme before its checkout
hook runs. Apple cautions against changing the project dynamically during a
Cloud build, so the hook generates a temporary comparison copy and fails on
drift; it does not replace Cloud's discovered project.
[Apple project requirements](https://developer.apple.com/documentation/xcode/setting-up-your-project-to-use-xcode-cloud)

Swift dependencies are pinned in the package manifests and both lockfiles. For
direct local `xcodebuild` builds/tests, pass `-disableAutomaticPackageResolution`
after the reviewed initial resolution. The OpenAPI plugin uses the committed
`Packages/DenAPI/Sources/DenAPI/openapi.json`, never a production API fetch in CI.
[Apple package CI guidance](https://developer.apple.com/documentation/xcode/building-swift-packages-or-apps-that-use-them-in-continuous-integration-workflows)

The XcodeGen installer downloads the
[2.45.4 release](https://github.com/yonaskolb/XcodeGen/releases/tag/2.45.4), checks
its pinned SHA-256, and unpacks into a temporary tools directory. It does not
change Homebrew or require `sudo`.

## Remote iteration with TestFlight

**September 14, 20:32 EDT:** the first local upload of **0.3.0 (1)** succeeded
and Apple reported the package processing. It includes the dictation repair in
`ada6aa9` plus the app privacy manifest below. Archive, distribution export and
the exported IPA's signatures passed. The re-signed app has production APNs,
`get-task-allow: false`, and `beta-reports-active: true`; the export retained
`testFlightInternalTestingOnly: true`. No Cloud run was started.

App Store Connect's Chrome session is now at the Apple sign-in gate. Processing
completion, first internal tester setup and installation are **not verified**.
Xcode's existing account successfully signed and uploaded despite that browser
gate. Upload emitted nonfatal missing-dSYM warnings for the prebuilt
LiveKitWebRTC and RustLiveKitUniFFI frameworks; Den's own dSYM is present.
Local receipts and the retained archive/IPA are under
`/tmp/den-ios-testflight-ada6aa9/`. The phone watcher remains stopped.

TestFlight delivery does not require the iPhone to be connected to the build Mac.
A local Release archive can be uploaded from the iMac; Nicholas then installs it
through TestFlight on his own internet connection. This path does not start an
Xcode Cloud run. Direct Xcode installs and physical-device debugging remain a
separate workflow.

Keep this lane's exports **internal TestFlight only**. The installed Xcode's
`-exportArchive` options are `method: app-store-connect`, automatic signing,
team `UH434K44A3`, and `testFlightInternalTestingOnly: true`. Use `destination:
export` for a local distribution-signing check and `destination: upload` for
delivery. This is not an App Store submission or an external-beta invitation.
Do not treat a successful archive/export/upload as proof that Apple finished
processing the build or that Nicholas can install it.

The app bundles `Den/PrivacyInfo.xcprivacy` with the UserDefaults reason
`CA92.1`: Den reads and writes its own preferences, server URL and installation
identifier. Dependency manifests do not replace the app's own declaration.
This declaration does not answer App Store Connect's separate data-collection
questions. [Apple required API reasons](https://developer.apple.com/documentation/bundleresources/app-privacy-configuration/nsprivacyaccessedapitypes/nsprivacyaccessedapitypereasons)

## Xcode Cloud preparation

**Prepared, not Cloud-verified.** No remote workflow or Cloud run was created by
these scripts. Account/source linking, remaining compute allowance, the available
Cloud image, and TestFlight delivery still require verification. Local test and
device acceptance results belong in [M5a notes](../../docs/M5A-NOTES.md) and the
M5b notes when that milestone lands.

Live account recheck on 2026-09-14 supersedes the earlier sign-in failure.
Nicholas is signed in to App Store Connect and Apple Developer as **Nicholas
Caron**, an Individual Apple Developer Program member on team **`UH434K44A3`**.
The registered signing App ID **`app.denchat.ios`** exists. Apple rejected the
first app-record name **Den** as already used. Nicholas then selected **denchat**,
and that record was created successfully for **TestFlight only**. Its saved App
Information verifies **Apple ID `6811985501`**, bundle ID and SKU
**`app.denchat.ios`**, and primary language **English (U.S.)**. The native installed
app remains **Den**. No App Store review or public release was requested or
submitted, and no pricing or distribution settings were edited.
[TestFlight record](https://appstoreconnect.apple.com/teams/ccfd7e2e-02f2-4cea-9bcc-78c9d019380c/apps/6811985501/testflight)

TestFlight says **Submit a build to start testing**. The app's Xcode Cloud page
still says **Get started in Xcode**; no build or workflow exists yet.

**Users and Access > Xcode Cloud** shows first-use **Get started in Xcode**, not a
usage meter or linked product. The included membership allowance is **25 compute
hours/month**, as documented by Apple, but this account's current used/remaining
balance is **not yet visible**. Do not report 25 remaining hours from the included
allowance alone. [Apple allowance and setup](https://developer.apple.com/xcode-cloud/get-started/)

Den's Cloud product and GitHub source authorization are not established. The
Integrations page offers App Store Connect API access request, which was not
submitted and is not a prerequisite for GUI setup. No workflow, run, recurring
schedule, paid subscription, source-access grant, public release, pricing, or
distribution change was made beyond creating the required app record. The
TestFlight record remains available for handoff.
Redacted receipt: `/tmp/den-ios-cloud-account-proof.json`.

Onboarding progressed at 19:14 UTC on September 14 to an **inactive local wizard
draft**, `Den native checks`, using the offered Xcode **26.6 (17F113)** image and
both fixture variables below. It is not a remote workflow. Onboarding reached
**Grant Access to Your Source Code** for `nhclink16/den`, with Next disabled.
The grant button is visible but missing from actionable accessibility children;
the desktop bridge also reports a window/frame mismatch for pixel input. No
source grant was made. Browser-only continuation still shows onboarding without
an account usage meter. **Den usage remains zero Cloud hours; the account's
used/remaining balance and billing period are unknown.**

After that exact source-access step is completed, restrict the GitHub grant to
`nhclink16/den`. The inactive draft still contains Apple's default **Branch
Changes (main)** and **Archive**; remove them and configure manual Analyze/Test
as below before activating or starting anything. Check the account meter first.
No paid plan, schedule, TestFlight build, or App Store submission was created.
The wizard was subsequently canceled and only this lane's Den project window
closed to release local test resources. Its reviewed values are retained in the
receipt, not an active or remotely saved workflow. Reopen Den's Cloud onboarding
to resume the source-access step; do not assume the local draft survived Cancel.
Receipt: `/tmp/den-ios-cloud-onboarding-live/checkpoint.json`.

A read-only Chrome refresh at **19:57 UTC** confirmed the account usage page,
Den usage page and Den build list all still show **Get started in Xcode**, with
no numeric meter or build rows. Den has started zero Cloud runs. The standard
included allowance remains **25 compute hours per month**; this is not evidence
that 25 hours remain across the account. Receipt:
`/tmp/den-ios-cloud-usage-final.json`.

### Before the first run

1. In App Store Connect, check **Users and Access > Xcode Cloud** for team usage
   and the remaining allowance. Do not purchase a plan or enable a recurring
   schedule. [Apple usage guidance](https://developer.apple.com/documentation/xcode/reviewing-xcode-cloud-usage-data/)
2. Verify the signed-in Apple Developer team, App Store Connect app record for
   `app.denchat.ios`, and GitHub permission for `nhclink16/den`. Complete any
   required account-holder/source authorization with Nicholas. These permissions
   are not established by a successful local signing build.
   [Apple setup requirements](https://developer.apple.com/documentation/xcode/setting-up-your-project-to-use-xcode-cloud)
3. Commit the project, shared scheme, project-level package lock, and current
   fixture source archive. Run `ci_post_clone.sh` locally; resolve failures before
   spending Cloud hours.
4. In Xcode's Report navigator, choose **Cloud > Get Started**, select Den, and
   review the proposed workflow. Keep the initial workflow manual: review and
   remove proposed automatic branch/PR triggers before completing setup. Do not
   add a schedule. [Apple first workflow guide](https://developer.apple.com/documentation/xcode/configuring-your-first-xcode-cloud-workflow/)
5. Select the offered image matching the verified Xcode 26.6 / Apple Swift 6.3.3
   baseline when available, and record both versions. Otherwise verify the image
   satisfies the package manifests above and the iOS 26 deployment target before
   running. Use one offered iPhone and one offered iPad simulator, not a broad
   device/runtime matrix. Verify the native UI flow on both locally first.

Suggested initial actions for the shared `Den` scheme:

| Action | Scope |
| --- | --- |
| Analyze | iOS app; required to pass |
| Test | Debug `DenTests` and `DenUITests`; one iPhone and one iPad; required to pass |
| Archive / TestFlight | Add after account, signing, and delivery setup is confirmed |

The Test action builds the test products, so a duplicate Build action is not
needed for build evidence. Keep an Archive separate from simulator testing.
[Apple action configuration](https://developer.apple.com/documentation/xcode/configuring-your-xcode-cloud-workflow-s-actions)

Set these **nonsecret workflow environment variables**:

```text
DEN_UI_FIXTURE_PATH=/tmp/den-ios-cloud-fixture.json
DEN_UI_REQUIRE_FIXTURE=1
```

Cloud passes custom environment variables to test runners; the test forwards the
fixture path to the simulator app. The required-fixture setting makes absent
provisioning a failed test instead of a skipped success. Do not put a real Den
password/session/APNs key in the workflow.
[Apple environment reference](https://developer.apple.com/documentation/xcode/environment-variable-reference)

### Script phases and the isolated UI server

Apple runs test actions in separate build and test environments. Only the build
environment receives the repository checkout; the test environments receive
`ci_scripts`. `ci_post_clone.sh` does not run on those test environments.
[Apple's workflow walkthrough](https://developer.apple.com/videos/play/wwdc2021/10269/)

| Script | Work |
| --- | --- |
| `ci_post_clone.sh` | Install pinned XcodeGen; check committed project/scheme/package locks and fixture archive drift. |
| `ci_pre_xcodebuild.sh` | For `test-without-building`, build/start the loopback fixture, seed two users and rooms, ready the exact `CI_TEST_DESTINATION_UDID`, and reset Den privacy for the first-use regression. Other actions do not start it. |
| `ci_post_xcodebuild.sh` | Stop the exact fixture process and remove its private database, uploads, log, credentials, and receipt, including after test failures. |

Hooks must remain executable and adjacent to the project in `ci_scripts`.
Failures return nonzero. No hook assumes that `/tmp` or files generated on the
build machine persist on a test worker.
[Apple custom script documentation](https://developer.apple.com/documentation/xcode/writing-custom-build-scripts)

The UI suite includes first-use dictation after a real privacy reset. A fresh,
single-use receipt must match the simulator, fixture, bundle and reset command;
a missing or stale reset fails the test. The simulator-only Debug source exercises
real microphone authorization and production PCM conversion without opening
microphone hardware or downloading a speech model. It is not spoken-phone proof.

For a local first-use run, supply the exact simulator UUID and existing native
test products; the runner provisions and cleans up its own disposable server:

```sh
python3 apps/ios/scripts/test-first-run-dictation.py \
  --simulator <UUID> --test-products <Den.xctestproducts> --output <new-proof-directory>
```

Use `--fixture <private-loopback-fixture.json>` to reuse an existing fixture without
stopping it, or `--all-ui-tests` for the full UI suite. `--xctestrun` is supported
instead of `--test-products`. For a separate Xcode/MCP invocation, run
`ci_scripts/reset-dictation-privacy.py --simulator <UUID> --fixture <path>` first.
No permissions are pregranted. Cloud execution of this new reset path remains
unverified until an actual Cloud test action runs.

The post-clone hook also enables the pinned OpenAPI build plugin on the Cloud
worker, using the exact preference key documented by Generator 1.13.1:
`IDESkipPackagePluginFingerprintValidatation`. This skips plugin fingerprint
validation on that worker; package locks remain pinned. Both
`CI_XCODE_CLOUD=TRUE` and a nonempty `CI_BUILD_ID` are required, so a local
Cloud-verifier invocation cannot change this Mac's Xcode preference merely by
setting the first variable. Apple lists the build ID as always available in
Cloud. [Pinned generator FAQ](https://github.com/apple/swift-openapi-generator/blob/1.13.1/Sources/swift-openapi-generator/Documentation.docc/Articles/Frequently-asked-questions.md#how-do-i-enable-the-build-plugin-in-xcode-and-xcode-cloud)

Local proof intercepted only the `defaults` executable while running the real
post-clone/project/archive checks. Ordinary local and local Cloud-verifier
invocations made no preference call; the simulated Cloud environment emitted
the exact documented command. Omitting that command failed the probe, and exact
restoration passed all three cases. No Mac preference was changed. Receipt:
`/tmp/den-ios-cloud-plugin-proof/receipt.json`. Actual Cloud execution remains
part of the first-run acceptance.

`fixture-source.tar.gz` is a deterministic copy of tracked `Cargo.toml`,
`Cargo.lock`, `.cargo`, `.sqlx`, `crates`, and license files. It contains source
and SQLx query metadata, **not a database, test credentials, or production data**.
Keeping it inside `ci_scripts` avoids relying on directory symlinks or a second
repository checkout on test workers, even though `nhclink16/den` is public.
Regenerate it whenever these inputs change:

```sh
python3 apps/ios/ci_scripts/package-fixture-source.py
python3 apps/ios/ci_scripts/package-fixture-source.py --check
```

The server builds with Cargo's committed lock and SQLx's offline query cache.
`build-fixture-server.sh` uses Rust 1.98.1; if absent, it installs the minimal
toolchain into its own temporary directories using an architecture-specific,
SHA-256-checked rustup 1.28.2 installer. No global toolchain change or `sudo` is
needed. Fresh workers need HTTPS access to the pinned Rust distribution and
Cargo dependencies. [Official rustup installation options](https://rust-lang.github.io/rustup/installation/other.html)

`fixture.py` creates credentials with mode `0600`, binds only to loopback, avoids
HTTP proxies for local requests, and records process identity before cleanup.
The app still uses the actual login screen and authenticated API. The fixture
does not configure LiveKit or APNs; this text-test setup cannot prove calls,
push delivery, background audio, or locked-phone behavior.
Native outgoing ReplayKit screen broadcasting is **not implemented**; no
Broadcast Upload Extension or app-group setup is included in the current app.

### Verification and first-run receipt

Locally verified on 2026-09-14:

- Pinned XcodeGen download, SHA-256, and reported version.
- A fresh Rust 1.98.1 `--locked` build from the M5b server archive at `8e9d2e6`,
  with no live SQLite database or repository-relative source dependencies
  (54.41 seconds for the Rust build).
- The earlier foundation smoke also passed with Rust/Homebrew absent from
  `PATH`, exercising the unchanged pinned installer and isolated toolchain.
- System Python 3.9.6 verified real password login for both disposable users,
  authorized text/DM access, and the second user's persisted seed message.
  Credentials/receipt/log were `0600`, the private directory was `0700`, and
  exact-process shutdown removed the private fixture files.
- Archive drift check rejected a deliberate changed archive and passed after
  restoration. The first isolated build correctly failed when SQLx's cache was
  absent; the bundle now includes it.
- The M5b refresh rejected the stale pre-merge archive, then passed under both
  the local Python and system Python 3.9.6 with the same SHA-256.
- `CI_XCODE_CLOUD=TRUE ci_post_clone.sh` passed against the tracked/staged
  generated project, shared scheme, workspace, matching package locks, and
  current archive. This is a local invocation, not a Cloud run.

The refreshed bundle contains **158 tracked files / 162,278 bytes**, SHA-256:

```text
bebff84b367df73f1925b061de1bee8e0651738a395670e2b9fa7582d596f6de
```

The first Cloud run must still prove package-plugin execution, service lifetime
through XCTest, environment propagation, and both simulator destinations in that
actual environment. None of those Cloud results is implied by the local fixture
smoke. Apple sign-in, team identity, and the denchat TestFlight record are
verified. Cloud/source onboarding and the account's used/remaining compute meter
remain open; see the live account receipt above.

Record the workflow/build URL, commit, Xcode image, simulator names/OS versions,
remaining-hours check, action results, and downloaded `.xcresult`/screenshots in
the milestone notes. Fix routine failures locally; do not repeatedly rerun an
unchanged failed Cloud job. Local and Cloud simulator results remain separate
from physical iPhone acceptance.
