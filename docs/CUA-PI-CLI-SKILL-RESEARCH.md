# CUA driver via Pi-way CLI + SKILL.md (no MCP) — research

Angle: how to expose a computer-use agent (CUA) driver to agents the same way Den already exposes chat/terminal/canvas — thin CLI over a typed core + `SKILL.md` progressive disclosure + explicit safety gates — and why not to add an MCP server.

Date: 2026-09-15. Sources are primary (official docs, source in this repo, pi package on disk) where noted.

## TL;DR

- Pi already defines the pattern: `bash` + `read` call a CLI; `SKILL.md` teaches when/how; optional TypeScript extension gates `tool_call` with `ctx.ui.confirm/select`. No MCP server needed.
- Den already implements it: `crates/den-cli` thin wrapper over REST+WS, shared types in `crates/den-core`, agent contract in `skills/den/SKILL.md`, safety gates as grants/requests/decide/revoke in `docs/M7B-HOST-BRIEF.md` + `docs/M7B-NOTES.md`. `docs/DESIGN.md` says explicitly: “Skills for Claude Code and Codex wrap the CLI. No MCP server.”
- For computer-use, copy that: a small `computer` CLI (or `den computer …`) over `trycua/cua` / OpenAI `computer-use-preview` / Anthropic `computer_*` drivers, typed actions in `den-core`, a `SKILL.md` section with ask/wait/act + 5s poll rule, and reuse of Den’s `Capability::{TerminalView,TerminalControl}` → `View/Control` + `access request/decide/revoke` + controller promotion + audit. Gate unattended runs with a pi `tool_call` extension (see `permission-gate.ts`) and run untrusted sessions in container per `security.md`/`containerization.md`.

## 1. Pi-way primitives (concrete names)

Source: `~/.local/npm-global/lib/node_modules/@earendil-works/pi-coding-agent/{README.md,docs/skills.md,docs/extensions.md,docs/security.md,docs/containerization.md,examples/extensions/permission-gate.ts}` on this machine.

### 1.1 Skills = CLI + SKILL.md, loaded on demand

- Spec: [Agent Skills standard](https://agentskills.io/specification) — pi implements it (`docs/skills.md`). Frontmatter required: `name` (≤64 chars, `[a-z0-9-]`, no leading/trailing/double hyphen), `description` (≤1024 chars, determines when model loads it).
- Discovery (`docs/skills.md#locations`):
  - Global: `~/.pi/agent/skills/`, `~/.agents/skills/`
  - Project (only after trust): `.pi/skills/`, `.agents/skills/` up to git root
  - Packages: `skills/` dirs or `pi.skills` in `package.json`
  - Settings: `skills: [...]`, CLI: `--skill <path>` (repeatable), `--no-skills` disables discovery (explicit `--skill` still loads).
  - Directories containing `SKILL.md` discovered recursively; in `~/.pi/agent/skills/` + `.pi/skills/` root `.md` with valid frontmatter also counts.
- Runtime (`docs/skills.md#how-skills-work`):
  1. Startup scans names+descriptions into system prompt as XML.
  2. Model uses `read` (or `bash` when `read` unavailable) to load full `SKILL.md`.
  3. Follows instructions; relative paths resolve from skill dir (e.g. `scripts/process.sh`, `references/API.md`).
  4. Invoked as `/skill:name [args]`; args appended as `User: <args>`.
- Den’s instance: `skills/den/SKILL.md` frontmatter `name: den`, env `DEN_URL` (default `http://127.0.0.1:7000`) + `DEN_TOKEN`, “Every command prints JSON.” Sections: `den health/channels/read/send/upload/tail/dm`, `den canvas create/get/patch/rename`, `## Machines and terminals` (`den host list/enroll`, `den access request/grants/revoke/decide`, `den terminal open/write`).

Commands to reuse verbatim:

```bash
den health
den channels
den read general --limit 20
den send general "hello" --reply-to MSG_ID
den upload clips ./run.mp4
den tail general        # JSON lines; message_created/message_edited/resync
den host list
den host enroll         # prints one-time code
den access request HOST_ID control --minutes 60 [--standing]
den access grants
den access revoke GRANT_ID
den access decide REQUEST_ID --allow   # omit --allow = deny
den terminal open HOST_ID --in general
den terminal write SESSION_ID 'printf "hello\n"'
```

Source: `skills/den/SKILL.md`, `crates/den-cli/src/main.rs` (`Cli { url, token, config }`, `Cmd::{Health,Host,Access,Terminal,Canvas,Send,Read,Tail,Upload,…}`), `crates/den-cli/src/hosts.rs` (`HostCmd::{List,Enroll}`, `AccessCmd::{Request,Grants,Revoke,Decide}`, `TerminalCmd::{Open,Write,Close}`).

### 1.2 Extensions = safety gates on `tool_call`

Source: `docs/extensions.md`, `examples/extensions/permission-gate.ts`, `examples/extensions/confirm-destructive.ts`.

Minimal gate (copy `permission-gate.ts`):

```ts
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
export default function (pi: ExtensionAPI) {
  pi.on("tool_call", async (event, ctx) => {
    if (event.toolName !== "bash") return undefined;
    const cmd = event.input.command as string;
    if (/\brm\s+(-rf?|--recursive)/i.test(cmd) || /\bsudo\b/.test(cmd)) {
      if (!ctx.hasUI) return { block: true, reason: "Dangerous command blocked (no UI)" };
      const choice = await ctx.ui.select(`⚠️ Dangerous command:\n\n  ${cmd}\n\nAllow?`, ["Yes","No"]);
      if (choice !== "Yes") return { block: true, reason: "Blocked by user" };
    }
    return undefined;
  });
}
```

Relevant API names (`docs/extensions.md#extensionapi-methods`, `#events`):

- `pi.registerTool({ name, label, description, parameters: Type.Object({…}), execute(toolCallId, params, signal, onUpdate, ctx) })`
- `pi.registerCommand("hello", { description, handler })`
- `pi.on("tool_call", …)`, `pi.on("session_start"/"session_shutdown"/"session_before_switch"/"session_before_fork"/"project_trust", …)`
- `ctx.ui.confirm(title, msg)`, `ctx.ui.select(msg, ["Yes","No"])`, `ctx.ui.input()`, `ctx.ui.notify(msg, "info"|"error")`, `ctx.ui.setStatus/setWidget`, `ctx.hasUI`, `ctx.sessionManager.getEntries()`
- Placement for `/reload`: `~/.pi/agent/extensions/*.ts` (global) or `.pi/extensions/*.ts` (project-local, only after trust). Test with `pi -e ./my-extension.ts`. Trust: `~/.pi/agent/trust.json`, `defaultProjectTrust: "ask"|"always"|"never"`, `--approve/-a` / `--no-approve/-na` override.
- No built-in sandbox (`docs/security.md#no-built-in-sandbox`): `read/write/edit/bash` run as pi’s UID. Isolation must come from OS/VM/container. For unattended work use `docs/containerization.md`: whole-`pi`-in-Docker, Gondolin micro-VM route (`examples/extensions/gondolin/` — overrides `read,write,edit,bash,grep,find,ls`), OpenShell, Docker Sandboxes (`sbx`). Never bind-mount `~/.pi/agent` or API keys you don’t intend to share.

Other pi surfaces to know but not abuse: `pi --mode rpc` (JSONL over stdin/stdout, `prompt/steer`, `bash_execution_update`, split on `\n` only — Node `readline` is non-compliant), `pi -p` / `--mode json` non-interactive, `AgentSession` in `@earendil-works/pi-coding-agent` SDK (`src/core/agent-session.ts`), custom providers via `~/.pi/agent/models.json` (`docs/models.md`, `docs/custom-provider.md`).

## 2. Den precedent: safety gates without MCP

Source: `docs/DESIGN.md` (Agents section), `docs/M7B-HOST-BRIEF.md`, `docs/M7B-NOTES.md`, `crates/den-core/src/host.rs`, `crates/den-server/migrations/0005_hosts.sql`.

Model:

- Tables `hosts(id,owner_id,name,token_hash,online,last_seen)`, `grants(id,host_id,grantee_id,capability,expires_at,created_by,created_at,revoked_at)`, `access requests` + `access_log` (last 200 in Settings → Access).
- Types in `den-core`: `Host`, `HostEnrollment { code, expires_at }`, `HostCredential { server_url, host_id, name, token }`, `Capability::{TerminalView,TerminalControl}` (`as_str() -> "terminal_view"|"terminal_control"`), `Grant{…}`, `RequestAccess{capability,duration_minutes,standing}`, `AccessRequest{id,host_id,host_name,owner_id,requester_id,capability,status,expires_at,grant_id}`, `AccessDecision{allow:bool}`, `HostFrame::{Open,Input,Resize,Close,Output,Exited,Ack,Viewer,Hello,Replay,Scrollback}`, `TerminalFrame`, `TerminalState{active_controller_id,viewer_ids,…}`.
- Endpoints (via CLI above): `POST /hosts/enroll` (10-min one-time code), `GET /hosts`, `POST /hosts/{id}/requests`, `POST /requests/{id}/decide`, `DELETE /grants/{id}` (revoke closes sessions, rejects input <1s), `POST /hosts/{id}/sessions`, `POST /sessions/{id}/write` (`TerminalWrite{text}`), `DELETE /sessions/{id}`, `POST /sessions/{id}/controller {user_id|null}`.
- Rules an agent MUST follow (`skills/den/SKILL.md## Machines and terminals`):
  - “Machines belong to people. Ask for permission before acting on another person’s machine. A chat card or host ID alone is not permission.”
  - `view` for watching, `control` for open+type. Default 1h. `--standing` only if owner asked.
  - Requests appear in owner’s DM (bot badge if agent). Wait for `access_decided allowed` on `den tail` before acting. Never poll REST faster than every 5s. Grant ≠ controller — owner must promote (`active_controller_id`); stop on denial/expiry/revocation.
  - Owner holds all caps implicitly; viewer recheck every 500ms; input recheck on write (403 on forged input — verified in `scripts/m7b-smoke.mjs`).
- Transport notes: `den-host login <code>` → `~/.config/den/host.toml` mode 600; `den-host run` dials `wss://<server>/hosts/ws`, binary frames = UTF-8 JSON `HostFrame`, 16ms coalesce, 1MiB unacked window + `Ack`, 256KiB scrollback/PTY, PTY survives socket drop (resize nudge reattaches). Direct Tailscale shortcut: host listens on Tailscale iface only, random port, 60s tickets (refresh 50s), 700ms fallback to relay, `direct` label when active. Recording: protected upload, NDJSON `[ms,base64]` with `CSI 8;rows;cols t` marker, cap 64MiB.

Why no MCP (`docs/DESIGN.md`, `AGENTS.md` “No plugin system yet… keep client store and API surface clean”): one small REST+WS surface, JSON on stdout, works identically for human (`bash`), Claude Code skill, Codex plugin, Hermes `adapter.py` (`BasePlatformAdapter`), OpenClaw channel. No persistent tool-server permissions to audit.

## 3. CUA drivers — concrete repos, commands, API names

All are primary upstream; pin before building.

### 3.1 `trycua/cua` — the closest to a “driver” (macOS/Win/Linux/Docker)

- Repo: https://github.com/trycua/cua
- Packages: `pip install cua-computer cua-agent` (PyPI `cua-computer`, `cua-agent`); computer loop in `libs/computer`, agent loop in `libs/agent`.
- Core class: `from cua_computer import Computer` — `await computer.screenshot()` (PNG bytes), `computer.mouse.move(x,y)/click(x,y,button)/drag(path)`, `computer.keyboard.press(keys)/type(text)`, display info for scaling. Backends: Lume VM (macOS, `lume` CLI), Docker Ubuntu (`cua` Docker image), host OS.
- Agent: `from cua_agent import CUAAgent` / `agent.run(task)` — pluggable LLM: OpenAI `computer-use-preview`, Anthropic computer-use, UI-TARS / OMNI grounding. Telemetry + blocklist hooks live in agent loop — the place to insert Den-style gates.
- Use as: subprocess/daemon behind CLI; do NOT expose its Python API directly to the model.

### 3.2 OpenAI computer-use-preview — model + tool spec

- Docs: https://platform.openai.com/docs/guides/tools-computer-use
- Model: `computer-use-preview` (Responses API `tools: [{ type: "computer_use_preview", display_width, display_height, environment: "mac"|"windows"|"ubuntu"|"browser" }]`).
- Actions (`computer_call.action`): `click(x,y,button="left"|"right"|"middle")`, `double_click(x,y)`, `drag(path:[{x,y}…])`, `keypress(keys:[])`, `move(x,y)`, `screenshot()`, `scroll(x,y,scroll_x,scroll_y)`, `type(text)`, `wait(ms)`.
- Safety: response includes `pending_safety_checks[]` (`{id,code,message}`); caller MUST acknowledge via `acknowledged_safety_checks` on next turn or get human approval before proceeding. This maps 1:1 to Den `access_decided` — treat unacknowledged check as denied.
- Coordinates 0-1000 scaled to `display_width/height` — CLI must do the scaling, not the model.

### 3.3 Anthropic computer-use — tool spec + reference harness

- Docs: https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/computer-use-tool
- Tools: `computer_20250124` / `computer_20250319` (`{ name:"computer", display_width_px, display_height_px, display_number? }`) + `text_editor_20250124` + `bash_20250124` in the reference loop.
- Actions: `key(text)`, `type(text)`, `mouse_move(x,y)`, `left_click(x,y)`, `right_click`, `middle_click`, `double_click`, `triple_click`, `left_mouse_down/up`, `scroll(x,y,direction,amount)`, `hold_key(text,duration)`, `wait(duration)`, `screenshot()`, `cursor_position()`.
- Reference: https://github.com/anthropics/anthropic-quickstarts/tree/main/computer-use-demo — Docker Ubuntu+X11+VNC (`docker run`), `pyautogui` driver, `loop.py` (screenshot→model→action). Copy its loop, replace `pyautogui` with `cua-computer` and its approve-step with Den grants.

### 3.4 Grounding / benchmarks (pick one, pin version)

- OmniParser: https://github.com/microsoft/omniparser — `get_screenshot_interactive()` → parse `(x,y,icon/text)` for models without native grounding. Use when LLM is UI-TARS-class.
- UI-TARS Desktop: https://github.com/bytedance/UI-TARS-desktop (open agent desktop + SDK).
- OSWorld: https://github.com/xlang-ai/OSWorld (Ubuntu/Windows benchmark + env harness) — for eval, not prod driver.
- UFO: https://github.com/microsoft/UFO (Windows agent) — reference for allowlist/blocklist patterns.

## 4. Proposed Pi-way CUA wiring (no MCP)

### 4.1 Thin CLI over the driver (JSON on stdout, like `den`)

New binary `den-computer` (or `den computer …`) in workspace, deps `tokio, cua bindings/managed subprocess, den-core`. Never hand-roll input injection — call OS/driver APIs (` Agents.md: “Crypto and protocol handling are exempt: never hand-roll those”` analogue for input).

```bash
den-computer screenshot --display 0 --cursor > shot.png
# -> { "png": "<path>", "width": 1024, "height": 768, "cursor": {"x":512,"y":384} }
den-computer click 512 384 --button left
den-computer double-click 512 384
den-computer drag --path '[{"x":100,"y":100},{"x":300,"y":300}]'
den-computer move 512 384
den-computer scroll 512 384 --dx 0 --dy -240
den-computer type --text 'hello' --enter
den-computer keypress --keys '["ctrl","c"]'
den-computer wait --ms 500
den-computer approve CHECK_ID   # acknowledge OpenAI pending_safety_checks
den-computer deny CHECK_ID --reason "purchases need owner"
```

Rules: coordinates are display pixels (CLI scales 0-1000↔pixels); every mutating command prints `{ ok, action_id, screenshot_hash }`; destructive set (`type/keypress/click/drag` on another person’s host) requires grant (see 4.2); output is JSON so `den tail`-style `resync` + `den read` pattern works.

### 4.2 Safety gates = reuse Den grants, not a new authz

- Capabilities: reuse `Capability::{TerminalView→ScreenView, TerminalControl→ComputerControl}` or add `ComputerView/ComputerControl` in `den-core` (new migration `0006_computer.sql`, old migrations immutable per `AGENTS.md`).
- Flow (mirror `skills/den/SKILL.md` machines section):
  ```bash
  den access request HOST_ID control --minutes 60   # or view
  den tail                          # wait for access_decided allowed + request ID
  den access grants
  den-computer screenshot           # view cap suffices
  den-computer click …              # needs control + active_controller_id == you
  den access revoke GRANT_ID        # owner or grantee; input stops <1s
  ```
- SKILL.md must state: host card ≠ permission; `control` grant ≠ controller (owner promotes via `POST /sessions/{id}/controller`); stop on denial/expiry/revocation; never poll faster than 5s; refetch screenshot after `resync`; ignore own `computer_output` events.
- Server enforcement (mirror M7b): check grant on every mutating action, recheck viewers every 500ms, audit every request/decide/promote/revoke/type to `access_log`, record session (`recording_upload_id` analogue = action log + screenshots, cap like 64MiB).
- Pi local gate (defense in depth): ship `computer-gate.ts` extension modelled on `permission-gate.ts`:
  ```ts
  pi.on("tool_call", async (e, ctx) => {
    if (e.toolName !== "bash") return;
    const c = e.input.command as string;
    if (!c.startsWith("den-computer ")) return;
    if (/(click|drag|type|keypress) /.test(c) && !approvedThisSession) {
      if (!ctx.hasUI) return { block: true, reason: "computer control needs human approval" };
      const ok = await ctx.ui.confirm("Allow computer action?", c);
      if (!ok) return { block: true, reason: "Denied by user" };
    }
  });
  ```
  Plus `protected-paths.ts`-style blocklist (no password managers, no purchase/accept dialogs — deny-list announced in SKILL.md).

### 4.3 Why not MCP (explicit)

- MCP = persistent tool server with broad, opaque JSON-schema tools the model can call without shell audit trail; permissions live in MCP config, separate from Den grants/audit/cards. Debugging = MCP logs, not `den tail` JSONL.
- CLI+SKILL = same surface for human, pi `bash`, Claude Code, Codex, Hermes `BasePlatformAdapter`: `DEN_URL`+`DEN_TOKEN`, JSON on stdout, `tail`+`resync`, fileable audit. Matches `docs/DESIGN.md` “No MCP server” and `AGENTS.md` “No new dependencies for things under 50 lines” + “Shared types go in den-core first.”
- If an MCP client is ever required, generate it FROM the CLI/OpenAPI (`/openapi.json` via `utoipa`), never as the primary surface.

## 5. Minimal build order (if pursued)

1. `den-core`: `ComputerAction` enum (`Screenshot,Click,Drag,Move,Scroll,Type,Keypress,Wait`) + `ComputerEvent`, OpenAPI via `utoipa`, TS types regenerated (per `AGENTS.md`: types in `den-core` first).
2. `den-computer` binary wrapping `cua-computer` (screenshot/mouse/keyboard) + OpenAI/Anthropic action mapping + coordinate scaling; JSON stdout; short branch (touches `den-core` types — per `AGENTS.md` tell other engineer before merging).
3. Server: `0006_computer.sql` (grants reuse + action log), endpoints + 500ms recheck + <1s revoke + audit; integration test per M7b pattern (view cannot act, control can after promotion, revoke stops, expired denied, outsider 404).
4. `skills/den/SKILL.md`: `## Computer use` section with ask/wait/act, 5s rule, `resync`→re-screenshot, OpenAI `pending_safety_checks`→`den-computer approve/deny` mapping.
5. `computer-gate.ts` pi extension + container recipe (`containerization.md` pattern) for unattended runs.
6. Eval on OSWorld tasks + smoke script modelled on `scripts/m7b-smoke.mjs` (screenshots to `docs/shots/cua-*.png`, notes to `docs/CUA-NOTES.md`).

## Sources

- Den: `docs/DESIGN.md` (Agents/No-MCP), `AGENTS.md` (den-core-first, migrations immutable, explicit-path staging), `skills/den/SKILL.md`, `crates/den-cli/src/main.rs`, `crates/den-cli/src/hosts.rs`, `crates/den-core/src/host.rs`, `crates/den-server/migrations/0005_hosts.sql`, `docs/M7B-HOST-BRIEF.md`, `docs/M7B-NOTES.md`, `scripts/m7b-smoke.mjs`.
- Pi (on-disk package `@earendil-works/pi-coding-agent`): `README.md`, `docs/skills.md` (Agent Skills https://agentskills.io/specification), `docs/extensions.md` (`pi.on("tool_call")`, `pi.registerTool/Command`, `ctx.ui.*`), `docs/security.md`, `docs/containerization.md`, `docs/rpc.md`, `docs/models.md`, `examples/extensions/permission-gate.ts`, `examples/extensions/confirm-destructive.ts`, `examples/extensions/gondolin/`, `examples/extensions/protected-paths.ts`.
- CUA upstream: https://github.com/trycua/cua (`cua-computer`, `cua-agent`), https://platform.openai.com/docs/guides/tools-computer-use (`computer-use-preview`, `computer_use_preview`, `pending_safety_checks`/`acknowledged_safety_checks`), https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/computer-use-tool (`computer_20250124`/`computer_20250319`), https://github.com/anthropics/anthropic-quickstarts/tree/main/computer-use-demo, https://github.com/microsoft/omniparser, https://github.com/bytedance/UI-TARS-desktop, https://github.com/xlang-ai/OSWorld.
