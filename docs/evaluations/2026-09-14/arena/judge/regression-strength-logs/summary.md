# Frozen repair regression-test strength

Independent, anonymous assessment. No model self-reports or native session logs were read.

| Candidate | Healthy | Header drain in actual loop | Actual guard changed to only 403 | Restored |
| --- | --- | --- | --- | --- |
| A | 3/3 pass | 3/3 pass, regression missed | 3/3 pass, regression missed | 3/3 pass |
| B | 1/1 pass | 0/1 pass, regression caught | 0/1 pass, regression caught | 1/1 pass |

A's three tests exercise extracted helpers. The helpers and tests were left unchanged while the actual tail loop was regressed. All tests remained green, so they do not protect the repaired callsites against either original defect.

B launches the actual CLI against a local WebSocket server. The header-drain mutant failed with missing second-connection authorization. The 403-only guard mutant failed its explicit deadline because the CLI kept reconnecting after a 401. Neither failure was a compiler error or an external supervisor timeout.

Each discovered test ran separately with a 40-second process-group timeout. No supervisor timeout fired. B's guard mutant failed after approximately 22.4 seconds, including its own 20-second exit deadline. Both candidates' restored suites passed. No frozen files or assertions were modified. The final judge CLI copy matches frozen B, and no test CLI processes remain.

Evidence is in ../regression-strength.json. This directory contains exact mutation diffs, build logs, per-test output, and the rerunnable run.py script. Each source and target was isolated from root's parallel hidden checks. Builds used a separate CARGO_TARGET_DIR, debug 0, no incremental compilation, three jobs, locked/offline dependency resolution, and an 8 GiB disk guard.
