# Den arena: Muse 1.3 vs Opus 5

Status: scoped only. No contestant launched, no panes created, no model inference requested.

## Question

Can Muse deliver reliable debugging and review work against Opus 5, and how much verification or repair does each result require?

Compare two real operating setups: Muse Spark 1.3 Contributor in interactive OpenCode and Claude Opus 5 in interactive Claude Code. Different system prompts and tool implementations remain a confound; this is not a pure model benchmark.

## Contestants and viewing

- Muse: `opencode-go/muse-spark-1.3-contributor`. Installed OpenCode is 1.18.31; its model list contains this exact ID.
- Opus: `claude-opus-5`. Installed Claude Code is 2.1.270; first-party Max login is present. Account-specific model access must still be confirmed in the live session before timing starts.
- Fresh interactive sessions, explicit model selection, no automatic fallback. Native TUIs remain visible in Herdr. No `claude -p` or headless `opencode run` contestants.
- One new Arena tab in the Den workspace. On the current narrow 95-column layout, stack Muse above Opus; widen and use side-by-side if the display permits. Leave existing iOS and comparison panes alone.
- Use Herdr agent start/prompt/read to coordinate. Judge stays in this conversation. Preserve full native session logs and frozen submissions; terminal scrollback alone is insufficient.
- Verify model identity, effort, cwd, and tool readiness in each session. Prefer each model's highest supported normal reasoning effort, recorded explicitly rather than assuming similarly named settings are equivalent. Do not enable premium fast mode.

## Round 1: seeded reconnect repair, 20 minutes

Create the same intentionally regressed version of Den's CLI event-stream handling for both contestants. Supply a short incident report and a local failing reproduction, without naming the faulty lines or disclosing the judge's full checks.

Relevant code: `crates/den-cli/src/stream.rs`, especially `tail()`. Keep the healthy server protocol unchanged.

Expected behavior includes:

- Reconnect after a dropped socket and authenticate every connection.
- Stop on explicit 401/403 rejection instead of retrying forever.
- Surface a resync/gap notification; never pretend missed events were replayed.
- Preserve channel filtering and pass through global control events.
- Handle ping/close frames and keep diagnostics out of JSON event stdout.

Deliver a minimal implementation fix, a useful regression test, the exact check results, and a short explanation of the cause and alternatives rejected. Allowed edits are CLI source and CLI tests only. No server/types/migrations/dependency changes.

Before launch, the judge must establish that the unmodified baseline passes, the seeded version fails, and the fixture is deterministic and fast. If this cannot be established, reframe the task before exposing it to either contestant. Hidden checks should include neighboring behaviors, not merely repeat the visible reproduction.

## Round 2: review calibration, 10 minutes

Start both contestants in new sessions on fresh snapshots, not their Round 1 modifications. Give each the same server-side WebSocket/auth patch with independently demonstrated correctness regressions and harmless nearby changes. Do not disclose the number or locations of real issues to contestants.

Output at most three findings. Each needs the affected operation, a concrete reproduction, expected versus actual behavior, and whether it was executed or inferred. Explicitly permit "no finding" for a harmless change. No code fixes.

The judge prepares a sealed answer key with two real bugs and two harmless changes, proves each classification before launch, and checks findings against reachable behavior. Do not reuse the prior upload patch or its answers.

## Fairness and operating limits

- Freeze current baseline `e188e4e` at preparation time; record the full SHA and file hashes. Do not follow main while the match runs.
- Identical source snapshots, task text, fixtures, repository instructions, dependency versions, and file/tool permissions for the paired runs.
- Seeded defects belong in each snapshot's initial commit. Do not leave a clean parent commit or an obvious uncommitted seed diff for contestants to revert.
- Distinct source directories and Cargo targets for each contestant. Warm dependencies before starting clocks; do not count an initial dependency build as model work. Confirm each target corresponds to its own source.
- Hide judge fixtures/answer keys outside contestant-accessible paths. No cross-reading, external research, subagents, commits, infrastructure actions, or access to production credentials. Local test servers only.
- Supply the same explicit project guidance to both; disable unrelated global plugins, MCPs, hooks, and auto-memory where the native clients support that without changing account settings.
- Start paired rounds together. Human hints are disallowed during scored time. Record interruptions and tool failures separately from model mistakes. Freeze whatever exists at the deadline.
- Record model turns, input/output/reasoning/cache tokens where exposed, elapsed time, and intervention count. Claude is using Max and Muse uses Go: API-equivalent cost is not an out-of-pocket invoice. Missing accounting data stays unknown.
- No repeated provider retries inside a scored round. If a provider never starts, mark the run unavailable rather than a coding failure. No substitute model without a new match.
- Storage was about 13 GiB free during scoping and another iOS task is active. Verify sufficient headroom after warming both targets; do not start below 10 GiB free. Pause if free space falls below 8 GiB. Remove only this match's rebuildable caches after judging.

## Judging

Score rounds separately before making an overall recommendation. Cost and speed are separate axes, not bonuses that can erase an incorrect answer.

Repair criteria:
1. Hidden behavior checks pass, with no existing-test regressions.
2. New regression tests exercise the actual behavior and are proven able to fail.
3. Fix stays within scope and preserves neighboring behavior.
4. Report distinguishes executed evidence, inference, and unfinished work.

Review criteria:
1. Finds the demonstrated bugs and explains their impact.
2. Avoids false positives on harmless changes or impossible reproductions.
3. Gives usable reproduction/test guidance and calibrated confidence.

A genuine authorization leak or weakened existing assertion prevents a repair win regardless of speed. Review false positives count against accuracy; longer findings do not earn points.

Read frozen candidates end to end and run independent checks. Use neutral A/B artifact labels for an independent read-only cross-judge, acknowledging that code style may reveal authorship. Compare the independent assessment with the primary judge's evidence. Neither contestant judges its own work.

## Output and scope boundary

Produce one scorecard, per-round verdicts, evidence links, and an operating recommendation: which setup to use for which work. Identify any useful ideas from the other submission without altering the scored artifacts. No synthesis, merge, or production deployment is included in this match. Any later combined implementation is a separate, unscored task.

Expected viewing time is about 30 minutes, plus preparation and independent verification. If both rounds tie, propose a cross-layer feature as a later tiebreaker rather than silently expanding this match.

## Phase checklist

- [x] Frame the proposed comparison and rubric.
- [ ] Prepare and prove fixtures, pin snapshots, preflight model access and storage.
- [ ] Fan out the two visible contestants for each round.
- [ ] Freeze, cross-judge, and independently verify submissions.
- [ ] Pick per-round winners and record the final recommendation.
- [ ] Record useful rejected/alternative ideas; no unrequested graft or merge.

## Reference

Anthropic documents `claude-opus-5` and standard API rates of $5/MTok input and $25/MTok output: https://platform.claude.com/docs/en/models/opus-5/whats-new-opus-5
