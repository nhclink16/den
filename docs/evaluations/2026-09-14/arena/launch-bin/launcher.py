#!/usr/bin/env python3
import os,sys,json
from pathlib import Path
R=Path('/tmp/den-arena-20260914');cwd=Path.cwd();name=cwd.name
meta=json.loads((R/'configs'/f'{name}-session.json').read_text())
kind=Path(sys.argv[0]).name
keep={'HOME','USER','LOGNAME','PATH','SHELL','TMPDIR','TERM','LANG','LC_ALL','COLORTERM','TERM_PROGRAM','TERM_PROGRAM_VERSION','XDG_DATA_HOME'}
env={k:v for k,v in os.environ.items() if k in keep or k.startswith('HERDR_')}
env.update(CARGO_TARGET_DIR=meta['target'],CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='3',SQLX_OFFLINE='true')
py=str(R/'runtime/bin/python')
if kind=='opencode':
 env.update(XDG_CONFIG_HOME=str(R/f'{name}-config-home'),XDG_STATE_HOME=str(R/f'{name}-state'),OPENCODE_TEST_HOME=str(R/f'{name}-config-home'),OPENCODE_CONFIG=str(R/'configs'/f'{name}-opencode.json'),OPENCODE_DISABLE_PROJECT_CONFIG='1',OPENCODE_DISABLE_CLAUDE_CODE='1',OPENCODE_DISABLE_EXTERNAL_SKILLS='1',OPENCODE_DISABLE_CLAUDE_CODE_SKILLS='1',OPENCODE_DISABLE_DEFAULT_PLUGINS='1',OPENCODE_DISABLE_AUTOUPDATE='1',OPENCODE_DISABLE_MODELS_FETCH='1',OPENCODE_DISABLE_LSP_DOWNLOAD='1',OPENCODE_DISABLE_SHARE='1')
 binary='/Users/nicholascaron/.opencode/bin/opencode'
 args=[binary,str(cwd),'--pure','--model','opencode-go/muse-spark-1.3-contributor','--agent','build']
 if sys.argv[1:]==['--arena-debug-config']:args=[binary,'--pure','debug','config']
else:
 env['CLAUDE_CODE_DISABLE_AUTO_MEMORY']='1'
 binary='/Users/nicholascaron/.local/bin/claude'
 tools='Read,Glob,Grep,Bash' if name.startswith('review') else 'Read,Glob,Grep,Edit,Write,Bash'
 args=[binary,'--safe-mode','--restricted','--setting-sources','','--strict-mcp-config','--mcp-config','{"mcpServers":{}}','--no-chrome','--disable-slash-commands','--model','claude-opus-5','--effort','high','--session-id',meta['session_uuid'],'--name','Arena Opus '+name.split('-')[0],'--tools',tools,'--permission-mode','dontAsk','--settings',str(R/'configs'/f'{name}-claude.json'),'--allowedTools','Read','Glob','Grep','Edit','Write','Bash(cargo *)','Bash(rustfmt *)','Bash(git diff)','Bash(git diff *)','Bash(git status)','Bash(git status *)',f'Bash({py} tools/arena_repro.py *)']
if kind=='claude' and name=='repair-opus':
 args[args.index('--session-id')]='--resume'
os.execve(binary,args,env)
