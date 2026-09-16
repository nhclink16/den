"""Independent artifact checks; models never receive this file or its tests."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import run
from check_cli import check

ROOT = run.ROOT
TREE = ROOT/'worktrees'/'verify'
SERVER = TREE/'crates/den-server/src/uploads.rs'
BASE_SOURCE = subprocess.check_output(['git','show',f'{run.BASE}:crates/den-server/src/uploads.rs'],cwd=run.REPO,text=True)

def command(args, logfile, timeout=180):
    start = time.time()
    with logfile.open('w') as log:
        try:
            p = subprocess.run(args, cwd=TREE, env=run.environment(), stdout=log, stderr=subprocess.STDOUT, timeout=timeout)
            return {'returncode':p.returncode,'seconds':round(time.time()-start,2),'log':str(logfile)}
        except subprocess.TimeoutExpired:
            return {'returncode':-1,'seconds':round(time.time()-start,2),'log':str(logfile),'timeout':True}

def original_upload_tests():
    return subprocess.check_output(['git','show',f'{run.BASE}:crates/den-server/tests/api/uploads.rs'],cwd=run.REPO)

def baseline():
    dest = ROOT/'results'/'baseline'
    dest.mkdir(exist_ok=True)
    (TREE/'crates/den-server/tests/api/eval_review.rs').write_bytes((ROOT/'review_checks.rs').read_bytes())
    api = TREE/'crates/den-server/tests/api.rs'
    original = subprocess.check_output(['git','show',f'{run.BASE}:crates/den-server/tests/api.rs'],cwd=run.REPO,text=True)
    api.write_text(original+'\n#[path = "api/eval_review.rs"]\nmod eval_review;\n')
    SERVER.write_text(BASE_SOURCE)
    (TREE/'crates/den-server/tests/api/uploads.rs').write_bytes(original_upload_tests())
    result = {'original_review_checks': command(['cargo','test','-p','den-server','--test','api','review_holdout','--','--nocapture'],dest/'original-review.log')}
    # Verify each changed operation separately, rather than crediting every suspicious diff.
    SERVER.write_text(BASE_SOURCE.replace('    let row = owned(&s, &a, &id).await?;\n    if row.complete ||','    let row = load(&s, &id).await?;\n    if row.complete ||',1))
    result['owner_bypass'] = command(['cargo','test','-p','den-server','--test','api','review_holdout_other_member','--','--nocapture'],dest/'owner-bypass.log')
    SERVER.write_text(BASE_SOURCE.replace('    file.set_len(offset as u64).await?;\n','',1))
    result['no_truncate_synthetic_corruption'] = command(['cargo','test','-p','den-server','--test','api','review_holdout_resume','--','--nocapture'],dest/'no-truncate-corruption.log')
    result['no_truncate_existing_resume_test'] = command(['cargo','test','-p','den-server','--test','api','upload_resume_survives','--','--nocapture'],dest/'no-truncate-valid-resume.log')
    SERVER.write_text(BASE_SOURCE)
    result['cli_build'] = command(['cargo','build','-p','den'],dest/'cli-build.log')
    if result['cli_build']['returncode'] == 0:
        result['cli_checks'] = check(ROOT/'target/debug/den')
    (dest/'verification.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({k:v for k,v in result.items() if k!='cli_checks'},indent=2),flush=True)

def mutants():
    return {
        'zero_size_accepted': BASE_SOURCE.replace('v.size <= 0 || v.size > s.max_upload','v.size < 0 || v.size > s.max_upload',1),
        'oversize_accepted': BASE_SOURCE.replace('v.size <= 0 || v.size > s.max_upload','v.size <= 0',1),
        'filenames_unchecked': BASE_SOURCE.replace('if v.filename.is_empty()','if false && (v.filename.is_empty()',1).replace(".any(|c| c.is_control() || c == '/' || c == '\\\\')",".any(|c| c.is_control() || c == '/' || c == '\\\\'))",1),
        'six_pending_allowed': BASE_SOURCE.replace('if pending >= 5 {','if pending > 5 {',1),
        'quota_shared_by_all_owners': BASE_SOURCE.replace('if pending >= 5 {','if sqlx::query_scalar::<_, i64>("SELECT count(*) FROM uploads WHERE complete=0").fetch_one(&s.db).await? >= 5 {',1),
        'completed_consume_quota': BASE_SOURCE.replace('if pending >= 5 {','if sqlx::query_scalar::<_, i64>("SELECT count(*) FROM uploads WHERE owner_id=?").bind(&a.user.id).fetch_one(&s.db).await? >= 5 {',1),
    }

def verify(name):
    dest = ROOT/'results'/name
    candidate = ROOT/'worktrees'/name
    # Stable verification source paths allow Rust's dependency build cache to be reused.
    for folder in ['src','tests']:
        source = candidate/'crates/den-cli'/folder
        target = TREE/'crates/den-cli'/folder
        if target.exists():
            shutil.rmtree(target)
        if source.exists():
            shutil.copytree(source,target)
    SERVER.write_text(BASE_SOURCE)
    (TREE/'crates/den-server/tests/api/uploads.rs').write_bytes((candidate/'crates/den-server/tests/api/uploads.rs').read_bytes())
    result = {'model':run.MODELS[name]}
    result['cli_tests'] = command(['cargo','test','-p','den'],dest/'verify-cli-tests.log')
    result['cli_build'] = command(['cargo','build','-p','den'],dest/'verify-cli-build.log')
    if result['cli_build']['returncode']==0:
        result['cli_checks'] = check(ROOT/'target/debug/den')
    result['upload_baseline'] = command(['cargo','test','-p','den-server','--test','api','uploads::','--','--nocapture'],dest/'verify-uploads.log')
    result['mutations'] = {}
    if result['upload_baseline']['returncode']==0:
        for mutant,source in mutants().items():
            SERVER.write_text(source)
            tested = command(['cargo','test','-p','den-server','--test','api','uploads::','--','--nocapture'],dest/f'mutant-{mutant}.log')
            log = Path(tested['log']).read_text()
            tested['compiled'] = 'Running tests/api.rs' in log
            tested['killed'] = tested['compiled'] and tested['returncode'] != 0 and 'test result: FAILED' in log
            result['mutations'][mutant] = tested
    SERVER.write_text(BASE_SOURCE)
    (dest/'verification.json').write_text(json.dumps(result,indent=2))
    print(json.dumps({'name':name,'cli_passed':sum(c['passed'] for c in result.get('cli_checks',[])), 'cli_total':len(result.get('cli_checks',[])), 'mutations_killed':sum(v['killed'] for v in result['mutations'].values()),'mutations_total':len(result['mutations'])}),flush=True)

if __name__=='__main__':
    if sys.argv[1]=='baseline':
        baseline()
    elif sys.argv[1]=='launch':
        with (ROOT/'verification.log').open('w') as log:
            p=subprocess.Popen([sys.executable,__file__,'batch'],stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        print(f'Verification PID {p.pid}')
    elif sys.argv[1]=='batch':
        for name in run.MODELS:
            while not (ROOT/'results'/name/'metrics.json').exists():
                time.sleep(5)
            verify(name)
        (ROOT/'verification.done').write_text('complete\n')
    else:
        verify(sys.argv[1])
