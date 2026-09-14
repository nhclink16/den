# Den iOS handoff to the iMac

Prepared 2026-09-14 for Nicholas's requested **GPT-6 Astra, xhigh** lane.

## Current state

- Work locally on the iMac at `/Users/nicholascaron/Projects/personal/den`.
  Its main checkout stays on `main`. The remote is `nhclink16/den` on GitHub.
- Start point is `13fa4c5`, with the signing gate cleared. The iMac checkout
  was clean before this handoff. The Debian checkout at `/home/nicholas/den`
  has unrelated untracked design directories; leave those untouched.
- Herdr workspace **Local > Den iOS**, remote workspace `w11`, pane `w11:p1`.
  These IDs belong to the iMac. Debian also has a `w11`, so always identify
  the machine before controlling a pane. Agent name will be `den-ios`.
- No native Den client has been implemented. The app installed on Nicholas's
  phone is a disposable signing probe showing `Den signing check`.
- Nicholas is awake and authorized continuing through text, voice/video calls,
  and testing with Xcode Cloud. The old instruction to stop after writing
  `M5A-NOTES.md` belongs to the overnight task. Do not stop just because that
  notes file already exists. Finish independent implementation and verification;
  document specific remaining attended checks honestly.

## What changed

`160a135` recorded the initial signing blocker. `13fa4c5` records the successful
signed physical-device build, installation, and launch. Neither commit implements
the app. This handoff adds context and Mac-local tools; calls and Xcode Cloud
have been discussed and authorized, but have not been implemented or configured.

Read these before changing code:

1. [Agent rules](../AGENTS.md), [design](DESIGN.md), and [roadmap](ROADMAP.md).
2. [M5a brief](M5A-IOS-BRIEF.md) and the **first section** of
   [M5a notes](M5A-NOTES.md). Later signing errors there are historical.
3. [Themes](THEMES.md), [M4 notes](M4-NOTES.md), [M8 notes](M8-NOTES.md),
   and [hosting plan](HOSTING-PLAN.md).
4. [M3 notes](M3-NOTES.md), `crates/den-server/src/calls.rs`,
   `crates/den-server/tests/api/calls.rs`, and the web call store
   `apps/web/src/lib/call.svelte.ts`. Locate the store with `rg --files` if moved.

## Ownership and coordination

You are the implementation lead for `apps/ios`, native tests, iOS build scripts,
screenshots, M5a/M5b notes, and Xcode Cloud preparation. Nicholas's subsequent
server addendum assigns device/APNs endpoints, DM invitations, and OpenAPI
generation to Debian Herdr agent `astra-ios`, in branch `server-ios` at
`/mnt/storage/den-server-ios`. Ownership was explicitly confirmed. See
[server notes](M5-SERVER-NOTES.md) for the API contract, schema artifact, tests,
and private native-test fixture. `fable` coordinates the other lanes.
Continue native work while coordinating server dependencies. Use the fleet
skill to verify a remote alias and identity before controlling another machine.

Stage explicit owned paths only, never `git add .` or `git add -A`. Commit small
slices and push. Before pulling or pushing, inspect the current state, preserve
other lanes' work, and resolve ordinary concurrent updates. Changes to `den-core`
or migrations require a short branch in a **separate worktree**, so the iMac's
main checkout remains on `main`. Notify the other engineer before merging.
Applied migrations are immutable. Avoid unrelated desktop, web, or design edits.

## Verified evidence and tools

The iMac is `Nicholas-Work.local`, user `nicholascaron`, logged-in GUI UID `502`.
Xcode is `26.6`, build `17F113`; XcodeGen is `2.45.4`. Native `xcodebuild`,
`simctl`, `devicectl`, and `mcpbridge` are present. Codex CLI is `0.154.0`.

The lane profile is `~/.codex/den-ios.config.toml`, selected with `-p den-ios`.
It sets `gpt-6-astra`, `model_reasoning_effort = "xhigh"`, and enables the
`den_xcode` MCP server. XcodeBuildMCP **2.7.0** is pinned outside the repo at
`~/.local/share/den-ios-tools/node_modules`. It provides simulator/device build
and test, screenshots, UI automation, project discovery, Swift package tools,
and LLDB debugging. Its telemetry is disabled in this profile. The existing
global Codex configuration and disabled iOS plugin were left intact.

Read `~/.agents/skills/den-ios-xcodebuildmcp/SKILL.md` before using it. Both
the MCP interface and the pinned CLI are available. Use the explicit executable
if your shell resolves another installation:

```sh
~/.local/share/den-ios-tools/node_modules/.bin/xcodebuildmcp --version
~/.local/share/den-ios-tools/node_modules/.bin/xcodebuildmcp-doctor
```

Doctor verified Xcode build tools, bundled AXe `1.8.0` UI automation and video
capture, and LLDB DAP. No additional Apple IDE bridge setup is required for
these tools. `mcpbridge` remains available if a specific Xcode-only task needs
it; do not enable extra integrations without a concrete need. Set the project's
scheme and exact simulator through MCP session defaults once generated.
The stdio MCP smoke connected, listed 66 tools, and successfully called
`list_sims` and `list_devices`, seeing Nicholas's connected phone. Evidence is
`~/.local/share/den-ios-tools/mcp-smoke.json`; rerun with
`node ~/.local/share/den-ios-tools/mcp-smoke.mjs` if needed.
See the [tool documentation](https://www.xcodebuildmcp.com/docs/clients)
for its MCP configuration and the installed CLI's `--help` for exact syntax.

### Physical device and signing

- Bundle ID `app.denchat.ios`; development team `UH434K44A3`.
- Apple Development signing identity is valid.
- iPhone 17 Pro Max, name `Nicholas (2)`, reports iOS `27.0`.
- Device ID `6013C890-8F56-57F9-B447-5A23057BC488`.
- The phone was paired, Developer Mode enabled, developer disk services ready.
  Recheck availability at use time.
- The probe built, installed and launched successfully, process `705` at the
  earlier check. This is signing evidence only.
- Original evidence is `/tmp/den-m5a-signing/{gui-build.log,gui-build.exit,
  install.json,launch.json}`. Those files are temporary and may be cleared.

The new Herdr terminal, like direct SSH, sees the keychain as locked. A build
inside the logged-in Aqua session sees it unlocked. A helper preserves this
working execution path without exporting or saving a keychain password:

```sh
python3 ~/.local/share/den-ios-tools/run-in-aqua.py \
  --cwd /Users/nicholascaron/Projects/personal/den/apps/ios -- \
  /usr/bin/xcodebuild -project Den.xcodeproj -scheme Den \
  -destination 'generic/platform=iOS' -allowProvisioningUpdates \
  -allowProvisioningDeviceRegistration -derivedDataPath /tmp/den-ios-device build
```

Adapt the project/scheme to the actual generated app. The helper creates one
temporary Aqua LaunchAgent, records stdout/stderr and exit status under
`~/.local/share/den-ios-tools/runs/`, then unloads that job. Its preflight in
`runs/aqua-tl0m05dp` returned `unlocked=yes readable=yes writable=yes`, exit 0.
Signed device builds should use this path if direct MCP signing fails.
Simulator builds do not need a development signing identity.

This Xcode's `devicectl` has no device screenshot subcommand. Use simulator
layout captures plus physical-device logs, and request a phone screenshot only
when it is needed for a specific visual check. Do not report the simulator as
physical-device evidence.

## Implementation and acceptance

Deliver M5a first, then M5b, each with its own acceptance notes. Work past a
missing APNs key on everything that does not require it.

M5a's brief is the detailed contract. Use native SwiftUI, iOS 26 minimum,
Swift 6 strict concurrency, XcodeGen `project.yml`, and Apple's Swift OpenAPI
Generator against the real `/openapi.json`. Shared types originate in `den-core`.
Keep bearer credentials in Keychain per canonical origin, use `/auth/ws-ticket`,
and follow the desktop's session isolation and redirect rules. Implement the
full text/DM/upload/inbox/search/appearance/offline and notification behavior
specified in the brief, with shared paired themes and licensed bundled fonts.
Canvas/terminal cards remain M5c placeholders; no M5c implementation is requested.

For push, implement user-scoped device registration/removal and APNs delivery
for mentions, DMs, and followed channels. Test denied access and logout cleanup.
Distinguish sandbox and production tokens, collapse by channel, deep-link to
the message, and remove invalid APNs tokens. Missing credentials must not break
chat. Never put credentials in Git, screenshots, commands, or test output.

For M5b, use the existing Den authorization and LiveKit Swift SDK:

- Join/leave the hangout and DM calls, mic mute, speaker/audio route choice,
  camera enable/disable/switch, remote audio/video, reconnect and interruption
  handling. Keep chatting while a call is active.
- Native CallKit incoming DM ringing, accept, decline, caller cancellation,
  timeout, and cleanup across foreground, background, and locked phone states.
  The current token endpoint does not supply this invitation lifecycle. Add
  authenticated server signaling and call state with race/idempotency tests.
  Verify PushKit and CallKit requirements from current Apple documentation.
  Alert APNs tokens and VoIP tokens have different purposes; model them clearly.
- Background audio and picture-in-picture where supported, with deliberate
  audio-session behavior. Verify actual speaker and Bluetooth routing on hardware.
- Receive every camera/share simultaneously in usable phone layouts. Include
  iOS outgoing screen broadcasting through a ReplayKit Broadcast Upload Extension
  and the required App Group if delivering native screen sharing. Treat its
  system consent and cross-app capture as physical-device acceptance checks.
- Preserve the design's same-account multi-device behavior: count a person once,
  additional devices join mic and sound off, suppress the same account's remote
  mic while allowing its screen-share audio, and leaving one device preserves
  its other connections. Do not silently impose a per-account device limit.

Write `docs/M5B-NOTES.md` with results for these behaviors. A token response or
a successful compile does not prove usable calling. Test with a second real
client and record both peers, tracks, and reconnect behavior.

### Test plan, including Xcode Cloud

Nicholas expressly requested cloud testing. Prepare reproducible project and
package generation, shared schemes, and Xcode Cloud scripts/configuration.
Set up build/analyze, meaningful Swift tests, and UI tests on representative
iPhone and iPad simulators. Create TestFlight delivery when account access
allows. Pin packages and record the exact CI run and artifact links.

Xcode Cloud is **not configured yet** and remaining compute allowance has not
been checked. Check it before scheduling runs; use local builds to resolve
routine iteration failures. Keep the cloud matrix modest and avoid recurring
schedules or paid upgrades. If initial GitHub/App Store Connect authorization
requires Nicholas, prepare everything else and document the exact action.

Local simulator UI tests must exercise login/session restore, rooms/DMs, send,
reply/react/edit/delete permissions, upload progress and playback/scrub,
inbox/search navigation, appearance, logout, offline cache, and reconnect as
applicable. Unit-test genuinely tricky grouping/mention/call-state logic.
Verify Dynamic Type, dark/light appearance, and keyboard/safe-area layouts.

Physical iPhone acceptance must separately cover install/launch, real server
login, Photos/video playback, APNs with the app closed and notification tap,
real mic/camera media, background/lock behavior, CallKit, interruption recovery,
speaker/Bluetooth routes, and a Wi-Fi/cellular transition. Cloud simulator
passes do not establish those results. Mark each item pass, fail, or needs
Nicholas with exact steps and evidence. Save layout shots under `docs/shots/`.

Use disposable accounts/channels for test writes and clean up exact created IDs.
Preserve friends' conversations and settings. Follow existing smoke scripts'
cleanup patterns; keep private credentials and call recordings out of Git.
Read existing deployment instructions before any production change, retain
rollback and verify public health after a deployment. Debian Rust builds use
a `CARGO_TARGET_DIR` on `/mnt/storage` because the root filesystem is tight.

## Known blockers

- `/home/nicholas/.config/den/apns.p8` on Debian was still absent at handoff.
  APNs delivery needs that key, Key ID, and Team ID in server configuration.
  Recheck presence without reading it into output. PushKit also requires a
  correctly configured VoIP topic/capability. No end-to-end push pass is claimed.
- Xcode Cloud source/account linking and remaining hours are unverified.
- OS consent, Face ID, actual audible media quality, and some phone gestures
  may need Nicholas. Complete independent work and leave a precise checklist;
  do not repeatedly poll an unavailable phone or claim unperformed checks passed.

## Next best step

First verify your own inherited Herdr IDs, `herdr pane current --current`,
working directory, branch, and tool access. Call `den_xcode` to list simulators
and devices. Read this handoff and the source briefs, then generate the native
project and deliver a real vertical slice: login, rooms, and send/read messages
on the simulator and physical phone. Continue through the acceptance list.

## Fresh thread prompt

> You are Den's iMac-local iOS lead, using GPT-6 Astra at xhigh. Work in
> `/Users/nicholascaron/Projects/personal/den`, main checkout on main. Read
> `docs/IOS-IMAC-HANDOFF.md` and the referenced briefs first. Nicholas authorized
> implementation of M5a text and M5b voice/video plus Xcode Cloud preparation
> and real-device testing. Verify inherited Herdr context and the `den_xcode`
> MCP tools first, then build the app. The signing gate is cleared, with an
> Aqua-session helper documented in the handoff. The existing M5A notes record
> a probe, not a finished app. Own the native work and coordinate required
> server dependencies with astra-ios on Debian, following the server addendum
> and M5-SERVER-NOTES.md. Preserve other lanes' work, stage explicit
> paths, commit and push, and record actual acceptance evidence. Continue
> independent work when credentials or physical consent need Nicholas.
