import sys,json,subprocess,shutil,os,signal,time
from pathlib import Path
from reconnect_fixture import exercise
R=Path('/tmp/den-arena-20260914');rows={}
env=os.environ.copy();env.update(CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='3',SQLX_OFFLINE='true')
for label,who in [('A','muse'),('B','opus')]:
 cwd=R/f'judge/frozen/repair/{label}/source';target=R/f'target-repair-{who}';env['CARGO_TARGET_DIR']=str(target)
 for name,args in [('build',['cargo','build','-p','den']),('tests',['cargo','test','-p','den']),('fmt',['cargo','fmt','-p','den','--','--check'])]:
  with (R/f'judge/frozen/repair/{label}/{name}.log').open('w') as f:
   p=subprocess.run(args,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT,timeout=120)
   rows.setdefault(label,{})[name]=p.returncode
 binary=target/'debug/den';rows[label]['behavior']={}
 for mode in ['reconnect','401','403','transient','filter']:
  rows[label]['behavior'][mode]=exercise(binary,mode=mode,channel=mode=='filter')
  print(label,mode,rows[label]['behavior'][mode]['passed'],flush=True)
 (R/'judge/repair-verification.json').write_text(json.dumps(rows,indent=2))
print('Independent behavior verification complete',flush=True)
