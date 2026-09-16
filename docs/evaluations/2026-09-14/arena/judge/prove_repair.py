from pathlib import Path
import json,sys
from guarded import run
from reconnect_fixture import exercise
R=Path('/tmp/den-arena-20260914');w=R/'judge/repair-work';src=w/'crates/den-cli/src/stream.rs';original=(R/'baseline/crates/den-cli/src/stream.rs').read_text();seeded=(R/'judge/seeded-stream.rs').read_text();binary=R/'judge/repair-target/debug/den'
results={}
try:
 variants={'combined':seeded,'headers_only':seeded.replace('response.status().as_u16() == 403','matches!(response.status().as_u16(), 401 | 403)',1),'auth_only':original.replace('matches!(response.status().as_u16(), 401 | 403)','response.status().as_u16() == 403',1)}
 for name,source in variants.items():
  src.write_text(source);run(['cargo','build','-p','den'],w,R/'judge/repair-target',R/f'judge/repair-{name}-build.log')
  mode='401' if name=='auth_only' else 'reconnect';result=exercise(binary,mode=mode)
  results[name]=result;print(name,result['passed'],flush=True);assert not result['passed']
finally:
 src.write_text(original)
 run(['cargo','build','-p','den'],w,R/'judge/repair-target',R/'judge/repair-restored-build.log')
 results['restored']=exercise(binary);assert results['restored']['passed']
 (R/'judge/repair-mutation-proof.json').write_text(json.dumps(results,indent=2))
print('REPAIR FIXTURE PROVEN',flush=True)
