# Native iOS API compatibility

2026-09-15. Branch `ios-decoder-impl`, worktree `den-ios-decoder-impl`. Covers `apps/ios` only.

**Policy: the native client ignores object properties it does not know about, and still rejects a
response that is missing required data or carries a known field it cannot read.** Adding a field to
an existing response is no longer a breaking change for an installed build. Removing a required
field, changing a field's type, or introducing a new enum value still is, and is surfaced as a
deliberate "Update Den" state rather than as an outage.

1. **Why this was needed.** An installed TestFlight build from `bcfc457` still authenticates against
   current `main` (`b02d6f3`): login, `/users/me`, `/channels` and `/channels/{id}/messages` all
   succeed. `/users/me/appearance` does not. The current response omits `theme`, which that build
   requires, and carries four fields it does not know: `background`, `contrast`, `light_theme` and
   `dark_theme`. The probe separated those two causes by injecting `theme` back into the real
   response, which left the unknown extras as an independent and sufficient reason for the old
   build's generated decoder to reject it. This is a description of the two shapes, not a history of
   the order the server changed in. Because `refresh()` fetches the bootstrap endpoints as one
   group, a single unreadable response failed the whole startup refresh and left the app
   authenticated but empty.
   Profile fields were tolerated; music and sounds work was not causal. Evidence is in
   `/tmp/den-build1-compat-probe/{results.jsonl,ignore-extra-fields-research.md,overlay-results.jsonl}`.
   The already-integrated split-appearance build is unaffected by that specific break; this change is
   about the next additive change, not that one.

2. **How tolerance is produced.** `Packages/DenAPI/Contract/openapi.json` is the server's OpenAPI
   document, stored exactly as fetched. `scripts/prepare-client-schema.py` derives the generator's
   input at `Packages/DenAPI/Sources/DenAPI/openapi.json` by deleting `additionalProperties: false`
   wherever it appears in a schema, and changing nothing else. Ten schemas are affected. Two genuine
   open maps (`additionalProperties: {}`) are left alone. `regenerate-api.sh` now writes the contract
   and reruns the derivation; `ci_scripts/ci_post_clone.sh` runs it with `--check`, so a hand-edited
   or stale derived schema fails the Cloud checkout before a build starts.

3. **Why omit rather than set `true`.** In swift-openapi-generator 1.13.1,
   `Translator/CommonTranslations/translateObjectStruct.swift` returns `.synthesized` when
   `additionalProperties` is absent, `.enforcingNoAdditionalProperties` when it is `false`, and the
   undocumented-properties container when it is `true`.
   `Translator/CommonTranslations/translateCodable.swift` then emits `ensureNoAdditionalProperties`
   for the `false` case and `decodeAdditionalProperties`/`encodeAdditionalProperties` for the `true`
   case. `true` would therefore retain unknown fields on decode and write them back out on
   `PUT /users/me/appearance`, letting an old client echo server state it does not understand.
   Omission gives plain synthesized `Codable`: unknown keys are ignored on decode and never encoded.
   Required fields, value types, enum cases and the `oneOf` tag discrimination are untouched. The
   generator (1.13.1) and runtime (1.12.1) pins are unchanged; there is no native decoder option that
   does this, and no generated Swift is hand-edited.

4. **What this change is not.** The stored contract is byte-identical to the snapshot that was
   previously committed at `Sources/DenAPI/openapi.json`, so the client's view of the API has not
   moved. Adopting a newer live server contract is a separate parity change and should be made by
   running `scripts/regenerate-api.sh` and reviewing the contract diff on its own. No version
   negotiation was added; the client does not ask the server what it supports.

5. **What the app does when a response is unreadable.** `SyncProblem` replaces the single `offline`
   flag. A `DecodingError` (or `DenFailure.invalidResponse`) becomes `.incompatibleResponse`: the
   cached conversations and the keychain session are kept, the WebSocket loop is cancelled instead of
   retried every few seconds, live actions are disabled, and an "Update Den" panel with a Try again
   button replaces the quiet Offline chip. The state clears only after a full bootstrap refresh plus
   the selected conversation both succeed. A genuine network failure still becomes `.networkOffline`
   with the previous chip and the previous automatic reconnect backoff. Unknown WebSocket event tags
   were already ignored by the native router, which decodes by wire tag rather than through the
   generated closed `Event` enum.

6. **A separate prerequisite, not part of this change.** The build Mac moved to Xcode 27.0
   (27A266a), Swift 6.4, which rejects the existing `DictationAudioCapture.convert` callback: it
   captures a local `Mutex`, and `Mutex` is noncopyable, so an escaping `@Sendable` closure cannot
   copy it. Commit `bb9b800` moves that one-shot state behind a small file-private `Sendable` class
   so the callback captures a reference instead. It keeps the lock, the `@Sendable` annotation,
   once-only consumption, the `.noDataNow` reply and the `AnalyzerInput` format behaviour, and
   changes nothing a user of dictation would notice. It is committed on its own because it is a
   toolchain prerequisite for compiling at all, not part of the API compatibility work.

7. **Verification status.** Verified offline: the derived schema matches the contract with only the
   ten `additionalProperties: false` deletions; the generator's behaviour for the three cases, read
   from the pinned 1.13.1 source; every stubbed response body in the new tests against the
   contract's required fields; and parse-only syntax checks of the changed Swift. Verified on real
   builds by the parent lane: all five compatibility tests pass on iOS 27 against the real keychain
   — three `CompatibilityRecoveryTests` and two `ResponseCompatibilityTests` — logged at
   `/tmp/den-ios-compat-native-green-v3.log`. The existing PCM converter regression case is green,
   which is what covers the prerequisite above. Two package-level negatives are proven: restoring
   `additionalProperties: false` fails the additive test on the nested `future_background` object,
   and deleting the required `light_theme` fails the rejection test, logged at
   `/tmp/den-ios-tolerance-{green,strict-negative,required-negative}.log`. The native negatives are
   proven as well: inverting the classifier so `DenFailure.incompatible` returns `error is URLError`
   swaps both verdicts, and all three recovery tests then fail on exactly the behaviour each one
   guards — a schema failure reported as a network outage, one readable endpoint clearing the
   incompatibility, a schema failure retried twice, and the network reconnect loop stopping after a
   single attempt. The file was restored byte-exact afterwards, and the run is logged at
   `/tmp/den-ios-compat-classification-negative.log`. The full DenTests regression on the restored code passed: 45 passed, zero failed, and one permission-gated test skipped. Log: /tmp/den-ios-compat-all-native-green.log. Running `DenTests` needs ad-hoc simulator signing,
   for the reason recorded in `apps/ios/README.md`. **No visual claim is made.** UI acceptance could
   not be driven: the automation snapshot timed out and the running simulator app exposed no
   resolvable accessibility windows, so no screenshot or visual assertion backs this document. No
   device install and no TestFlight upload were made from this branch. This is a source change, so
   builds already installed on devices keep the decoding behaviour they shipped with until they are
   replaced by an updated build.
