# A/V extensions QA ledger

This is the durable acceptance record for `finish/av-extensions`. **Blocked** and
**Pending** are not passes. The last exact product head exercised on codexbox is
`849493e`; the final evidence commit and pull-request checks must still be
recorded before merge.

## Acceptance inventory

| Surface | Acceptance criteria | Verdict | Evidence |
| --- | --- | --- | --- |
| Account contract | `none`, `blur`, and `light_blur` round-trip per camera; legacy camera JSON without `background` reads as `none`; unknown values are rejected; updates remain account-private | Pass | Server integration tests, including the focused legacy row test |
| Settings preview | None, Blur, and Light blur work before a call; assets stay same-origin; cancel, failure, pause, close, and normal teardown release owned resources | Pass | Exact-head primitive and integrated Chromium smokes |
| Published camera | Saved effects apply on enable; Settings and toolbar transitions retain the participant/publication; saved Light restores as Light; camera off/on creates a fresh processor; a remote client keeps decoding after every transition | Pass | Two-account run; eight transition-local frame-count advances in `verification.json` |
| Permissions and failure | Camera denial preserves the effect; effect-only failure leaves plain video, announces fallback, and clears only the resolved physical camera preference—even when the picker is Default | Pass | `av-permission.mjs` at 60/20 fps and `av-background-edge.mjs` |
| Initialization race | A permanently stalled model request cannot own the global GPU lane forever; camera-off and leave cancel the unregistered processor promptly, end its cloned track, and leave later blur usable | Pass | Permanent-stall primitive; in-flight camera-off and leave edge scenarios |
| Unsupported browser/device | No canvas fallback runs; controls explain the unavailable feature; plain preview/call remain usable; processor assets never load | Pass | Modern transform APIs removed in the edge and primitive smokes |
| Performance fallback | Processing pressure steps the unsaved cap 30 → 24 → 15, then reports that the effect cannot continue; saved requested rate is untouched | Pass | Deterministic 60-frame windows in `av-blur-smoke.mjs`; published camera is capped at 30 fps while blurred |
| Existing A/V touched paths | Gain, DSP apply-or-visible-rollback, exact selected-mic constraint, mute/unmute, 720/1080, 24/60/30 fps, mirror, pause/reopen/reload, mobile layout, and remote audio/video remain usable | Pass | The exact `849493e` run passed. It established live, enabled, unmuted raw and processed tracks before sampling, measured the persisted 0.8 baseline → zero → 2.0, and retained all eight transition-local camera checks. Fake Chromium exposed only one physical microphone, so no hardware-to-hardware switch is claimed |
| Cleanup | Normal destroy, stuck SDK teardown, permanent init stall, model failure, effect off, camera off/on, Settings close, and call leave end owned input/output resources and leave no processor canvas | Pass | Primitive, edge, permission, and integrated smokes |
| Accessibility and responsive UI | Native named radios expose selection/disabled state; failure/status text is announced; camera cannot be activated mid-effect transition but can always be turned off; controls remain keyboard-operable and fit 390×844 | Pass | Code/accessibility review plus inspected desktop/mobile screenshots |
| Web and Electron assets | Production build is local-only; lazy processor chunk stays out of initial JS; signed macOS app contains model, WASM, license, attribution, and provenance files | Pass | `npm run build`; `npm run pack`; deep strict codesign; packaged-file inspection |
| Supply chain | Exact installed versions reviewed; model origin/hash recorded; licenses/attribution ship; no new advisory belongs to the MediaPipe/LiveKit path | Pass | Independent supply-chain review, lockfile audit, packaged-file inspection |
| Browser/platform scope | Modern Chromium path is exercised. Browsers/webviews without insertable video transforms degrade to unblurred video without loading the unsafe canvas implementation | Pass for declared scope | Capability-gate run. Native Safari/iOS blur is intentionally not claimed |
| Repository regression | Workspace tests and checks pass at the exact PR head | Pending | The prior exact-head GitHub checks were green. The final rebased head is running normal pull-request checks; local affected checks are recorded below |
| Pull request / CI | Non-draft PR is linked and every required exact-head job is green | Pending | PR #50 is open; the first CI run for the final head is pending |

Screen evidence is under `docs/shots/pr/feat/av-extensions/`. `blur-active.png`
shows a live blurred call plus the selected Blur radio; `mobile.png` shows the
390×844 Voice layout; `verification.json` is the machine-readable exact-head
two-account result.

## Test integrity: red → green proofs

Every new assertion below was observed failing before its behavior was restored.
No temporary break remains in the branch.

- Removing the shared background contract made the server contract test fail to
  compile; adding it made the contract test pass.
- Allowing the package's canvas fallback made the unsupported-capability assertion
  report supported/accelerated; restoring the modern-transform-only gate passed.
- Letting the SDK await its stuck writable control made teardown exceed the 500 ms
  bound; direct owned-resource cleanup passed at about 280 ms.
- Allowing a second segmenter to start during the first destroy made the lifecycle
  overlap assertion true; serialized teardown made it false.
- A model request that never resolved kept `CameraBlur.destroy()` pending beyond
  750 ms. Cancellation plus late-result draining made destroy bounded and a second
  lifecycle completed normally.
- The Default camera picker plus a physical-ID saved blur timed out waiting for the
  fallback notice and preference clear. The unified post-capture fallback passed.
- Camera off during a stalled processor init exposed `backgroundBusy: false` and
  an enabled button. Busy accounting plus transition serialization passed and left
  processor/source/input cleared.
- With saved Light blur, temporarily restoring the old toolbar logic returned
  radius 10 and timed out waiting for radius 5. Synchronizing `lastBackground`
  passed Light → off → Light.
- Temporarily passing a microphone device ID as a string made the first call
  reacquisition constraint fail (`string` versus `{ exact: id }`). The exact
  constraint assertion passes.
- Closing the observer peer connections before the final continuity sample made
  the new transition-local frame assertion time out. With the behavior restored,
  all eight before/after samples advanced.
- Removing `#[serde(default)]` from `CameraSettings` made the focused legacy JSON
  test receive an API error. Restoring it returned `background: none`.
- On the pre-fix call code, a permanently held model request left the visible
  camera-off control disabled. Call-level cancellation made the control available,
  settled camera-off within 1.5 seconds, ended the processor clone, preserved the
  saved effect, and allowed a later blur lifecycle.
- Temporarily removing the new leave cancellation kept the processor clone `live`.
  Removing the old-room guards as well produced a fallback notice after leave.
  Restoring both made leave bounded and prevented the old call from changing the
  notice or saved preference.
- The gain smoke twice reached its unchanged `doubleGainRms > 0.001` assertion
  with a muted raw call source. The instrumented red run measured zero in every
  baseline/zero/double window and recorded `source.enabled: false`. Establishing
  an unmuted raw and processed track before the A/V sequence retained the same
  thresholds and passed with a nonzero baseline, double near `0.0177`, and zero
  exactly `0`.

## Exact product evidence (`849493e`, 2026-09-20)

- Installed versions verified before API decisions: `livekit-client` 2.15.6,
  `@livekit/track-processors` 0.8.0, `@mediapipe/tasks-vision` 0.10.14,
  Svelte 5.56.x, and Vite 8.2.x. Context7 and installed source were both checked.
- Primitive blur smoke: 640×480 output; radii 5/10; same-origin model/WASM;
  source/output cleanup; about 281 ms stuck teardown; no destroy/init overlap;
  permanent-stall cancellation and later reuse; model failure cleanup; unsupported
  capability gate. Pass.
- Edge smoke: permanent-stall camera-off and leave both settled within 1.5 seconds,
  ended the cloned track, preserved the effect, and made no stale notice. A later
  camera lifecycle applied blur. Default-picker effect failure kept plain camera
  with a visible notice and cleared the physical preference; permission failure
  preserved it; the unsupported path loaded no assets. Pass.
- Permission/owned-track cleanup smoke at synthetic 60 fps and unsupported 20 fps:
  pass.
- Two-account smoke at `849493e`: owned preview, account sync, Light/full/off
  transitions, camera restart, exact mic constraint, gain/DSP/mute, video
  resolution/rate/mirror, responsive/reload/Settings cleanup, remote audio/video,
  and zero browser errors. Pass. All eight transition-local remote frame samples
  increased. Remote audio received 132,723 bytes with total energy 0.00366459.
- The authorized diagnostic red run proved the earlier RMS failure sampled a
  LiveKit-muted raw call track: it was `live` but `enabled: false`, while the
  processed output was live/enabled and each running AudioContext advanced about
  0.46 seconds. All three baseline, zero, and double windows were exactly silent
  around 32.4–33.6 seconds into the 180-second fixture. The harness had waited for
  processor creation but had not established that the asynchronous call unmute
  was complete and live.
- The changed-head green run first required `micOn`, raw enabled, processed
  enabled, both tracks live and unmuted, and a processor. Its 250/100/100 ms
  windows measured the persisted baseline gain 0.8 at 0.00704152 / 0.00709874 /
  0.00709348 RMS, zero gain at exactly 0 / 0 / 0, and double gain 2 at
  0.01766231 / 0.01761152 / 0.01774081. The later windows
  changed all 2,048 samples, both tracks stayed live/enabled/unmuted at 48 kHz,
  and the sampling contexts advanced by 0.426667–0.469333 seconds. The full smoke
  passed at about 30.4 seconds, well inside the fixture. The committed
  `verification.json` is this run's machine-readable result.
- Web: `npm run check` has zero errors and one pre-existing unused Login CSS
  warning. Production build passes; initial app JS is 737.62 kB / 213.25 kB gzip;
  the lazy processor chunk is 161.22 kB / 47.96 kB gzip. Existing large-chunk
  advisories remain.
- Rebased local checks pass `cargo fmt --all -- --check`, Clippy with warnings
  denied, and all 82 web script tests. A local `cargo test --workspace` attempt
  compiled the workspace but could not link the server test binary because the
  host volume reached `ENOSPC`; generated build artifacts were removed and that
  unchanged environment failure was not rerun. Exact-head GitHub CI is the clean
  workspace verdict.
- Electron: unit tests 3/3 pass. `npm run pack` produced signed mac-arm64 `Den.app`;
  `codesign --verify --deep --strict` passes. The package contains model, four
  versioned WASM files, MediaPipe/LiveKit licenses, LiveKit attribution, and
  provenance.
- Vendored and packaged model SHA-256:
  `191ac9529ae506ee0beefa6b2c945a172dab9d07d1e802a290a4e4038226658b`.
- Install audit baseline: 32 existing advisories (28 moderate, 4 high); none are
  in the reviewed LiveKit/MediaPipe dependency path.

## Review and issue record

- Existing issues were searched for blur, camera, MediaPipe, processor, A/V, and
  SQLite locks before filing. #48 records the pre-existing selected-microphone
  defect fixed by this branch. #49 was closed after the supposed selected-camera
  defect was disproved: the synthetic fixture has one physical camera and aliases
  are not a real device switch. Closed #20 documents an older SQLx pool-close lock,
  not the historical local music-test lock below.
- Independent supply-chain review passed model provenance, exact pins, licenses,
  package contents, CSP/offline asset handling, and advisory scope.
- Independent code review found six initial material items: Default-picker fallback,
  stalled init cancellation, camera-off/init race, saved-Light toolbar drift,
  nondeterministic mic evidence, and weak remote-frame continuity. All six have
  red → green evidence above. Follow-up review found one call-level cancellation
  defect; rebased commit `8be4075` fixes it with the red → green evidence above.
  Follow-up review approved that product delta with no remaining P0–P2 finding.
- Decisions: canvas fallback remains disabled because its renderer/lifecycle
  behavior is unsafe; device preferences remain account-backed and keyed by the
  resolved physical ID; a different browser/profile device ID starts at None;
  frames stay local and no runtime CDN request is allowed.

## Commits and final-check status

Relevant final review commits:

- `5ef9d25` — bound stalled blur initialization.
- `9637662` — serialize camera/effect transitions and strengthen integrated proof.
- `52d6c00` — add legacy compatibility proof and correct attribution wording.
- `8be4075` — cancel an unregistered blur on camera-off or leave and reject stale
  fallback work.
- `5c55b99` — record exact call-track gain diagnostics.
- `2408f38` — establish an active call microphone before gain checks.
- `849493e` — require both call tracks to be live, enabled, and unmuted.

An earlier codexbox workspace run compiled and passed the AV legacy test, but its
84-test server binary finished **79 passed / 5 failed** under concurrent load. Failures
were SQLite lock/pool exhaustion or wall-clock lifecycle timeouts. Per the one-rerun
rule, the five failures were retried once sequentially. Two iOS tests passed; the
third, `music::a_failed_first_track_advances_to_the_next_one`, failed again after
86.1 seconds with SQLite code 5 (`database is locked`), and the loop stopped before
the last two. This test and music code are unchanged by the A/V branch. The same
base passed in a clean GitHub runner that day, and every check on the previous
exact PR head was green. Per direction, no further local rerun or workflow change
was made. This remains disclosed as environment evidence; the current rebased
exact-head CI must still pass and becomes a blocker if it repeats the failure.

The explicitly authorized instrumented rerun resolved the synthetic-audio blocker.
It captured exact zero values together with the disabled raw source, rather than
inferring fixture exhaustion from the separate Settings meter. The only behavior
change was test setup: wait for join completion, request unmute when needed, and
require the raw and processed call tracks to be live, enabled, and unmuted before
measuring them. No product gain code or assertion threshold changed. The changed
test then passed the full two-account scenario with the measurements above.

PR #50 is open and non-draft. The normal pull-request checks on the final pushed
head remain the clean-environment verdict for the unrelated local SQLite-lock
failure. If they repeat it, stop rather than rerun.
