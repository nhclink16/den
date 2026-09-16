# Evaluation paused for storage cleanup

User requested pause on September 14, 2026. No model ranking finalized.

- DeepSeek original run: zero tokens/tools, killed after 720 seconds. Tiny direct request timed out after 30 seconds; China region gate remains unconfirmed. No opt-in setting changed. See deepseek-diagnostic.json.
- GLM original logs contain disk-full linker failure followed by OpenCode errors. Do not count this as clean model capability failure.
- Shared Cargo target cache returned Muse test names during GLM verification. Any current verify-* logs are invalid/incomplete. Rebuild candidate packages reliably before judging.
- Verification process interrupted. verify.py finally restored server uploads source. Verification checkout retains temporary candidate/test files; do not treat its state as a candidate.
- Storage cleanup deleted shared evaluation target only, not worktrees/results. Also removed npm/Bun download caches, inactive /private/tmp/den-m4-debug/target, and 21 old Xcode test-product bundles while preserving all result-bundles and seven recent bundles. Receipt: /tmp/den-storage-cleanup-receipt.json.
- Read-only reviewer provisionally preferred Muse artifact completeness, but its second confirmed truncation finding is unsupported under reachable bounded HTTP writes. Authorization bypass finding is valid. GLM has no C review or summary. Both A/B implementations require fresh independent verification.
- Main checkout source was not edited by this task.
