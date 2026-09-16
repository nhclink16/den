"""Reproducible, isolated OpenCode Go comparison on a fixed Den revision."""
import concurrent.futures
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent
REPO = Path('/Users/nicholascaron/Projects/personal/den')
BASE = '8b2adfc89e5ff798edd3d8df6aab046b36534b13'
MODELS = {
    'muse': 'muse-spark-1.3-contributor',
    'glm': 'glm-5.3',
    'kimi': 'kimi-k3',
    'luna': 'gpt-5.6-luna',
    'deepseek': 'deepseek-v4.1-flash',
    'mimo': 'mimo-v2.5',
}
PROMPT = '''You are doing a matched evaluation on a disposable Den checkout. Read AGENTS.md and the relevant source. Complete these three tasks. No other agents, network research, commits, infrastructure, or changes outside the assigned files. Do not inspect git history. Do not apply review-proposal.diff.

A. Implement `den read CHANNEL --all` in crates/den-cli. Keep current one-page behavior without --all. With --all, fetch every matching page and print ONE JSON array in ascending message-ID order, with no duplicates. Existing --limit remains the page size, 1..200. No cursor means walk backwards from newest to oldest; --before is an exclusive upper bound and walks backwards; --after is an exclusive lower bound and walks forwards. Server responses are ascending within each page; before/no-cursor chooses the newest matching page, after chooses the oldest matching page. Reject --before with --after and limits outside 1..200 before any HTTP request, for both modes. Resolve channel names/@users once. On a later-page HTTP error, exit nonzero without a partial success array on stdout. If a full page repeats without cursor progress, fail promptly rather than hang. Use existing den-core types; no server API changes or new dependencies. Add useful CLI tests if practical. Allowed edits for A: crates/den-cli/src/* and crates/den-cli/tests/* only.

B. Add behavior-level integration tests to crates/den-server/tests/api/uploads.rs using the existing Test harness. Cover POST /uploads size bounds and invalid filenames, and the per-owner five-pending-upload limit. Prove rejected admission creates no upload row or file, another owner has an independent allowance, and completing an upload frees a slot. Assert exact HTTP status/error codes, not 'any error'. Keep this concise. No server implementation changes, migrations, dependency changes, or existing test deletions. Run relevant tests if possible.

C. Review review-proposal.diff as a proposed change against the current server implementation. Write EVAL_REVIEW.md with at most three actionable correctness findings. Give the affected line/operation, concrete reproduction, expected versus actual behavior, and a regression-test idea. Distinguish a confirmed issue from a hypothesis; don't flag harmless changes or optional features. Do not fix or apply this patch.

Run targeted checks. In EVAL_SUMMARY.md, give your changes, exact checks/outcomes, remaining concerns, and any task left unfinished. You have a 12-minute wall-clock limit and 45 model turns. Prefer completed, small changes over broad refactoring. Tool access is limited to repository reads/edits and cargo, rustfmt, git diff/status. The checkout has no node_modules and no production data. Do not touch apps/ios or den-core. Begin now.'''

def config():
    return {
        '$schema': 'https://opencode.ai/config.json',
        'mcp': {k: {'enabled': False} for k in ['cua-driver', 'cua-arch', 'railway']},
        'share': 'disabled', 'snapshot': False, 'lsp': False, 'formatter': False,
        'agent': {'eval-worker': {
            'mode': 'all', 'description': 'Isolated Den coding evaluation worker',
            'steps': 45,
            'prompt': 'Complete the assigned Den tasks. Be precise about what you actually verified. Follow repository conventions. Do not delegate.',
            'permission': {
                '*': 'deny', 'read': 'allow', 'glob': 'allow', 'grep': 'allow',
                'edit': 'allow',
                'bash': {'*': 'deny', 'cargo *': 'allow', 'rustfmt *': 'allow',
                         'git diff*': 'allow', 'git status*': 'allow'},
            },
        }},
    }

def environment():
    env = os.environ.copy()
    for key in ['DEN_URL', 'DEN_TOKEN', 'DEN_CONFIG', 'DEN_PASSWORD', 'DEN_INVITE']:
        env.pop(key, None)
    env.update({
        'OPENCODE_CONFIG_CONTENT': json.dumps(config()),
        'OPENCODE_DISABLE_EXTERNAL_SKILLS': '1',
        'OPENCODE_DISABLE_CLAUDE_CODE_SKILLS': '1',
        'OPENCODE_PURE': '1',
        'SQLX_OFFLINE': 'true',
        'CARGO_TARGET_DIR': str(ROOT / 'target'),
    })
    return env

def prepare():
    import difflib
    (ROOT / 'worktrees').mkdir(exist_ok=True)
    (ROOT / 'results').mkdir(exist_ok=True)
    source = subprocess.check_output(['git', 'show', f'{BASE}:crates/den-server/src/uploads.rs'], cwd=REPO, text=True)
    changed = source.replace('    let row = owned(&s, &a, &id).await?;\n    if row.complete ||', '    let row = load(&s, &id).await?;\n    if row.complete ||', 1)
    changed = changed.replace('    // A crash may leave bytes beyond the committed offset. Retries replace them.\n    file.set_len(offset as u64).await?;\n', '', 1)
    changed = changed.replace('"private, no-store".parse().unwrap()', '"no-store".parse().unwrap()', 1)
    patch = ''.join(difflib.unified_diff(source.splitlines(True), changed.splitlines(True), fromfile='a/crates/den-server/src/uploads.rs', tofile='b/crates/den-server/src/uploads.rs'))
    (ROOT / 'review-proposal.diff').write_text(patch)
    (ROOT / 'prompt.txt').write_text(PROMPT)
    (ROOT / 'config.json').write_text(json.dumps(config(), indent=2))
    for name in [*MODELS, 'verify']:
        dest = ROOT / 'worktrees' / name
        if not dest.exists():
            subprocess.run(['git', 'worktree', 'add', '--detach', str(dest), BASE], cwd=REPO, check=True)
        if name in MODELS:
            (dest / 'review-proposal.diff').write_text(patch)
    (ROOT / 'manifest.json').write_text(json.dumps({'base': BASE, 'models': MODELS, 'timeout_seconds': 720, 'max_parallel': 3, 'reasoning': 'provider defaults; no explicit variant', 'opencode': subprocess.check_output(['opencode', '--version'], text=True).strip()}, indent=2))
    print('Prepared six model checkouts plus independent verification checkout.')

def summarize(name):
    dest = ROOT / 'results' / name
    events = []
    for line in (dest / 'events.jsonl').read_text().splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        events.append(event)
    finishes = [e['part'] for e in events if e.get('type') == 'step_finish']
    tools = [e['part'] for e in events if e.get('type') == 'tool_use']
    metrics = json.loads((dest / 'process.json').read_text())
    metrics.update({
        'model': 'opencode-go/' + MODELS[name],
        'cost_usd_reported': sum(p.get('cost', 0) for p in finishes),
        'steps': len(finishes),
        'input_tokens': sum(p.get('tokens', {}).get('input', 0) for p in finishes),
        'output_tokens': sum(p.get('tokens', {}).get('output', 0) for p in finishes),
        'reasoning_tokens': sum(p.get('tokens', {}).get('reasoning', 0) for p in finishes),
        'cached_input_tokens': sum(p.get('tokens', {}).get('cache', {}).get('read', 0) for p in finishes),
        'tool_calls': len(tools),
        'tool_errors': [{'tool': p.get('tool'), 'error': p.get('state', {}).get('error')} for p in tools if p.get('state', {}).get('status') == 'error'],
        'errors': [e for e in events if e.get('type') == 'error'],
        'session_ids': sorted({e['sessionID'] for e in events if 'sessionID' in e}),
    })
    (dest / 'metrics.json').write_text(json.dumps(metrics, indent=2))
    text = '\n\n'.join(e['part']['text'] for e in events if e.get('type') == 'text')
    (dest / 'response.md').write_text(text)
    tree = ROOT / 'worktrees' / name
    (dest / 'tracked.diff').write_bytes(subprocess.check_output(['git', 'diff', '--no-ext-diff'], cwd=tree))
    (dest / 'status.txt').write_bytes(subprocess.check_output(['git', 'status', '--short'], cwd=tree))
    print(json.dumps(metrics), flush=True)

def run_one(name):
    dest = ROOT / 'results' / name
    dest.mkdir(exist_ok=True)
    start = time.time()
    with (dest / 'events.jsonl').open('w') as out, (dest / 'stderr.log').open('w') as err:
        p = subprocess.Popen(['opencode', 'run', '--pure', '--agent', 'eval-worker', '--model', 'opencode-go/' + MODELS[name], '--format', 'json', '--title', f'Den matched eval {name}', PROMPT], cwd=ROOT/'worktrees'/name, env=environment(), stdout=out, stderr=err, start_new_session=True)
        timed_out = False
        try:
            code = p.wait(timeout=720)
        except subprocess.TimeoutExpired:
            import signal
            timed_out = True
            os.killpg(p.pid, signal.SIGTERM)
            try:
                code = p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL)
                code = p.wait()
    (dest / 'process.json').write_text(json.dumps({'elapsed_seconds': round(time.time()-start, 2), 'returncode': code, 'timed_out': timed_out}, indent=2))
    summarize(name)

def batch():
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        list(pool.map(run_one, MODELS))
    (ROOT / 'batch.done').write_text('complete\n')

if __name__ == '__main__':
    action = sys.argv[1]
    if action == 'prepare':
        prepare()
    elif action == 'launch':
        with (ROOT / 'batch.log').open('w') as log:
            p = subprocess.Popen([sys.executable, __file__, 'batch'], stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        print(f'Batch running as PID {p.pid}; artifacts: {ROOT}')
    elif action == 'batch':
        batch()
    elif action == 'summarize':
        summarize(sys.argv[2])
