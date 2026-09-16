import sys,subprocess,json,hashlib
from pathlib import Path
sys.path.insert(0,str(Path(__file__).parent))
from guarded import run
R=Path('/tmp/den-arena-20260914')
for who in ['muse','opus']:
 target=R/f'target-repair-{who}'; cwd=R/f'repair-{who}'
 if not target.exists(): subprocess.run(['cp','-cR',str(R/'judge/repair-target'),str(target)],check=True)
 run(['cargo','clean','-p','den','-p','den-core'],cwd,target,R/f'judge/warm-{who}-clean.log')
 run(['cargo','build','-p','den'],cwd,target,R/f'judge/warm-{who}-build.log')
 print(who,'ready',flush=True)
