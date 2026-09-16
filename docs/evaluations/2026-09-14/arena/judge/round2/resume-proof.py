import importlib.util,json,subprocess
from pathlib import Path
s=importlib.util.spec_from_file_location('proof',Path(__file__).with_name('prove.py'));m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
m.TESTS['read']='m2_live::websocket_targets_notifications_and_read_state_without_dm_leaks'
results=json.loads((m.OUT/'results.json').read_text())
try:
    for label,parts,expected in m.MATRIX:
        if label in results: continue
        m.write(parts)
        binary=m.build(label)
        results[label]={}
        for key,test in m.TESTS.items():
            with (m.OUT/f'{label}-{key}.log').open('w') as log:
                run=subprocess.run([binary,'--exact',test,'--nocapture'],cwd=m.ROOT,stdout=log,stderr=subprocess.STDOUT,timeout=90)
            results[label][key]=run.returncode
        (m.OUT/'results.json').write_text(json.dumps(results,indent=2)+'\n')
        print(label,results[label],flush=True)
        assert [k for k,v in results[label].items() if v!=0]==expected
finally:
    m.write([])
subprocess.run(['python3',str(m.OUT/'prove-direct-read.py')],check=True)
