# Review cross-judgment

## Decision

**Tie on the core review task.** Both identify the two demonstrated bugs, reject both harmless hunks, and give reachable requests or event sequences. Neither invents an additional finding. B traces more client consequences and browser mitigations, but also adds an attacker-impact claim that is not newly enabled by the patch. Both overstate what their executed WebSocket test proves.

I read only the neutral responses, relevant frozen source, proposal, and authorized sealed answer key and preparation receipt. The six compared implementation, client, and test files match byte-for-byte between A and B. I did not read native transcripts or freeze metadata, run builds or tests, mutate candidates, or use outside research. Execution claims in the responses are not independently verified by this cross-review. The sealed judge evidence is a separate source of proof, not work credited to either candidate.

## Per-criterion assessment

| Criterion | A | B |
| --- | --- | --- |
| Find demonstrated bugs | Finds recipient-guard removal and cookie-check logic regression | Finds the same two |
| Avoid harmless-change false positives | Correctly declines unstable sort and trim extraction | Correctly declines both; distinct HashMap keys strengthens its sorting explanation |
| Reachable reproduction | Exact cookie request; real WebSocket test sequence | Same cookie request and WebSocket test |
| Privacy scope | Shared-channel read-state leakage, not private-message leakage | Same; correctly limits DM read-state impact to participants |
| Client impact | Correct channel-keyed badge overwrite analysis, not explicitly labeled inference locally | Adds correct `markRead` early-return impact and explicitly labels client effects inferred |
| Browser/CSRF calibration | Calls out no browser execution and spoofable Origin in reqwest | Adds SameSite/HttpOnly and unchanged WS check, but overstates cookie-holder escalation and "all browser traffic" |
| Executed versus inferred | Reports exact earlier failure but presents leak reproduction as executed | More explicitly and repeatedly calls the privacy leak executed, despite the same earlier assertion failure |
| Check breadth | Reports two targeted failures | Reports full API suite, 35 passes and the same two failures; broader coverage, not stronger direct privacy proof |

## What is established

### Cookie writes

The existing test at `crates/den-server/tests/api/auth.rs:58-77` sends a logged-in user's session cookie, a foreign Origin, and that user's valid CSRF token to `POST /tokens`. Both responses report the exact regression, expected 403 versus actual 200. This is a direct behavior assertion. The symmetric correct-Origin/wrong-CSRF bypass follows from the predicate at `src/auth.rs:135`, but neither response separately executes it.

Both correctly describe the intended contract: both checks must succeed. A's "two-factor" wording should be "two checks" or "layered CSRF validation"; this is not multifactor authentication.

B's claim that the token check is "dead for all browser traffic" is too broad. It is bypassed for matching-Origin cookie writes; wrong-Origin writes still require the matching token. `GET`, `HEAD`, and `OPTIONS` are excluded from this write check, and bearer requests use a different path. B's SameSite=Strict mitigation and its caveat about not establishing a realistic foreign-origin token-acquisition attack are useful and supported by source. Neither candidate demonstrates arbitrary cross-site exploitation.

**Material impact correction for B:** a non-browser attacker holding the raw session cookie could already use that same secret as `Authorization: Bearer <session secret>`. `auth.rs:100-124` selects bearer before cookie, matches the session digest, and skips the cookie checks when bearer is present. That credential could already call `POST /tokens`, whose implementation is `credentials.rs:74-90`, not the cited `auth.rs:215+`. Therefore the patch's new acceptance of a forged-Origin cookie request is a genuine contract regression, but is not a newly demonstrated persistent-token escalation for the stated raw-cookie-holder attacker. No additional severity credit is justified for that example.

### Read-state delivery and the client

`ws.rs:129` removes recipient identity while `ws.rs:142-143` preserves channel visibility. `inbox.rs:134-176` emits personalized states per visible-channel member; `inbox.rs:103-130` also emits state on explicit mark-read. Both source analyses identify the real leak. Neither claims DM messages reach nonmembers. A's "admin outsider" means outside a later DM, not outside public-channel visibility.

The reported checked-in test failure is at `tests/api/m2_live.rs:48`, a notification-count assertion. The test stops there, before direct owner assertions at lines 82-84 and 101-103. Extra read-state events can make its `until(ReadStateUpdated)` return before the expected notification. The response does not inspect the offending event's user ID at the failing assertion. Thus both candidates have executed evidence of disrupted event sequencing plus a convincing source explanation, not an executed direct privacy assertion. The sealed judge's separate owner-mismatch test supplies that direct proof. B's "server-side leak executed" and A's "Reproduction executed" should be narrowed accordingly.

Client consequences are real by inspection. `store.svelte.ts:293` applies every incoming read state without checking `user_id`, and `setRead` at line 93 keys only by channel. B additionally follows `markRead` at lines 85-91: if another user's leaked state says the latest loaded message is read and unread_count is zero, the client can skip its own read update. That is a reachable inference. For example, another member marks the channel's latest message read while this user's tab is hidden, then this user returns to the tab. `ChannelView.svelte:28-31` invokes `markRead`, which can take the early return. Neither candidate runs this browser sequence.

## Overall

There is no material difference in bug discovery or harmless-hunk judgment. B provides more useful downstream analysis and reports broader tests; A avoids B's unnecessary attacker escalation claim and is shorter. Both need the same principal evidence correction: an earlier notification-count failure is not a directly asserted privacy leak. Treat the round as a tie rather than rewarding report length or converting the judge's hidden proof into contestant credit.
