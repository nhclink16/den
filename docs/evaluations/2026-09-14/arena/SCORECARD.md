# Den visible arena — September 14, 2026

**Final verdict: Opus wins repair; review is a tie.** Both production fixes pass every independent scenario. Opus wins because its regression test catches both real bugs when reintroduced; Muse's tests miss both. In review, both find both defects and correctly dismiss both harmless changes.

| Round | Muse | Opus | Verdict |
| --- | --- | --- | --- |
| Repair | Working fix; helper-only regression coverage | Working fix; binary-level regression coverage, **max** | Opus |
| Review | 2/2 bugs, both harmless hunks rejected | Same, **high** | Tie |

These are two small tasks, not a broad model ranking.

## Setups

- Muse Spark 1.3 Contributor (`opencode-go/muse-spark-1.3-contributor`), configured xhigh, interactive OpenCode 1.18.31, Go account. Repair records the xhigh variant; review does not record a native variant, so review effort is configuration evidence only.
- Claude Opus 5 (`claude-opus-5`), interactive Claude Code 2.1.270, Max account. Repair finished on **max**; user requested **high** before the review prompt, and all remaining scored work uses high.
- Native clients, system prompts, tool behavior, and subscriptions differ. This is a comparison of working setups, not a model-only benchmark. Prices are reported/API-equivalent token costs, not actual subscription charges.

## Isolation and protocol

Both rounds use pinned Den commit `e188e4ef2f57e1b3db153f5512ee1d33c9e0da8b`, independent source snapshots and separate Cargo targets, with seeded changes in the sole initial commit. Inputs matched before launch. Dependencies were warmed before timing. No contestant saw the answer key or another contestant's submission. No production changes or merge were made.

Both ran visibly in Herdr tab `w11:t5`, Muse in `w11:p6`, Opus in `w11:p7`. Repair had a 20-minute cap, review a 10-minute cap. Both may finish early. File/tool restrictions are not an OS sandbox.

One repair harness interruption: the originally allowed Python command with shell environment expansion was denied by Claude. Both contestants were interrupted; the judge supplied a symlink to each contestant's own CLI binary and the same literal reproduction command. Both ran the preflight successfully; both received a renewed 20-minute cap. No code hint was supplied. This is excluded as a model failure and limits strict timing comparability.

Storage thresholds: start ≥ 10 GiB, guard below 8 GiB. Preparatory builds stopped when required. Old inactive rebuildable caches were cleared; source files, sessions and databases were preserved. No storage interruption occurred during either scored round.

## Repair evidence

Both frozen fixes independently pass build, their submitted tests, formatting, and all five black-box scenarios:

1. Authenticated reconnect, events delivered once, ping handling, honest resync/gap, JSON stdout.
2. Immediate401 exits after one attempt.
3. Immediate403 exits after one attempt.
4. Transient503 retries and recovers before final authentication rejection.
5. Channel filtering preserves global control events and excludes another channel's message.

Primary verification: [repair-verification.json](judge/repair-verification.json).
Baseline/seed proof: [baseline](judge/repair-baseline.json), [independent seeded defects and restored baseline](judge/repair-mutation-proof.json).
Frozen submissions: [A response](judge/frozen/repair/A/response.md), [B response](judge/frozen/repair/B/response.md).
Independent blind review: [repair-crossjudge.md](judge/repair-crossjudge.md).

## Review evidence

The sealed patch contains two behavioral defects and two harmless refactors. A nine-state mutation matrix and a separate direct socket-recipient check establish the classifications. The baseline was restored afterward. Contestants received only the patched source and proposed diff, not the issue count or answer key.

[Preparation receipt](judge/round2/preparation-receipt.md) · [sealed answer key](judge/round2/answer-key.md).

## Repair scoring

Neutral artifact mapping, revealed after independent cross-judging: A = Muse, B = Opus.

| Criterion | Muse | Opus |
| --- | --- | --- |
| Independent behavior checks | 5/5 scenarios pass | 5/5 scenarios pass |
| Submitted tests and formatting | 3 unit tests pass; formatting passes | 1 integration test passes; formatting passes |
| Restore header-drain bug in actual reconnect loop | All 3 tests stay green: missed | Test fails on missing reconnect credentials |
| Restore 403-only guard in actual reconnect loop | All 3 tests stay green: missed | Test fails on continued retry after 401 |
| Scope and maintainability | Correct, but introduces 3 helpers and moves unrelated filtering | Minimal production change, separate CLI integration test |
| Evidence quality | Discloses unexecuted modes, but helper mutations do not prove loop protection | Discloses most limits; later401 is not proof of initial401 or403 |

Both suites pass again after restoring their fixes. No existing assertions were weakened. Muse's helpers and tests were left unchanged during the real-loop mutations; the surviving bugs demonstrate a wiring blind spot, not a compiler or test-runner failure. Opus's failures were its own meaningful assertions, not the judge's supervisor killing the run. [Mutation proof](judge/regression-strength.json) · [summary and exact logs](judge/regression-strength-logs/summary.md).

**Before adopting Opus's test:** bound the blocking `TcpListener::accept()` calls. Its20-second watchdog starts only after the third handshake, so an earlier missing reconnect can hang. This is a test robustness issue; the production repair passes. No candidate was patched after freezing, and nothing was merged.

## Review scoring and evidence corrections

| Criterion | Muse | Opus high |
| --- | --- | --- |
| Real bug discovery | 2/2 | 2/2 |
| Harmless changes correctly declined | 2/2 | 2/2 |
| Executed checks | Two targeted failing tests | Full API suite: 35 pass, 2 fail |
| Direct cookie rejection regression | Proven 200 where 403 required | Same |
| Direct privacy assertion | Not executed by contestant | Not executed by contestant |
| Review verdict | Tie | Tie |

Both correctly identify personalized read-state events reaching other channel members and cookie writes accepting only one of the Origin/CSRF checks. Both correctly accept unstable sorting of unique user IDs and the equivalent trim refactor.

Both overstate the directness of their privacy evidence: the existing test fails on notification count before reaching owner-ID assertions. Their source explanation is correct; the judge's separate recipient-owner test establishes the actual leak. That hidden proof is not credited as contestant work.

Opus adds useful, explicitly inferred client consequences, including corrupted unread state and skipped mark-read updates. Its raw-cookie-holder escalation example is not newly enabled: the original server already accepts the same session secret as Bearer authentication. Its phrase “all browser traffic” is also too broad. Muse's “two-factor” wording should be “two checks,” not multifactor authentication. Neither demonstrated an arbitrary cross-site browser exploit. These corrections do not create a third finding or change the 2/2 discovery tie. [Independent review assessment](judge/review-crossjudge.md) · [Muse review](judge/frozen/review/A/response.md) · [Opus review](judge/frozen/review/B/response.md).

## Cost and speed, separate from correctness

Task-only costs exclude readiness and the harness-only permission check.

| Round | Muse reported token cost | Opus API-equivalent token cost |
| --- | ---: | ---: |
| Repair | $0.00981 | $2.06319 — max |
| Review | $0.00700 | $0.82279 — high |

The review cost ratio is approximately 118:1 while bug-discovery accuracy ties. These are not charges: Muse uses Go and Claude uses Max. Do not treat the mixed-effort combined total as an Opus-high benchmark. Claude's repeated content-block usage was deduplicated by message ID, thinking was not double-counted, and cache-write TTLs were priced separately. Prices and exact raw tokens are in [repair accounting](judge/repair-accounting.md) and [review accounting](judge/review-accounting.md), including [official Anthropic pricing](https://platform.claude.com/docs/en/about-claude/pricing).

Review ran 103.2 seconds for Muse and 111.4 seconds for Opus high. Repair finished 160.0 and 164.5 seconds after resuming, but the earlier work and differently acknowledged interruption mean those are not whole-task timings. No speed winner is assigned to repair. Actual response model IDs stayed fixed; no observed fallback.

## Operating recommendation and alternatives

Use Muse for inexpensive first-pass implementation and review, with behavioral verification kept independent. It produced a correct repair and found the same review bugs for much lower reported token cost in these tasks. Use Opus high as a challenger where regression-test quality, authorization boundaries, and evidence claims matter. This match does not establish that Opus high would reproduce its max-effort repair result; the repair result must retain its max label.

The useful lesson from Opus's repair is the real CLI-level test, not extra abstraction. Muse's fresh-per-connection request construction is a valid alternative, but its auth/filter helper extraction and helper-only tests are not worth grafting into the smaller repair. Both reviews' shared diagnosis is useful; retain the evidence corrections above. Per approved scope there was no graft, merge, deployment, or production edit. The visible review panes remain open with their completed answers.

## Storage and preserved artifacts

All seven match-owned Cargo caches were removed after judging, preserving sources, frozen submissions, native transcripts, fixtures and logs. Free space measured 13.51 GiB afterward; logical cloned-cache sizes are not physical reclaimed space. [Cleanup receipt](judge/post-match-cache-cleanup.json). Re-running builds will recreate those targets.

Preparation source manifests and native accounting JSON retain exact IDs, hashes and timestamps. The match made no edits to the shared main checkout; concurrent unrelated work was left alone.
