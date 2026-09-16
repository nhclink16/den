# Den Go model comparison: paused, results invalid

Paused at the user's request on 2026-09-14 after their usage screenshot showed 62.1% five-hour, 24.8% weekly, and 12.4% monthly usage. Do not relaunch the batch automatically.

The initial six-model runner used separate git worktrees as subprocess working directories, but model tool calls resolved to the main checkout. The inherited PWD was the main checkout; the precise OpenCode resolution mechanism still needs verification. The workers collided on CLI changes and could not find the review fixture. Their outputs cannot support a fair model ranking.

Batch process 26601 and then-live model process groups 26608, 26609, and 34346 were stopped. A subsequent process check confirmed all were gone. Main-checkout changes attributable to this batch were preserved in invalid-batch-main-checkout.diff, then restored for crates/den-cli/src/main.rs and crates/den-server/tests/api/uploads.rs. The batch-created, unreferenced crates/den-cli/src/read.rs was removed. Final git status showed only the previously created .opencode directory untracked; the evaluated source paths had no diff.

The prior single Muse review and actual subagent handoff were valid smoke tests, but do not establish implementation quality. The six-model comparison has no valid scores yet.

Before any resumption:

- Agree on a much smaller usage budget. One bounded task and one model at a time is the next sensible trial.
- Explicitly set the subprocess PWD and OpenCode --dir, then prove the model's actual read/edit path with a disposable canary before assigning source changes. These are proposed fixes, not yet validated.
- Preserve cost and quota separately. Go models have different allowances; token cost alone does not express the share of the subscription consumed.
- The review fixture's owner-check removal is a real authorization regression. The truncation-removal finding is not established under valid HTTP uploads: the synthetic corruption test appends beyond declared size, which ordinary chunk admission prevents. Do not score that as a second proven regression.
- The independent verification script was interrupted by a shell timeout during baseline compilation. No complete acceptance or mutation results exist. Validate the verifier before using any results.

Files in this directory retain the prompt, configuration, fixed base revision, isolated checkouts, attempted outputs, and draft acceptance scripts for a later controlled rerun.
