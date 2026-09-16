"""Fresh, package-clean verification of the two preserved candidate submissions."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import shutil
import subprocess
import time
import verify

ROOT = verify.ROOT
TARGET = ROOT / 'judge-target'
OUT = ROOT / 'judge-results'
OUT.mkdir(exist_ok=True)
original_env = verify.run.environment

def environment():
    env = original_env()
    env.update(CARGO_TARGET_DIR=str(TARGET), CARGO_PROFILE_DEV_DEBUG='0',
               CARGO_PROFILE_TEST_DEBUG='0', CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='4')
    return env

verify.run.environment = environment
verify.run.MODELS['glmflash'] = 'glm-5.3-flash'

def command(args, logfile, timeout=600):
    start = time.monotonic()
    logfile = OUT / logfile.parent.name / logfile.name
    logfile.parent.mkdir(exist_ok=True)
    with logfile.open('w') as log:
        # Candidate packages are rebuilt for every operation. Dependencies are shared.
        if '-p' in args:
            package = args[args.index('-p')+1]
            clean = subprocess.run(['cargo','clean','-p',package],cwd=verify.TREE,
                                   env=environment(),stdout=log,stderr=subprocess.STDOUT)
            if clean.returncode:
                return {'returncode':clean.returncode,'log':str(logfile),'clean_failed':True}
        log.write('COMMAND '+repr(args)+'\n'); log.flush()
        try:
            p = subprocess.run(args,cwd=verify.TREE,env=environment(),stdout=log,
                               stderr=subprocess.STDOUT,timeout=timeout)
            result = {'returncode':p.returncode,'seconds':round(time.monotonic()-start,2),'log':str(logfile)}
        except subprocess.TimeoutExpired:
            result = {'returncode':-1,'timeout':True,'log':str(logfile)}
    print(logfile.parent.name+'/'+logfile.name, result['returncode'], flush=True)
    return result

verify.command = command

def digest(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

def snapshot(name):
    p = ROOT/'worktrees'/name
    return {'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=p,text=True).strip(),
            'diff_sha256':hashlib.sha256(subprocess.check_output(['git','diff','--no-ext-diff'],cwd=p)).hexdigest(),
            'cli_sha256':digest(p/'crates/den-cli/src/main.rs'),
            'uploads_test_sha256':digest(p/'crates/den-server/tests/api/uploads.rs')}

before = {name:snapshot(name) for name in ['glmflash','muse']}
(OUT/'provenance.json').write_text(json.dumps({'candidates_before':before,'target':str(TARGET),
    'cache_policy':'clean candidate package before every build/test; debug=0; incremental=0'},indent=2))
# Paths in the old verifier's black-box runner are redirected to this fresh target.
old_check = verify.check
verify.check = lambda ignored: old_check(TARGET/'debug/den')
# Restore untouched CLI for baseline checks.
base_cli = subprocess.check_output(['git','show',f'{verify.run.BASE}:crates/den-cli/src/main.rs'],cwd=verify.run.REPO)
try:
    (verify.TREE/'crates/den-cli/src/main.rs').write_bytes(base_cli)
    verify.baseline()
    shutil.copy2(ROOT/'results/baseline/verification.json', OUT/'baseline/verification.json')
    for name in ['glmflash','muse']:
        with contextlib.redirect_stdout(io.StringIO()): verify.run.summarize(name)
        shutil.copy2(ROOT/'results'/name/'metrics.json',OUT/f'{name}-metrics.json')
        verify.verify(name)
        shutil.copy2(ROOT/'results'/name/'verification.json',OUT/name/'verification.json')
        # Real restored source must pass after the mutation loop.
        restored = command(['cargo','test','-p','den-server','--test','api','uploads::','--','--nocapture'],OUT/name/'restored-uploads.log')
        data=json.loads((OUT/name/'verification.json').read_text())
        data['restored_uploads']=restored
        (OUT/name/'verification.json').write_text(json.dumps(data,indent=2))
finally:
    verify.SERVER.write_text(verify.BASE_SOURCE)
    after={name:snapshot(name) for name in ['glmflash','muse']}
    data=json.loads((OUT/'provenance.json').read_text());data['candidates_after']=after;data['candidates_unchanged']=before==after
    data['server_mutations_restored']=verify.SERVER.read_text()==verify.BASE_SOURCE
    (OUT/'provenance.json').write_text(json.dumps(data,indent=2))
print('VERIFICATION COMPLETE',flush=True)
