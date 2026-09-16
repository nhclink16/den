import os,signal,subprocess,time,shutil
from pathlib import Path

def run(cmd,cwd,target,log):
 env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(target),CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='3',SQLX_OFFLINE='true')
 with Path(log).open('w') as f:
  p=subprocess.Popen(cmd,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT,start_new_session=True)
  start=time.monotonic()
  while p.poll() is None:
   if shutil.disk_usage('/System/Volumes/Data').free<8*1024**3 or time.monotonic()-start>600:
    os.killpg(p.pid,signal.SIGTERM);p.wait();raise RuntimeError('Stopped for disk/time limit; '+str(log))
   time.sleep(1)
 if p.returncode:raise RuntimeError('Command failed; '+str(log))
 return p.returncode
if __name__=='__main__':
 R=Path('/tmp/den-arena-20260914');run(['cargo','build','-p','den'],R/'judge/repair-work',R/'judge/repair-target',R/'judge/repair-build.log')
