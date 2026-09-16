from pathlib import Path
import json,os,signal,subprocess,shutil,time,difflib,hashlib
ROOT=Path('/tmp/den-arena-20260914')
WORK=ROOT/'judge/regression-strength-work'
TARGET=ROOT/'judge/regression-strength-target'
OUT=ROOT/'judge/regression-strength-logs'
RESULT=ROOT/'judge/regression-strength.json'
CLI=WORK/'crates/den-cli'
STREAM=CLI/'src/stream.rs'
ENV=os.environ.copy();ENV.update(CARGO_TARGET_DIR=str(TARGET),CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='3')
SEED=(ROOT/'judge/seeded-stream.rs').read_text()
start=SEED.index('    let mut delay = 1;',SEED.index('pub fn tail('))
end=SEED.index('        match tungstenite::connect(request) {',start)
HEADER_SEED=SEED[start:end]

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def inventory(p):return {str(f.relative_to(p)):sha(f) for f in p.rglob('*') if f.is_file()}

def build(label):
    assert shutil.disk_usage(WORK).free>=8*1024**3,'Storage below 8 GiB'
    log_path=OUT/(label+'-build.log')
    with log_path.open('w') as log:
        p=subprocess.Popen(['cargo','test','-p','den','--no-run','--locked','--offline','--message-format=json'],cwd=WORK,env=ENV,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
        while p.poll() is None:
            if shutil.disk_usage(WORK).free<8*1024**3:
                os.killpg(p.pid,signal.SIGTERM);p.wait();raise RuntimeError('Stopped build below 8 GiB')
            time.sleep(.5)
    assert p.returncode==0,str(log_path)
    tests=[];local=[]
    for line in log_path.read_text().splitlines():
        try:i=json.loads(line)
        except ValueError:continue
        if i.get('reason')!='compiler-artifact':continue
        if i['target']['name'] in ['den_core','den','tail']:
            assert Path(i['manifest_path']).resolve().is_relative_to(WORK.resolve()),i['manifest_path']
            local.append({'target':i['target']['name'],'manifest_path':i['manifest_path'],'fresh':i['fresh']})
        if i.get('executable') and i['profile']['test']:
            binary=Path(i['executable'])
            assert binary.resolve().is_relative_to(TARGET.resolve())
            listing=subprocess.check_output([str(binary),'--list'],text=True,timeout=10)
            names=[line.removesuffix(': test') for line in listing.splitlines() if line.endswith(': test')]
            tests.extend((str(binary),name) for name in names)
    assert tests,'No candidate tests discovered'
    return tests,local

def execute(label,tests):
    rows=[]
    for number,(binary,name) in enumerate(tests):
        log_path=OUT/f'{label}-test-{number}.log'
        started=time.monotonic()
        with log_path.open('w') as log:
            p=subprocess.Popen([binary,'--exact',name,'--nocapture'],cwd=WORK,env=ENV,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            try:code=p.wait(timeout=40);timed_out=False
            except subprocess.TimeoutExpired:
                os.killpg(p.pid,signal.SIGKILL);p.wait();code=None;timed_out=True
        rows.append({'name':name,'binary':binary,'exit_code':code,'timed_out':timed_out,'seconds':round(time.monotonic()-started,3),'log':str(log_path)})
    return rows

results={'purpose':'Independent callsite-level mutation test of frozen anonymous candidates; no model reports used','timeout_seconds_per_test':40,'candidates':{}}
for candidate in ['A','B']:
    source=ROOT/f'judge/frozen/repair/{candidate}/source/crates/den-cli'
    original_inventory=inventory(source)
    shutil.rmtree(CLI)
    shutil.copytree(source,CLI)
    for p in CLI.rglob('*'):
        if p.is_file():os.utime(p,None)
    with (OUT/(candidate+'-clean.log')).open('w') as log:
        subprocess.run(['cargo','clean','-p','den'],cwd=WORK,env=ENV,stdout=log,stderr=subprocess.STDOUT,check=True)
    original=STREAM.read_text()
    tail_start=original.index('pub fn tail(')
    tail_end=original.index('\npub fn upload(',tail_start)
    prefix=original[:tail_start];suffix=original[tail_end:]
    row={'source':str(source),'source_sha256':original_inventory,'variants':{}}
    results['candidates'][candidate]=row
    try:
        for variant in ['healthy','header-drain','auth-only-403','restored']:
            mutated=original
            if variant=='header-drain':
                a=mutated.index('    let mut delay = 1;',tail_start)
                b=mutated.index('        match tungstenite::connect(request) {',a)
                mutated=mutated[:a]+HEADER_SEED+mutated[b:]
            elif variant=='auth-only-403':
                old='if is_auth_rejection(response.status().as_u16()) =>' if candidate=='A' else 'if matches!(response.status().as_u16(), 401 | 403) =>'
                assert mutated.count(old)==1
                mutated=mutated.replace(old,'if response.status().as_u16() == 403 =>')
            assert mutated[:mutated.index('pub fn tail(')]==prefix,'Helper prefix changed'
            assert mutated[mutated.index('\npub fn upload('):]==suffix,'Upload/tests changed'
            STREAM.write_text(mutated)
            label=candidate+'-'+variant
            (OUT/(label+'.diff')).write_text(''.join(difflib.unified_diff(original.splitlines(True),mutated.splitlines(True),fromfile='candidate/stream.rs',tofile='mutant/stream.rs')))
            tests,artifacts=build(label)
            outcome=execute(label,tests)
            row['variants'][variant]={'source_sha256':sha(STREAM),'tests':outcome,'local_artifacts':artifacts,'all_passed':all(r['exit_code']==0 and not r['timed_out'] for r in outcome)}
            RESULT.write_text(json.dumps(results,indent=2)+'\n')
            print(candidate,variant,[(r['name'],r['exit_code'],r['timed_out']) for r in outcome],flush=True)
            if variant in ['healthy','restored']:assert row['variants'][variant]['all_passed'],label+' must pass'
    finally:
        STREAM.write_text(original)
        assert inventory(source)==original_inventory,'Frozen source changed'
        assert inventory(CLI)==original_inventory,'Candidate source not restored'
    row['frozen_source_unchanged']=True
    row['judge_cli_restored']=True
    RESULT.write_text(json.dumps(results,indent=2)+'\n')
results['free_bytes']=shutil.disk_usage(WORK).free
results['completed']=True
RESULT.write_text(json.dumps(results,indent=2)+'\n')
print('Complete; frozen sources unchanged and judge CLI restored to B',flush=True)
