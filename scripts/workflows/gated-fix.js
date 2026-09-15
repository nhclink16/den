export const meta = {
  name: 'den_gated_fix',
  description: 'Fix chosen issues in isolated worktrees, gated on the test suite actually passing',
  phases: [
    { title: 'Fix', detail: 'one worktree per issue, iterate until tests pass' },
    { title: 'Review', detail: 'independent read of each diff that passed' },
    { title: 'Report', detail: 'what landed as a PR, what was abandoned, and why' },
  ],
}

// args: { issues: [{ number, title, hint? }], openPrs?: boolean }
// Nothing here picks its own work. A human chose these.
const issues = args?.issues || []
const openPrs = args?.openPrs !== false
if (!issues.length) {
  return 'No issues supplied. Pass args.issues as [{number, title, hint}] — this workflow deliberately does not choose its own work.'
}

// The whole point: a model that can run things should be judged on whether the thing runs.
const CHECKS = [
  'cargo fmt --all -- --check',
  'cargo test --workspace',
  'npm --prefix apps/web run check',
  'node --experimental-strip-types --test scripts/streams-source.test.ts scripts/m7c-layout.test.ts scripts/mentions.test.ts scripts/login-pattern.test.ts',
]

const RULES = `
Ground rules, from CLAUDE.md and from what this project has already learned the hard way:

- Write the tests a careful human would write. One integration test per behaviour that matters,
  a unit test where the logic is genuinely tricky. Do not test getters or what the type system
  already guarantees. Volume is not the goal.
- Stage explicit paths. Never "git add -A": other agents share this repository.
- Shared types belong in crates/den-core first; never hand-duplicate a type in the client.
- Do not edit .github/workflows or any file in crates/den-server/migrations.
- Do not touch anything outside the issue you were given. No drive-by refactors.
- If the issue turns out to be wrong, or not worth fixing, say so and stop. Reporting that an
  issue is invalid is a good outcome, not a failure.`

phase('Fix')
const attempts = await parallel(
  issues.map((issue) => () =>
    gate(
      async (feedback, attempt) =>
        agent(
          `Fix issue #${issue.number} in Den: ${issue.title}
${issue.hint ? `\nMaintainer note: ${issue.hint}` : ''}
${attempt > 1 ? `\nYour previous attempt did not pass the checks. What failed:\n${feedback}\n\nFix the cause, not the symptom.` : ''}

Read the issue with: gh issue view ${issue.number} --repo nhclink16/den

${RULES}

When the change is complete, run every one of these from the repository root and make them all pass:
${CHECKS.map((c) => `  ${c}`).join('\n')}

Then report, as plain text:
  SUMMARY: one sentence on what you changed and why.
  FILES: the paths you touched.
  CHECKS: the exact final output line of each command above.
  RISK: anything you are unsure about, or "none".

Do not claim a check passed without having run it. If you cannot make them pass, say so plainly
and describe where you got stuck — that is more useful than a change that does not build.`,
          {
            label: `fix:#${issue.number}${attempt > 1 ? `:retry${attempt}` : ''}`,
            phase: 'Fix',
            tier: 'big',
            isolation: 'worktree',
            keepWorktree: true,
          },
        ),
      // The validator is the gate. A claim of success is not success.
      async (value) => {
        const text = String(value || '')
        if (/cannot|could not|stuck|gave up|unable to/i.test(text) && !/CHECKS:/.test(text)) {
          return { ok: false, feedback: 'The agent reported it could not finish. Treat as a failed attempt.' }
        }
        const missing = CHECKS.filter((c) => !text.includes(c.split(' ')[0]))
        if (missing.length) return { ok: false, feedback: `No evidence these were run: ${missing.join(', ')}` }
        if (/error\[E\d+\]|test result: FAILED|FAILED\b|\bfailed\b/i.test(text)) {
          return { ok: false, feedback: 'The reported output still contains a failure. All checks must pass.' }
        }
        return { ok: true }
      },
      { attempts: 3 },
    ).then((r) => ({ issue, ok: r.ok, attempts: r.attempts, value: r.value })),
  ),
)

const passed = attempts.filter(Boolean).filter((a) => a.ok)
const failed = attempts.filter(Boolean).filter((a) => !a.ok)
log(`${passed.length} of ${issues.length} reached a green suite; ${failed.length} did not`)

phase('Review')
const reviewed = await parallel(
  passed.map((a) => () =>
    agent(
      `Independently review the fix for issue #${a.issue.number} (${a.issue.title}).

The agent that wrote it reported:
${a.value}

Read the actual diff in its worktree with git, do not rely on that summary. Judge three things:
1. Does it fix the stated issue, or does it fix a symptom?
2. Does it break anything adjacent? Check callers and existing tests.
3. Is the test it added one a careful human would have written, or is it padding?

Answer VERDICT: SHIP or VERDICT: REWORK on its own line, then your reasoning in a few sentences.
Shipping something subtly wrong is far worse than asking for rework.`,
      { label: `review:#${a.issue.number}`, phase: 'Review', tier: 'big' },
    ).then((verdict) => ({ ...a, verdict })),
  ),
)

const ship = reviewed.filter(Boolean).filter((r) => /VERDICT:\s*SHIP/i.test(String(r.verdict)))

if (openPrs) {
  phase('Review')
  await parallel(
    ship.map((r) => () =>
      agent(
        `Open a pull request for the fix to issue #${r.issue.number} from its worktree branch.

Push the branch and open the PR with gh. The body must state what the issue was, what you changed,
the exact check output proving the suite passes, and anything left unverified.

Do NOT merge it. Do NOT enable auto-merge. A human reviews before anything lands on main.`,
        { label: `pr:#${r.issue.number}`, phase: 'Review', tier: 'medium' },
      ),
    ),
  )
}

phase('Report')
return {
  requested: issues.length,
  greenSuite: passed.length,
  shipped: ship.map((r) => r.issue.number),
  reworkNeeded: reviewed.filter((r) => !/VERDICT:\s*SHIP/i.test(String(r.verdict))).map((r) => ({
    issue: r.issue.number,
    verdict: String(r.verdict).slice(0, 400),
  })),
  neverPassedChecks: failed.map((a) => ({
    issue: a.issue.number,
    attempts: a.attempts,
    lastReport: String(a.value || '').slice(0, 400),
  })),
}
