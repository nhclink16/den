# Round 2 preparation receipt

Ready for the parent to launch. No model sessions were started here.

## Proof

The nine-state matrix in results.json matched expectations. Baseline and restored baseline pass all four checks. Each isolated bug fails only its relevant existing test. Each harmless change passes all checks. Descending-sort and untrimmed-cookie negative controls each fail only their new regression check. The combined proposal fails both bug checks and passes both harmless checks.

The supplemental direct read-state test passes baseline, fails the isolated mutant with an explicit socket-owner/user-ID mismatch, and passes restored code. All three new behavior tests were demonstrated capable of failure. Existing assertions were not weakened or adapted.

The judge working copy now byte-matches the archived baseline. Main was not edited. Server targets and source checkouts were kept separate throughout. Storage guard interruptions are retained in logs and are preparation events, not contestant failures.

## Prepared contestants

- Source folders: /tmp/den-arena-20260914/review-muse and /tmp/den-arena-20260914/review-opus.
- Target folders: /tmp/den-arena-20260914/target-review-muse and /tmp/den-arena-20260914/target-review-opus.
- Both sources have the identical sole seed commit: 4447411536bb233f6656becfeaeb4122d4359bc4.
- All 676 source/input file hashes match. Snapshots are clean. The generated initial commit was amended and its old unreachable object pruned, leaving no clean parent to revert.
- Both targets were COW-cloned from the cleaned baseline target. den-server and den-core were cleaned from each clone, then compiled freshly from that contestant's own source.
- Both normal API binaries list exactly 37 tests and no hidden judge tests. Executable paths, source manifests, distinct inodes and hashes are recorded in review-warm-receipt.json.
- Warm command: cargo test -p den-server --test api --no-run --locked --offline, with separate CARGO_TARGET_DIR, dev/test debug 0, incremental off, and 3 jobs.
- Current free storage: 11.22 GiB at receipt creation.

## Files to use

Share candidate-prompt.md and the snapshot's review-proposal.diff. The prompt requires AGENTS.md first, read-only source work, at most three findings, and final-response output rather than REVIEW.md. Do not expose answer-key.md, proof files or judge logs.

The clean judge target remains at /tmp/den-arena-20260914/judge/round2-target. It can be removed as a rebuildable match cache after the parent accepts the warmed-target receipt.
