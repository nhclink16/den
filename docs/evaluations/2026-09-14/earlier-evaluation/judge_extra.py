"""Boundary and submitted-unit-test checks, in verification checkout only."""
import json, os, shutil, subprocess
from pathlib import Path
import verify
R=verify.ROOT; T=verify.TREE; O=R/'judge-results'; S=verify.SERVER
E=verify.run.environment(); E.update(CARGO_TARGET_DIR=str(R/'judge-target'),CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='4')

def run(name,label,package,filter):
    dest=O/name/(label+'.log')
    with dest.open('w') as out:
        subprocess.run(['cargo','clean','-p',package],cwd=T,env=E,stdout=out,stderr=subprocess.STDOUT,check=True)
        cmd=['cargo','test','-p',package]
        if package=='den-server': cmd+=['--test','api']
        cmd += [filter,'--','--nocapture']
        p=subprocess.run(cmd,cwd=T,env=E,stdout=out,stderr=subprocess.STDOUT,timeout=180)
    text=dest.read_text(); result={'returncode':p.returncode,'compiled':'Running ' in text,'failed_tests':[l for l in text.splitlines() if l.startswith('test ') and 'FAILED' in l],'log':str(dest)}
    print(name,label,result['returncode'],flush=True)
    return result

result={}
cli=T/'crates/den-cli/src/main.rs'
cli_original=cli.read_bytes()
try:
    for name in ['glmflash','muse']:
        shutil.copyfile(R/'worktrees'/name/'crates/den-server/tests/api/uploads.rs',T/'crates/den-server/tests/api/uploads.rs')
        result[name]={}
        for label,wrong in [('reject_valid_maximum','v.size <= 0 || v.size >= s.max_upload'),('accept_max_plus_one','v.size <= 0 || v.size > s.max_upload + 1')]:
            mutated=verify.BASE_SOURCE.replace('v.size <= 0 || v.size > s.max_upload',wrong,1)
            assert mutated!=verify.BASE_SOURCE
            S.write_text(mutated)
            result[name][label]=run(name,label,'den-server','uploads::')
        S.write_text(verify.BASE_SOURCE)
        result[name]['restored_boundaries']=run(name,'restored-boundaries','den-server','uploads::')
    original=(R/'worktrees/muse/crates/den-cli/src/main.rs').read_text()
    for label,before,after,filter in [
        ('unit_validation_mutant','if !(1..=200).contains(&limit) {','if false && !(1..=200).contains(&limit) {','read_args_reject_both_cursors_and_bad_limits'),
        ('unit_sort_mutant','out.sort_by(|a, b| a.id.cmp(&b.id));','// judge mutation: do not sort','sort_messages_orders_ascending_and_dedups')]:
        mutated=original.replace(before,after,1);assert mutated!=original
        cli.write_text(mutated)
        result['muse'][label]=run('muse',label,'den',filter)
    cli.write_text(original)
    result['muse']['restored_cli_units']=run('muse','restored-cli-units','den','')
finally:
    S.write_text(verify.BASE_SOURCE)
    cli.write_bytes(cli_original)
    (O/'extra-verification.json').write_text(json.dumps(result,indent=2))
