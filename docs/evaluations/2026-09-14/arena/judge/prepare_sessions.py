import os,json,shutil,subprocess,uuid
from pathlib import Path
R=Path('/tmp/den-arena-20260914');PYTHON=str(R/'runtime/bin/python')
configs=R/'configs';configs.mkdir(exist_ok=True)
for round in ['repair','review']:
 for model in ['muse','opus']:
  name=f'{round}-{model}';p=R/name
  if p.exists():raise RuntimeError('Refusing to overwrite '+str(p))
  shutil.copytree(R/'baseline',p)
  if round=='repair':
   shutil.copyfile(R/'judge/seeded-stream.rs',p/'crates/den-cli/src/stream.rs')
   (p/'tools').mkdir(exist_ok=True);shutil.copyfile(R/'judge/reconnect_fixture.py',p/'tools/arena_repro.py')
  subprocess.run(['git','init','-q',str(p)],check=True)
  subprocess.run(['git','-C',str(p),'config','user.name','Arena Fixture'],check=True)
  subprocess.run(['git','-C',str(p),'config','user.email','arena@localhost'],check=True)
  # New disposable repository owned entirely by this preparer, explicit tracked source paths.
  paths=[x.name for x in p.iterdir() if x.name!='.git']
  subprocess.run(['git','-C',str(p),'add','--',*paths],check=True)
  subprocess.run(['git','-C',str(p),'commit','-qm','Initialize arena snapshot'],check=True)
  empty=R/f'{name}-config-home';empty.mkdir(exist_ok=True)
  (R/f'{name}-state').mkdir(exist_ok=True)
  bash={'*':'deny','cargo *':'allow','rustfmt *':'allow','git diff':'allow','git diff *':'allow','git status':'allow','git status *':'allow',f'{PYTHON} tools/arena_repro.py *':'allow'}
  config={'$schema':'https://opencode.ai/config.json','model':'opencode-go/muse-spark-1.3-contributor','enabled_providers':['opencode-go'],'autoupdate':False,'share':'disabled','plugin':[],'mcp':{},'lsp':False,'formatter':False,'instructions':[],
  'provider':{'opencode-go':{'models':{'muse-spark-1.3-contributor':{'options':{'reasoningEffort':'xhigh'}}}}},
  'permission':{'*':'deny','read':'allow','glob':'allow','grep':'allow','list':'allow','edit':'allow','external_directory':'deny','bash':bash}}
  (configs/f'{name}-opencode.json').write_text(json.dumps(config,indent=2))
  (configs/f'{name}-claude.json').write_text(json.dumps({'availableModels':['claude-opus-5'],'autoMemoryEnabled':False},indent=2))
  (configs/f'{name}-session.json').write_text(json.dumps({'session_uuid':str(uuid.uuid4()),'cwd':str(p),'target':str(R/f'target-{name}')},indent=2))
print('Four isolated snapshots initialized; seed exists in initial history only.')
