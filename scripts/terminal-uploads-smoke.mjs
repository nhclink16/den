// Run on disposable data. BEFORE=1 captures the reported failures before the fix.
import { chromium } from 'playwright-core'
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises'
import { spawn, execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'
const base = process.env.DEN_SMOKE_URL || 'http://localhost:5173'
const before = process.env.BEFORE === '1'
const phase = before ? 'before' : 'after'
const shots = new URL('../docs/shots/pr/fix/terminal-uploads/', import.meta.url).pathname
await mkdir(shots, {recursive:true})
const scratch = await mkdtemp('/mnt/storage/terminal-smoke-')
execFileSync('ffmpeg',['-y','-f','lavfi','-i','testsrc2=size=640x360:rate=15','-t','2','-c:v','libx264','-preset','ultrafast',`${scratch}/clip.mp4`],{stdio:'ignore'})
// A playable short clip padded to three chunks makes the network checkpoints deterministic.
const clip = Buffer.alloc(24*1024*1024);(await readFile(`${scratch}/clip.mp4`)).copy(clip)
const password = process.env.DEN_SMOKE_PASSWORD
assert(password, 'Set DEN_SMOKE_PASSWORD')
let token, browser, hostProcess, host, page, videoStart, herdrStart, herdrEnd, uploadStart, uploadEnd
const rooms=[]
async function api(method,path,body) {
 const r=await fetch(base+path,{method,headers:{'content-type':'application/json',...(token?{authorization:`Bearer ${token}`}:{})},body:body===undefined?undefined:JSON.stringify(body)})
 assert(r.ok,`${method} ${path}: ${r.status} ${r.ok?'':await r.text()}`); return r.status===204?null:r.json()
}
const until = async (f,label) => {for(let i=0;i<200;i++){if(await f())return;await new Promise(r=>setTimeout(r,100))}throw Error(label)}
const shot = async name => {await page.waitForTimeout(350);await page.screenshot({path:`${shots}${name}-${phase}.png`})}
const editor = () => page.getByTestId('terminal-editor')
const screen = () => editor().evaluate(el=>{const t=el.denTerminal,b=t.buffer.active;return Array.from({length:b.length},(_,i)=>b.getLine(i)?.translateToString(true)||'').join('\n')})
const type = async text => {await editor().evaluate(el=>el.denTerminal.focus());await page.keyboard.type(text);await page.keyboard.press('Enter')}
const sessionName = `den-terminal-${Date.now()}`
const herdr = (...args) => {const out=execFileSync('herdr',['--session',sessionName,...args],{encoding:'utf8'});return out.trim()?JSON.parse(out).result:undefined}
let sessionStarted = false
async function extraChecks(channelId) {
 const o=await api('POST',`/hosts/${host.id}/sessions`,{channel_id:channelId})
 await page.locator(`[data-object-id="${o.id}"]`).click()
 await until(()=>editor().evaluate(el=>!!el.denTerminal).catch(()=>false),'second Ghostty mounted')
 const toggle=page.getByRole('checkbox',{name:'Record session',exact:true})
 await toggle.click()
 await until(async()=>(await api('GET',`/objects/${o.id}`)).state.terminal.recording_enabled,'recording toggle persisted')
 assert((await api('GET',`/users/me/hosts/${host.id}/recording`)).enabled)
 await until(async()=>await toggle.getAttribute('aria-disabled')!=='true','recording toggle ready')
 await toggle.focus();await page.keyboard.press('Space')
 await until(async()=>!(await api('GET',`/objects/${o.id}`)).state.terminal.recording_enabled,'keyboard disables recording')
 await until(async()=>await toggle.getAttribute('aria-disabled')!=='true','recording toggle ready again')
 await page.keyboard.press('Space')
 await until(async()=>(await api('GET',`/objects/${o.id}`)).state.terminal.recording_enabled,'keyboard enables recording')
 await type("printf 'RECORDED_OUTPUT\\n'")
 await shot('recording-on')
 const rawPath=`${scratch}/mouse.bin`
 const program=`${scratch}/mouse.py`
 await writeFile(program, `import os,sys,termios,tty\nold=termios.tcgetattr(0)\ntty.setraw(0)\ntry:\n os.write(1,b"\\x1b[?1002h\\x1b[?1006hMOUSE_READY\\r\\n")\n with open("${rawPath}","wb",buffering=0) as f:\n  while True:\n   b=os.read(0,1024)\n   if b==b"q": break\n   f.write(b)\nfinally:\n os.write(1,b"\\x1b[?1002l\\x1b[?1006l\\r\\nMOUSE_DONE\\r\\n")\n termios.tcsetattr(0,termios.TCSADRAIN,old)\n`)
 await type(`python3 ${program}`)
 await until(()=>editor().evaluate(el=>el.denTerminal.hasMouseTracking()),'program enabled mouse tracking')
 const b=await editor().locator('canvas').boundingBox()
 await page.mouse.move(b.x+80,b.y+60);await page.mouse.down();await page.mouse.move(b.x+140,b.y+80,{steps:4});await page.mouse.up();await page.mouse.wheel(0,99);await page.waitForTimeout(300)
 await page.keyboard.type('q');await until(async()=>(await screen()).includes('MOUSE_DONE'),'raw mouse program exited')
 const reports=await readFile(rawPath,'utf8')
 assert.match(reports,/\x1b\[<0;\d+;\d+M/,'PTY received press')
 assert.match(reports,/\x1b\[<32;\d+;\d+M/,'PTY received drag')
 assert.match(reports,/\x1b\[<0;\d+;\d+m/,'PTY received release')
 assert.match(reports,/\x1b\[<65;\d+;\d+M/,'PTY received wheel')
 console.log('PTY received SGR click, drag, release and wheel')
 await writeFile(program,(await readFile(program,'utf8')).replace('1006h','1006l'))
 await type(`python3 ${program}`)
 await until(()=>editor().evaluate(el=>el.denTerminal.hasMouseTracking()&&!el.denTerminal.getMode(1006)),'legacy mouse mode enabled')
 const grid=await editor().evaluate(el=>({cols:el.denTerminal.cols,rows:el.denTerminal.rows}))
 const column=Math.min(100,grid.cols);assert(column>95,'legacy probe needs a high-bit coordinate')
 await page.mouse.click(b.x+(column-.5)*b.width/grid.cols,b.y+2.5*b.height/grid.rows)
 await page.waitForTimeout(150);await page.keyboard.type('q')
 await until(()=>editor().evaluate(el=>!el.denTerminal.hasMouseTracking()),'legacy program exited')
 const legacy=await readFile(rawPath)
 assert(legacy.includes(Buffer.from([27,91,77,32,column+32,35])),'legacy coordinates reach PTY as bytes, not UTF-8')
 console.log('Legacy high-bit mouse coordinate passed')
 await type("seq 1 200")
 await page.waitForTimeout(300)
 const initial=await editor().evaluate(el=>el.denTerminal.getViewportY())
 await page.mouse.wheel(0,-330);await page.waitForTimeout(500)
 const scrolled=await editor().evaluate(el=>el.denTerminal.getViewportY())
 assert(scrolled>initial,'wheel with tracking off scrolls scrollback')
 await page.getByRole('button',{name:'End session',exact:true}).click()
 await page.getByRole('slider',{name:'Replay position'}).waitFor()
 const state=(await api('GET',`/objects/${o.id}`)).state.terminal
 assert(state.recording_upload_id,'opt-in stores recording')
 await until(async()=>+(await page.getByLabel('Replay position').getAttribute('max'))>1,'recording downloaded')
 await page.getByLabel('Replay position').fill('0')
 await shot('replay')
 await page.getByRole('button',{name:'Close terminal',exact:true}).click()
 const remembered=await api('POST',`/hosts/${host.id}/sessions`,{channel_id:channelId})
 assert(remembered.state.terminal.recording_enabled,'new session remembers account preference')
 await api('POST',`/sessions/${remembered.id}/recording`,{enabled:false})
 await api('DELETE',`/sessions/${remembered.id}`)
 console.log('Recording toggle, replay, remembered preference and scrollback passed')
 let release,started=false,cancelId,completed=false
 const hold=new Promise(r=>release=r)
 await page.route('**/uploads/**',async route=>{const req=route.request();if(req.method()==='PATCH'){cancelId=new URL(req.url()).pathname.split('/').at(-1);started=true;await hold}if(req.url().endsWith('/complete'))completed=true;await route.continue()})
 await page.locator('input[type=file]').setInputFiles({name:'cancel-me.bin',mimeType:'application/octet-stream',buffer:Buffer.alloc(1024)})
 await until(()=>started,'cancel upload began')
 await page.locator('.pending').getByRole('button',{name:'Remove',exact:true}).click()
 const response=page.waitForResponse(r=>r.request().method()==='PATCH'&&r.url().endsWith(cancelId))
 release();await response;await page.waitForTimeout(200)
 assert(!completed,'cancelled upload must not finalize')
 assert.equal(await page.locator('.pending').count(),0)
 assert.equal((await api('GET',`/uploads/${cancelId}`)).complete,false)
 await page.unrouteAll({behavior:'wait'})
 console.log('Removing an in-flight upload prevents completion')
}
try {
 const login=await api('POST','/auth/login',{username:'nicholas',password});token=login.token
 const enrollment=await api('POST','/hosts/enroll',{})
 const env={...process.env,DEN_HOST_CONFIG_DIR:scratch,DEN_HOST_NAME:'terminal-night'}
 for(const key of Object.keys(env))if(key.startsWith('HERDR_'))delete env[key]
 const hostBin=process.env.DEN_SMOKE_HOST_BIN||'/mnt/storage/den-terminal-target/debug/den-host'
 execFileSync(hostBin,['login',enrollment.code],{env,stdio:['ignore','ignore','pipe']})
 host=(await api('GET','/hosts')).find(h=>h.name==='terminal-night')
 hostProcess=spawn(hostBin,['run'],{env,stdio:'ignore'})
 await until(async()=>(await api('GET','/hosts')).some(h=>h.id===host.id&&h.online),'host connected')
 const room=await api('POST','/channels',{name:'terminal-check',kind:'text',position:0})
 const other=await api('POST','/channels',{name:'elsewhere-check',kind:'text',position:0})
 rooms.push(room.id,other.id)
 browser=await chromium.launch({executablePath:'/usr/bin/chromium',headless:true,args:['--no-sandbox']})
 const context=await browser.newContext({viewport:{width:1440,height:900},recordVideo:{dir:scratch,size:{width:1440,height:900}}})
 await context.addCookies([{name:'den_session',value:token,url:base,httpOnly:true,sameSite:'Lax'}])
 await context.addInitScript(csrf=>localStorage.setItem('den.csrf',csrf),login.csrf_token)
 page=await context.newPage();videoStart=Date.now()
 page.on('pageerror',e=>console.error('PAGE ERROR',e.message))
 await page.goto(`${base}/c/${room.id}`)
 const opened=await api('POST',`/hosts/${host.id}/sessions`,{channel_id:room.id})
 const id=opened.id
 await page.locator(`[data-object-id="${id}"]`).click()
 await until(()=>editor().evaluate(el=>!!el.denTerminal).catch(()=>false),'Ghostty mounted')
 await type("printf 'TERMINAL_READY\\n'")
 await until(async()=>(await screen()).includes('TERMINAL_READY'),'PTY ready')
 await shot('recording')
 console.log('Recording toggle count:',await page.getByRole('checkbox',{name:'Record session',exact:true}).count())
 if (!before) assert.equal(await page.getByRole('checkbox',{name:'Record session',exact:true}).isChecked(),false)
 herdrStart=Date.now()
 await page.getByRole('button',{name:'Expand terminal',exact:true}).click()
 await type(`herdr --session ${sessionName}`)
 await until(async()=>(await screen()).includes('machines'),'Herdr rendered');sessionStarted=true
 const w=herdr('workspace','list').workspaces[0]
 const panes=herdr('pane','list','--workspace',w.workspace_id).panes
 const first=panes[0]
 herdr('pane','run',first.pane_id,"printf '\\033[2J\\033[HLEFT PANE\\n'")
 const split=herdr('pane','split',first.pane_id,'--direction','right','--no-focus').pane
 herdr('pane','run',split.pane_id,"printf '\\033[2J\\033[HRIGHT PANE\\n'")
 const tab=herdr('tab','create','--workspace',w.workspace_id,'--label','Second tab','--no-focus')
 await page.waitForTimeout(700)
 const bounds=await editor().locator('canvas').boundingBox()
 await page.mouse.click(bounds.x+bounds.width*.8,bounds.y+bounds.height*.5)
 await page.waitForTimeout(500)
 const focused=herdr('pane','get',split.pane_id).pane.focused
 console.log('Herdr right pane focused after click:',focused)
 if(!before)assert(focused,'Click must focus right pane')
 await shot('mouse')
 await writeFile(`${scratch}/herdr-screen.txt`,await screen())
 // Tabs sit on the top row. Find the exact label in the rendered grid.
 const target=await editor().evaluate(el=>{const t=el.denTerminal,b=t.buffer.active;for(let y=0;y<t.rows;y++){const line=b.getLine(b.viewportY+y)?.translateToString(true)||'';const x=line.indexOf('Second tab');if(x>=0)return{x:x+3,y}}})
 if(target){await page.mouse.click(bounds.x+(target.x+.5)*bounds.width/(await editor().evaluate(el=>el.denTerminal.cols)),bounds.y+(target.y+.5)*bounds.height/(await editor().evaluate(el=>el.denTerminal.rows)));await page.waitForTimeout(500)}
 console.log('Herdr tab click target:',target)
 if(!before){const tabs=herdr('tab','list','--workspace',w.workspace_id).tabs;assert(tabs.some(t=>t.tab_id===tab.tab.tab_id&&t.focused),'Click must focus second tab')}
 herdrEnd=Date.now()
 await page.getByRole('button',{name:'End session',exact:true}).click()
 await until(async()=>(await api('GET',`/objects/${id}`)).state.terminal.ended_at,'session ended')
 const ended=(await api('GET',`/objects/${id}`)).state.terminal
 console.log('Default-off ended recording:',!!ended.recording_upload_id)
 if(!before){assert(!ended.recording_upload_id);assert.equal(await page.getByRole('button',{name:/Play replay|Pause replay/}).count(),0)}
 await shot('ended')
 await page.getByRole('button',{name:'Close terminal',exact:true}).click()
 await page.goto(`${base}/settings/machines`);await page.getByRole('heading',{name:'Machines',exact:true}).waitFor();await shot('machines')
 await page.goto(`${base}/c/${room.id}`)
 uploadStart=Date.now()
 let releaseFirst, release;const firstHold=new Promise(r=>releaseFirst=r),hold=new Promise(r=>release=r);let chunks=0,uploadId
 await page.route('**/uploads/*',async route=>{if(route.request().method()==='PATCH'){uploadId=new URL(route.request().url()).pathname.split('/').at(-1);chunks++;if(chunks===1)await firstHold;else if(chunks===2)await hold}await route.continue()})
 await page.locator('input[type=file]').setInputFiles({name:'overnight-clip.mp4',mimeType:'video/mp4',buffer:clip})
 await until(()=>chunks===1,'upload chunk started')
 await page.locator(`.side a[href="/c/${other.id}"]`).click();releaseFirst()
 await until(()=>chunks===2,'upload continues while away')
 assert.equal((await api('GET',`/uploads/${uploadId}`)).offset,8*1024*1024,'first chunk committed in the background')
 await page.waitForTimeout(700);await shot('upload-away')
 await page.locator(`.side a[href="/c/${room.id}"]`).click();await page.waitForTimeout(700);await shot('upload-return')
 const retained=await page.locator('.pending .fname').count();console.log('Pending upload after return:',retained)
 if(!before){assert.equal(retained,1);assert.equal(await page.locator(`.side a[href="/c/${room.id}"] [aria-label="Uploading files"]`).count(),1)}
 release();await page.unrouteAll({behavior:'wait'})
 if(!before){await page.locator('.chip.done').waitFor();await page.getByTitle('Send (Enter)',{exact:true}).click();await page.locator('.pending').waitFor({state:'hidden'});const messages=await api('GET',`/channels/${room.id}/messages`);assert(messages.some(m=>m.attachments?.some(u=>u.filename==='overnight-clip.mp4')),'retained upload sends to its original room')}
 uploadEnd=Date.now()
 if (!before) await extraChecks(room.id)
 const video=page.video();await context.close();await video.saveAs(`${scratch}/${phase}.webm`)
 for(const [name,start,end] of [['herdr',herdrStart,herdrEnd],['uploads',uploadStart,uploadEnd]]) execFileSync('ffmpeg',['-y','-ss',String(Math.max(0,(start-videoStart)/1000)), '-i',`${scratch}/${phase}.webm`,'-t',String((end-start)/1000),'-an','-vf','fps=12','-c:v','libx264','-crf','28','-pix_fmt','yuv420p','-movflags','+faststart',`${shots}${name}-${phase}.mp4`],{stdio:'ignore'})
 console.log(`PASS ${phase} capture; scratch ${scratch}`)
} catch(e){if(page){await shot('failure').catch(()=>{});console.error(await screen().catch(()=>''))}throw e}
finally {if(sessionStarted)try{herdr('server','stop')}catch{};await browser?.close();hostProcess?.kill();if(host)await api('DELETE',`/hosts/${host.id}`).catch(()=>{});for(const room of rooms)await api('DELETE',`/channels/${room}`).catch(()=>{})}
