"""Narrowed sequential 3-model Den eval. Launched detached; polls results dirs."""
import json, os, subprocess, time
from pathlib import Path
ROOT = Path("/var/folders/4s/y00rkcbd2bb08bkpw776jwyr0000gp/T/opencode/den-go-eval")
MODELS = {"glmflash": "glm-5.3-flash", "deepseek": "deepseek-v4.1-flash", "muse": "muse-spark-1.3-contributor"}
PROMPT = ("Your project directory is your current working directory. Use relative paths for all reads and edits. "
          "The files review-proposal.diff, AGENTS.md, crates/den-cli/src/main.rs, and crates/den-server/tests/api/uploads.rs "
          "are at the root of your checkout. Do not use absolute paths outside your checkout.\n\n") + (ROOT/"prompt.txt").read_text()
CFG = json.dumps({"$schema": "https://opencode.ai/config.json",
  "mcp": {k: {"enabled": False} for k in ["cua-driver", "cua-arch", "railway"]},
  "share": "disabled", "snapshot": False, "lsp": False, "formatter": False,
  "agent": {"eval-worker": {"mode": "all", "description": "Isolated Den coding evaluation worker", "steps": 45,
    "prompt": "Complete the assigned Den tasks. Be precise about what you actually verified. Follow repository conventions. Do not delegate.",
    "permission": {"*": "deny", "read": "allow", "glob": "allow", "grep": "allow", "edit": "allow",
      "bash": {"*": "deny", "cargo *": "allow", "rustfmt *": "allow", "git diff*": "allow", "git status*": "allow"}}}}})

def run_one(name, mid):
    import signal
    tree = ROOT/"worktrees"/name
    dest = ROOT/"results"/name
    dest.mkdir(exist_ok=True)
    env = os.environ.copy()
    for k in ["DEN_URL", "DEN_TOKEN", "DEN_CONFIG", "DEN_PASSWORD", "DEN_INVITE"]:
        env.pop(k, None)
    env.update({"OPENCODE_CONFIG_CONTENT": CFG, "OPENCODE_DISABLE_EXTERNAL_SKILLS": "1",
      "OPENCODE_DISABLE_CLAUDE_CODE_SKILLS": "1", "OPENCODE_PURE": "1",
      "SQLX_OFFLINE": "true", "CARGO_TARGET_DIR": str(ROOT/"target"), "PWD": str(tree)})
    start = time.time()
    with (dest/"events.jsonl").open("w") as o, (dest/"stderr.log").open("w") as e:
        p = subprocess.Popen(["opencode", "run", "--pure", "--agent", "eval-worker",
          "--model", "opencode-go/"+mid, "--format", "json", "--title", f"Den eval {name}",
          "--dir", str(tree), PROMPT],
          cwd=str(tree), env=env, stdout=o, stderr=e, start_new_session=True)
        try:
            code = p.wait(timeout=720)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGTERM)
            try:
                code = p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL)
                code = p.wait()
    (dest/"process.json").write_text(json.dumps(
      {"elapsed_seconds": round(time.time()-start, 2), "returncode": code}, indent=2))

if __name__ == "__main__":
    log = (ROOT/"narrowed.log").open("a")
    def emit(m):
        log.write(m+"\n"); log.flush()
    emit("narrowed.py started pid=%d" % os.getpid())
    for name, mid in MODELS.items():
        if (ROOT/"results"/name/"process.json").exists():
            emit(f"{name} already complete, skipping"); continue
        if (ROOT/"results"/name/"events.jsonl").exists():
            # Adopt an already-running run (e.g. orphaned by a killed launcher):
            # wait for its exit instead of launching a duplicate.
            emit(f"{name} has events but no process.json; waiting for live run")
            deadline = time.time() + 720
            while time.time() < deadline:
                found = subprocess.run(["pgrep", "-f", f"Den eval {name}"],
                                       capture_output=True, text=True).stdout.strip()
                if not found:
                    break
                time.sleep(15)
            emit(f"{name} live run ended; marking externally completed")
            (ROOT/"results"/name/"process.json").write_text(json.dumps(
              {"externally_completed": True}, indent=2))
            continue
    for name, mid in MODELS.items():
        if (ROOT/"results"/name/"process.json").exists():
            emit(f"{name} already complete, skipping"); continue
        emit(f"starting {name}")
        try:
            run_one(name, mid)
        except Exception as ex:
            emit(f"{name} ERROR {ex}")
        emit(f"finished {name}")
    (ROOT/"narrowed.done").write_text("complete\n")
    emit("ALL DONE")
