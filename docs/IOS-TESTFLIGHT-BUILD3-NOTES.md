# TestFlight build 0.3.0 (3)

## Delivery, September 15, 2026

Build **0.3.0 (3)** was archived from main **5152c5e**, uploaded at **20:51 UTC**,
processed successfully by Apple, and added to the existing internal group
`Nicholas`. App Store Connect shows **Testing**, **2 testers**, and **2 builds**.
Andy remains enrolled; neither tester nor build 1 was removed. This was an
internal TestFlight delivery, not a public App Store submission. No Xcode Cloud
build was started.

[Build 3 in App Store Connect](https://appstoreconnect.apple.com/teams/ccfd7e2e-02f2-4cea-9bcc-78c9d019380c/apps/6811985501/testflight/ios/d4b8134f-8008-4955-bc0b-cbd0d2cbaeb5)

## Signing finding

The absence of a local Apple Distribution certificate was real but did not
prevent distribution. Apple Developer already contained a **Distribution Managed**
certificate, `CC9Q4AC94J`, expiring September 14, 2027. The retained build 1 export
receipt showed that it had used this cloud-managed certificate too.

The first build 3 export returned `No Accounts`. Xcode 27's desktop Apple Accounts
screen then showed no signed-in account. Nicholas completed native Xcode sign-in;
the same archive exported successfully afterward. Browser sign-in alone had not
signed Xcode in. No certificate was created, revoked, or imported, and no password
was requested or stored by the agent.

The exported IPA passed `codesign --verify --deep --strict`. Its signing authority
is **Apple Distribution: Nicholas Caron (UH434K44A3)**; the distribution summary
identifies **Cloud Managed Apple Distribution**, fingerprint
`96C7649E6D3D3F3D3723CBD0356A1C00DF2E36AC`. Production APNs,
`get-task-allow: false`, `beta-reports-active: true`, and
`testFlightInternalTestingOnly: true` were verified.

## Source and verification

This build includes the compatibility repair in `b4a7b1c`, profile-update parity
in `a1ce40b`, and the current saved server schema from `829c8fd`. Native source is
unchanged from the local regression recorded at `f345806`: 55 native tests and two
existing text UI tests passed, with one permission-gated test skipped. The
[GitHub native run](https://github.com/nhclink16/den/actions/runs/35016411474)
also passed 55 native tests with that explicit skip. These are separate from
physical TestFlight acceptance.

The archive used Xcode 27.0, the iOS 27.0 SDK, Release configuration, and the
command-line override `CURRENT_PROJECT_VERSION=3`; the checked-in project remains
at build 2. The next upload must use a number greater than 3.

Apple's encryption answers retain standard encryption beyond Apple's OS for the
unchanged LiveKit binaries. Nicholas explicitly confirmed **No France
distribution** for this build. Upload emitted the same nonfatal missing-dSYM
warnings for the prebuilt LiveKitWebRTC and RustLiveKitUniFFI frameworks; Den's
own dSYM is present.

**Physical acceptance is pending.** Nicholas has been asked to install build 3
from TestFlight, sign out and back into denchat.app, and verify that both Rooms
and the New message people list populate. Successful upload and group assignment
are not being counted as that result.

Local archive, IPA, commands, signing receipts, and upload logs are retained under
`/tmp/den-ios-testflight-build3-5152c5e/`; `release.json` records each stage. This
directory is temporary evidence, not a durable source dependency.
