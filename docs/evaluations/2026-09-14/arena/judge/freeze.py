import sys,json,pathlib,subprocess,shutil,datetime
R=pathlib.Path('/tmp/den-arena-20260914')
round,who,label=sys.argv[1:]
cwd=R/f'{round}-{who}'; out=R/'judge/frozen'/round/label
out.mkdir(parents=True,exist_ok=False)
shutil.copytree(cwd,out/'source',ignore=shutil.ignore_patterns('.git','node_modules','target','__pycache__'),symlinks=True)
for cmd,name in [(['git','diff','--no-ext-diff'],'diff.patch'),(['git','status','--porcelain'],'status.txt')]:
 (out/name).write_text(subprocess.check_output(cmd,cwd=cwd,text=True))
if who=='muse':
 import sqlite3
 db=pathlib.Path.home()/'.local/share/opencode/opencode.db'; con=sqlite3.connect(f'file:{db}?mode=ro',uri=True)
 sid=con.execute('select id from session where directory=? order by time_created desc limit 1',(str(cwd.resolve()),)).fetchone()[0];con.close()
 with (out/'native.json').open('w') as f: subprocess.run(['/Users/nicholascaron/.opencode/bin/opencode','--pure','export',sid],stdout=f,check=True)
 data=json.loads((out/'native.json').read_text()); msgs=data['messages'];last=next((m for m in reversed(msgs) if m['info']['role']=='assistant' and m['info'].get('finish')=='stop'),msgs[-1]);response='\n\n'.join(p['text'] for p in last['parts'] if p['type']=='text')
else:
 sid=json.loads((R/f'configs/{round}-{who}-session.json').read_text())['session_uuid']
 original=next(pathlib.Path.home().glob(f'.claude/projects/*{round}-opus/{sid}.jsonl'))
 shutil.copyfile(original,out/'native.jsonl');msgs=[json.loads(l) for l in original.read_text().splitlines()];last=next(m for m in reversed(msgs) if m.get('type')=='assistant' and any(c.get('type')=='text' for c in m.get('message',{}).get('content',[])));response='\n\n'.join(c['text'] for c in last['message']['content'] if c.get('type')=='text')
(out/'response.md').write_text(response)
(out/'freeze.json').write_text(json.dumps({'time':datetime.datetime.now(datetime.timezone.utc).isoformat(),'who':who,'label':label,'round':round,'session':sid},indent=2))
print(out)
