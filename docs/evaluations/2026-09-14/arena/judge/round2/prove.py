from pathlib import Path
import difflib, json, os, shutil, signal, subprocess, time
ROOT=Path('/tmp/den-arena-20260914/judge/round2-work')
OUT=Path('/tmp/den-arena-20260914/judge/round2')
BASE=Path('/tmp/den-arena-20260914/baseline')
FILES=['crates/den-server/src/ws.rs','crates/den-server/src/auth.rs']
ORIG={f:(BASE/f).read_text() for f in FILES}
WS,AUTH=FILES

def replace(s,a,b):
    assert s.count(a)==1, (a,s.count(a))
    return s.replace(a,b)

def fixture(parts):
    v=ORIG.copy()
    if 'read' in parts:
        v[WS]=replace(v[WS],'''        Event::ReadStateUpdated { user_id, state } => {
            if user_id != &a.user.id {
                return false;
            }
            Some(&state.channel_id)
        }''','''        Event::ReadStateUpdated { state, .. } => Some(&state.channel_id),''')
    if 'csrf' in parts:
        v[AUTH]=replace(v[AUTH],'if origin != Some(&s.origin) || hash(csrf) != row.csrf_hash {','if origin != Some(&s.origin) && hash(csrf) != row.csrf_hash {')
    if 'sort' in parts:
        v[WS]=replace(v[WS],'    online_user_ids.sort();','    online_user_ids.sort_unstable();')
    if 'cookie' in parts:
        v[AUTH]=replace(v[AUTH],'''                v.split(';')
                    .find_map(|p| p.trim().strip_prefix("den_session="))''','''                v.split(';')
                    .map(str::trim)
                    .find_map(|p| p.strip_prefix("den_session="))''')
    if 'negative_sort' in parts:
        v[WS]=replace(v[WS],'    online_user_ids.sort();','    online_user_ids.sort_by(|a, b| b.cmp(a));')
    if 'negative_cookie' in parts:
        v[AUTH]=replace(v[AUTH],'p.trim().strip_prefix("den_session=")','p.strip_prefix("den_session=")')
    return v

def write(parts):
    for f,s in fixture(parts).items(): (ROOT/f).write_text(s)

def diff(parts,name):
    v=fixture(parts)
    (OUT/name).write_text(''.join(''.join(difflib.unified_diff(ORIG[f].splitlines(keepends=True),v[f].splitlines(keepends=True),fromfile='a/'+f,tofile='b/'+f)) for f in FILES))

ENV=os.environ.copy();ENV.update(CARGO_TARGET_DIR='/tmp/den-arena-20260914/judge/round2-target',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='3')
TESTS={
 'read':'arena_review::read_state_events_are_owner_only_in_shared_channel',
 'csrf':'auth::auth_consumes_invites_protects_cookies_and_revokes_credentials',
 'sort':'arena_review::presence_order_remains_sorted_and_unique_across_tabs',
 'cookie':'arena_review::cookie_parser_preserves_trim_and_first_matching_cookie',
}
MATRIX=[
 ('baseline',[],[]),
 ('read-only',['read'],['read']),
 ('csrf-only',['csrf'],['csrf']),
 ('sort-only',['sort'],[]),
 ('cookie-only',['cookie'],[]),
 ('negative-sort',['negative_sort'],['sort']),
 ('negative-cookie',['negative_cookie'],['cookie']),
 ('proposal',['read','csrf','sort','cookie'],['read','csrf']),
 ('restored',[],[]),
]
def build(label):
    assert shutil.disk_usage(ROOT).free>=8*1024**3, 'Storage below 8 GiB'
    log=OUT/(label+'-build.log')
    with log.open('w') as handle:
        p=subprocess.Popen(['cargo','test','-p','den-server','--test','api','--no-run','--message-format=json'],cwd=ROOT,env=ENV,stdout=handle,stderr=subprocess.STDOUT,start_new_session=True)
        while p.poll() is None:
            if shutil.disk_usage(ROOT).free<8*1024**3:
                os.killpg(p.pid,signal.SIGTERM);p.wait();raise RuntimeError('Stopped build: below 8 GiB free')
            time.sleep(1)
    assert p.returncode==0, str(log)
    for line in log.read_text().splitlines():
        try: item=json.loads(line)
        except ValueError: continue
        if item.get('reason')=='compiler-artifact' and item.get('target',{}).get('name')=='api' and item.get('executable'): binary=item['executable']
    return binary

if __name__=='__main__':
    harness=ROOT/'crates/den-server/tests/api.rs'
    hidden=ROOT/'crates/den-server/tests/api/arena_review.rs'
    original_harness=(BASE/'crates/den-server/tests/api.rs').read_text()
    harness.write_text(original_harness+'\n#[path = "api/arena_review.rs"]\nmod arena_review;\n')
    hidden.write_text((OUT/'arena_review.rs').read_text())
    results={} 
    try:
        for label,parts,expected_failed in MATRIX:
            write(parts)
            binary=build(label)
            results[label]={}
            for key,test in TESTS.items():
                with (OUT/f'{label}-{key}.log').open('w') as log:
                    run=subprocess.run([binary,'--exact',test,'--nocapture'],cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,timeout=90)
                results[label][key]=run.returncode
            (OUT/'results.json').write_text(json.dumps(results,indent=2)+'\n')
            failed=[k for k,v in results[label].items() if v!=0]
            print(label,results[label],flush=True)
            assert failed==expected_failed,(label,failed,expected_failed)
    finally:
        write([])
        harness.write_text(original_harness)
        hidden.unlink(missing_ok=True)
    binary=build('warm-clean-baseline')
    listed=subprocess.check_output([binary,'--list'],text=True)
    assert 'arena_review::' not in listed
    (OUT/'clean-target-tests.txt').write_text(listed)
    diff(['read','csrf','sort','cookie'],'review-proposal.diff')
    for key in ['read','csrf','sort','cookie']:
        diff([key],key+'-only.diff')
