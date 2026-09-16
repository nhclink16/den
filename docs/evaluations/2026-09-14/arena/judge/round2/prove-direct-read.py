import importlib.util, json, subprocess
from pathlib import Path
s=importlib.util.spec_from_file_location('proof',Path(__file__).with_name('prove.py'));m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
harness=m.ROOT/'crates/den-server/tests/api.rs'
hidden=m.ROOT/'crates/den-server/tests/api/arena_review.rs'
original=(m.BASE/'crates/den-server/tests/api.rs').read_text()
harness.write_text(original+'\n#[path = "api/arena_review.rs"]\nmod arena_review;\n')
hidden.write_text((m.OUT/'arena_review.rs').read_text())
results=json.loads((m.OUT/'direct-read-results.json').read_text()) if (m.OUT/'direct-read-results.json').exists() else {}
try:
    for label,parts,expected in [('direct-read-baseline',[],0),('direct-read-mutant',['read'],101),('direct-read-restored',[],0)]:
        if results.get(label)==expected: continue
        m.write(parts)
        binary=m.build(label)
        with (m.OUT/(label+'.log')).open('w') as log:
            run=subprocess.run([binary,'--exact',m.TESTS['read'],'--nocapture'],cwd=m.ROOT,stdout=log,stderr=subprocess.STDOUT,timeout=30)
        results[label]=run.returncode
        (m.OUT/'direct-read-results.json').write_text(json.dumps(results,indent=2)+'\n')
        print(label,run.returncode,flush=True)
        assert run.returncode==expected
finally:
    m.write([])
    harness.write_text(original)
    hidden.unlink(missing_ok=True)
binary=m.build('warm-clean-baseline')
listed=subprocess.check_output([binary,'--list'],text=True)
assert 'arena_review::' not in listed
(m.OUT/'clean-target-tests.txt').write_text(listed)
print('Clean baseline test target ready',flush=True)
