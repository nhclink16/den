# Final review accounting

Frozen review sessions only. Earlier unused Opus max-preflight session excluded. No inference requests made for accounting.

| Metric | Muse Spark 1.3 Contributor | Opus 5 high |
| --- | ---: | ---: |
| Task-only token cost | $0.00700031 | $0.82278800 |
| Whole-session token cost | $0.00758538 | $0.82278800 |
| Task responses with usage | 14 | 18 |
| Whole-session responses with usage | 16 | 18 |
| Task uncached input | 43052 | 36 |
| Task output including reasoning | 9717 | 10527 |
| Task cache read | 375854 | 415766 |
| Task cache write | 0 | 35155 |
| Review elapsed seconds | 103.223 | 111.412 |

Claude amounts are API-equivalent under Max; Muse amounts are OpenCode-reported token costs under Go. Neither is a billed-charge claim. Muse task-only excludes its readiness check. Opus high session contains only the review task, so whole-session and task-only match.

## Model and effort evidence

- Actual response metadata stayed `opencode-go/muse-spark-1.3-contributor` and `claude-opus-5`. No observed fallback.
- Every Opus response records `high`. Muse was **configured xhigh; review variant not recorded in native metadata**. Verified both `configs/review-muse-opencode.json` and `judge/review-muse-resolved-config.json` set the provider model's `options.reasoningEffort` to `xhigh`. This confirms configuration, not provider-confirmed effort.
- Claude usage repeats across content-block rows and was deduplicated by message ID. Muse message totals match its native session totals. Thinking is included in Claude output but separately counted for Muse.
- Opus price calculation uses TTL-specific cache writes and [official standard pricing](https://platform.claude.com/docs/en/about-claude/pricing). All reported native Muse message costs reproduce from the cached Go model rates.

## Raw task timing

| UTC timestamp | Muse | Opus |
| --- | --- | --- |
| Review prompt | 2026-09-14T21:02:37.949Z | 2026-09-14T21:02:39.090Z |
| Final answer complete | 2026-09-14T21:04:21.172Z | 2026-09-14T21:04:30.502Z |

No harness interruption appears within the scored review turns. Each duration starts with that candidate's prompt.

[review-accounting.json](review-accounting.json) contains source hashes, per-request usage, exact message/turn IDs, native totals, and by-turn costs.
