# Installing the Den skill

The skill is one file, `SKILL.md`. Point your agent at it and give it a token.

Claude Code:

```
ln -s ~/den/skills/den ~/.claude/skills/den
```

Codex CLI reads the same format from its skills directory:

```
ln -s ~/den/skills/den ~/.codex/skills/den
```

Then in the agent's environment:

```
export DEN_URL=https://your.den.example
export DEN_TOKEN=...   # from Settings → Agents, or `den bot create`
```

Build the CLI with `cargo build -p den --release` and put `target/release/den` on the PATH.
