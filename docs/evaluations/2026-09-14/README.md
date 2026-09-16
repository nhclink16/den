# WIP — unreviewed model-evaluation preservation

Standing down September 16, 2026. This branch preserves this lane’s September14 evaluation work, fixtures, test evidence, native transcripts, reports and skills audit. **Not a production implementation or an approval to merge. Some fixtures intentionally contain bugs.**

No new development, tests, cleanup or live configuration changes were started during preservation. The ios-cleanup lane owns local worktree/artifact cleanup. Main and unrelated `.opencode/agents/muse-review.md` were untouched.

## Pushed WIP source branches

| Branch | Commit | Preserved work |
| --- | --- | --- |
| `wip/unreviewed/eval-deepseek-20260916` | `98087aca352092668e3cd948a9c0fc9e8f52932c` | review-proposal.diff |
| `wip/unreviewed/eval-glm-20260916` | `ca869fd200d15c03751e2f76d3446dfc4dfee7bb` | review-proposal.diff |
| `wip/unreviewed/eval-glmflash-20260916` | `12e873bf88591e58712df8c85cbf5d04c499c555` | crates/den-cli/src/main.rs, crates/den-server/tests/api/uploads.rs, review-proposal.diff |
| `wip/unreviewed/eval-kimi-20260916` | `6707103f2ad1aca869a2c214a5cca4cc8867d931` | review-proposal.diff |
| `wip/unreviewed/eval-luna-20260916` | `3a7f405a802bb86ddf1405d09625b4919bf0e760` | review-proposal.diff |
| `wip/unreviewed/eval-mimo-20260916` | `f413271eafe3b0e230e34d054ce616a1ec2cc62b` | review-proposal.diff |
| `wip/unreviewed/eval-muse-20260916` | `5e5872e9d5daf7073b9bd979afa6748482af692b` | crates/den-cli/src/main.rs, crates/den-server/tests/api/uploads.rs, EVAL_REVIEW.md, EVAL_SUMMARY.md, review-proposal.diff |
| `wip/unreviewed/eval-verify-20260916` | `190ce45ab26ada216100494bf85869b6daa6a377` | crates/den-cli/src/main.rs, crates/den-server/tests/api.rs, crates/den-server/tests/api/uploads.rs, crates/den-server/tests/api/eval_review.rs |
| `wip/unreviewed/arena-repair-muse-20260916` | `29a2d9bbdfce0a223da6571b1dd0dd11913c9b0a` | crates/den-cli/src/stream.rs |
| `wip/unreviewed/arena-repair-opus-20260916` | `94174168e8b677e874a42f9bd1735c95526bfcfd` | crates/den-cli/src/stream.rs, crates/den-cli/tests/tail.rs |
| `wip/unreviewed/arena-review-muse-20260916` | `d624329f6ca2a399a834d08a865351104a7f6fc4` | Previously committed shared review fixture, with unreviewed WIP marker |

The review-opus snapshot has the same prepared seed commit as review-muse; that shared seed is an ancestor of the pushed review branch. Candidate-empty earlier worktrees preserve the supplied review proposal, not a completed model submission.

## Artifacts

- `arena/SCORECARD.md`: final matched-arena results. Repair used Opus max; review used Opus high at the user’s direction.
- `arena/judge/`: behavior proofs, mutation logs, blind reviews, frozen responses and native transcripts, usage accounting and skills audit.
- `earlier-evaluation/`: previous Go comparisons, interrupted/invalid runs clearly retained as such, final corrected judgment and verification.
- Root judgment and scope documents preserve the original boundaries.

Rebuildable Cargo targets, Python environments, OpenCode caches/auth storage, duplicate full baseline/source trees and cache bytecode are excluded. Source changes live on the pushed branches above. Skill-discovery receipts omit repeated skill bodies but retain discovery metadata; those bodies belong to the canonical skill library, not this evaluation. Local temporary binary symlinks are not deliverables and were not committed. Scripts retain original absolute paths as provenance and may need path adaptation to rerun elsewhere.

No lane-owned unique source or evaluation evidence intentionally remains only on the local disk.
