# Muse/OpenCode skills audit

Verified September 14, 2026, installed OpenCode 1.18.31. Read-only audit; no live settings changed.

## Latest arena

All four frozen native transcripts were inspected. Neither contestant called a skill tool or read a SKILL.md. Both read project guidance. Arena launchers deliberately disabled external skills and customizations. The comparison does not establish how either model performs with the shared skill library active.

## Normal native discovery

`opencode --pure debug skill` from Den reports 51 skills, including the built-in customize-opencode. `--pure` suppresses external plugins but retains native skill discovery. Full discovery receipts: discovered.json and default-discovered.json. The audit shell had Claude compatibility disabled; explicitly removing those inherited disable flags in a subprocess still yielded 51 skills. No persistent environment settings were changed.

The verification, TDD, debugging and Context7 skills resolve to the canonical ~/.agents/skills library. Only 4 skills have direct OpenCode-local folders, but this is not the available-skill count: OpenCode also reads ~/.agents and ~/.claude skills. Profile symlink targets do not hide canonical files from native discovery. Codex-shaped skills are therefore visible too; visibility is not tool compatibility.

No ~/.config/opencode/AGENTS.md exists. The global OpenCode configuration has no instructions list or custom agent prompt/skills activation. OpenCode documents fallback to ~/.claude/CLAUDE.md, whose current delegation section sends substantial implementation/debugging/review to GPT-5.6 Sol. This fallback applies when Claude compatibility is enabled; it is not active in the initial audit subprocess with compatibility disabled. A dedicated OpenCode worker instruction file would avoid that conditional inherited routing. The two live OpenCode processes inspected had no matching compatibility-disable flags, but were not tied to a specific scored pane.

## Recommended policy

Use a concise OpenCode worker contract as always-loaded instructions: test real user-visible behavior; restore the actual original bug without changing the test; make minimal scoped changes; label execution versus inference; report concrete evidence and remaining limitations.

The short principle-prove-it-works skill is the strongest always-on candidate. Require diagnosing-bugs on repairs, TDD on behavior changes, and Context7 when unfamiliar/version-sensitive APIs are used. Keep design/browser/domain skills task-specific. Do not make arena/interrogate automatic for every worker task. The current full TDD skill demands user confirmation of test boundaries, so do not inject it globally unchanged; an already explicit task boundary should be honored without a redundant prompt.

Next useful evaluation: a fresh bug comparing Muse with this worker policy against plain Muse. Skill loading alone is not proof of improved test behavior.

Sources: https://opencode.ai/docs/skills/ and https://opencode.ai/docs/rules/; Context7 /anomalyco/opencode; local skill contents, profiles.toml and global Claude instructions.

## Portability notes

The canonical profile calls several skills Codex-only, but normal OpenCode native discovery nevertheless lists them from ~/.agents/skills. Discovery does not validate required tools. OpenCode documentation also says unrecognized frontmatter fields are ignored; manual-invocation settings from another client should not be assumed portable. Keep orchestration-heavy arena/interrogate explicit rather than automatically loaded in every worker.
