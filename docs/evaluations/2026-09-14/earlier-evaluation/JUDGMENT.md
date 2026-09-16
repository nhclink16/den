# Den model judgment: Muse vs GLM Flash

September 14, 2026. Judgment of these preserved submissions, not a general model benchmark.

## Verdict

**Muse Spark 1.3 Contributor wins this round.** Both candidates' implementations passed the independent functional checks. Muse submitted all three deliverables, passed formatting, and used roughly one quarter of the reported token cost. Its review still needs supervision: one finding is valid and the other is overstated.

GLM-5.3-Flash produced working code and useful tests. Its run ended after a disk-full linker failure followed by OpenCode errors. Missing deliverables make its submission weaker, but this is not evidence that it could not finish on a healthy machine.

DeepSeek V4.1 Flash is excluded, not scored zero. It produced no code, tokens, or tool calls. No further DeepSeek requests were made for this judgment.

## Results

| Check | GLM-5.3-Flash | Muse Spark 1.3 Contributor |
|---|---|---|
| Independent CLI behavior checks | 19/19 passed | 19/19 passed |
| Upload integration suite | 4/4 passed | 4/4 passed |
| Core upload bugs caught by submitted tests | 6/6 | 6/6 |
| Rejecting the valid maximum upload size | Missed | Caught |
| Accepting one byte above the maximum | Caught | Missed |
| Submitted CLI unit tests | None | 2 passed; both proven able to fail |
| Formatting of changed files | Failed | Passed |
| Review deliverable | Missing | 1 valid finding, 1 overclaim |
| Summary deliverable | Missing | Present |
| Reported token cost | $0.09105 | $0.02100 |

Reported costs are OpenCode event metrics, not separately verified invoices. GLM's event span was about 10 minutes before interruption; Muse's process ran for 216 seconds. Timing is not a fair speed benchmark because the machine ran out of storage and builds shared a cache during the original runs.

## What actually worked

Both CLIs correctly handled backward and forward pagination, exclusive cursors, empty and exact-multiple results, page size one, a later-page HTTP error without printing a partial success array, a repeated full page without hanging, single channel/DM resolution, and argument rejection before HTTP requests. Legacy single-page behavior also passed.

Both sets of upload tests assert exact error codes and statuses, no admission rows/files after rejection, the five-pending limit, separate owner allowance, and freeing a slot after completion. Each catches six deliberate server regressions. Neither covers both exact maximum-size boundaries. Their new admission and quota tests failed on the appropriate mutants and passed again after restoration.

Muse's two CLI unit tests fail when their validation/sort helper is deliberately broken, then pass after restoration. They do not test the paginator itself. The independent HTTP-fixture checks provide that evidence.

## Review judgment

Muse correctly identified that replacing `owned()` with `load()` in the chunk handler removes ownership and visibility checks. An independently rebuilt mutant allowed another member's PATCH where the original returned 403. The reproduction must use the current offset; using zero after Alice has advanced the upload would produce an offset conflict instead.

Its second claimed issue is not established under the stated HTTP behavior. The reproduction manually grows a size-10 upload file to 15 bytes. The real chunk handler caps writes at the declared size and serializes them. The synthetic corruption test fails without truncation, but that alone does not establish a reachable crash regression. The existing bounded crash-resume test still passes. The additional claim that trailing garbage could be served is false: completion checks the stored length and downloads require completion.

The cache-control change was correctly left unflagged. Removing `private` while retaining `no-store` was not shown to introduce a correctness regression.

## Code quality and reporting

GLM's implementation is shorter, but places the full paging loop inside the command match, scans the accumulated map for cursors, and accidentally joins the password helper's opening line to its first statement. The latter is formatting damage, not a demonstrated functional bug.

Muse separates the paginator, but repeats cursor checks and sorts/deduplicates the result twice. Its completion report also claimed a clean post-format check without an observed second check in the original events. The final artifact does pass the judge's fresh formatting check. Neither candidate's original logs prove a red/green exercise of its new tests; the judge supplied that verification.

For the next bounded implementation trial, I would choose Muse and keep an independent reviewer. The tested code correctness is a tie; completion and reported cost decide this submission comparison. I would not use this run to conclude GLM is a worse coder in general.

## Verification and preservation

The initial multi-model batch and the interrupted shared-cache verification are excluded. The shared cache had returned Muse's unit tests while checking GLM. This run used a fresh `judge-target`, disabled incremental/debug artifacts, and cleaned the candidate package before every build or test while retaining only dependency caching. GLM then correctly reported zero CLI unit tests and Muse reported two.

Both candidates remain on base `8b2adfc89e5ff798edd3d8df6aab046b36534b13`. Before/after source and diff hashes match. Mutations were applied only in the disposable verification checkout and restored. No candidate fix was credited as model work. No changes were applied to Den main, committed, or merged by this judging task.

Reproduce with `python3 judge_verify.py`, followed by `python3 judge_extra.py`, in the evaluation directory. These scripts modify only its disposable verification checkout and test artifacts. The generated build cache may be deleted after a run without deleting evidence.

## Evidence

- [Build isolation and candidate hashes](/private/var/folders/4s/y00rkcbd2bb08bkpw776jwyr0000gp/T/opencode/den-go-eval/judge-results/provenance.json)
- [GLM verification](/private/var/folders/4s/y00rkcbd2bb08bkpw776jwyr0000gp/T/opencode/den-go-eval/judge-results/glmflash/verification.json)
- [Muse verification](/private/var/folders/4s/y00rkcbd2bb08bkpw776jwyr0000gp/T/opencode/den-go-eval/judge-results/muse/verification.json)
- [Boundary and unit-test mutation checks](/private/var/folders/4s/y00rkcbd2bb08bkpw776jwyr0000gp/T/opencode/den-go-eval/judge-results/extra-verification.json)
- [Review mutation checks](/private/var/folders/4s/y00rkcbd2bb08bkpw776jwyr0000gp/T/opencode/den-go-eval/judge-results/baseline/verification.json)
- [GLM original events](/private/var/folders/4s/y00rkcbd2bb08bkpw776jwyr0000gp/T/opencode/den-go-eval/results/glmflash/events.jsonl)
- [Muse original events](/private/var/folders/4s/y00rkcbd2bb08bkpw776jwyr0000gp/T/opencode/den-go-eval/results/muse/events.jsonl)
- [Assigned task](/private/var/folders/4s/y00rkcbd2bb08bkpw776jwyr0000gp/T/opencode/den-go-eval/prompt.txt)
