const password = process.env.DEN_PASSWORD
if (!password) throw Error('Set DEN_PASSWORD to the isolated dev account password')
import { chromium } from 'playwright-core'
import { mkdir, writeFile, readFile, unlink } from 'node:fs/promises'
import { execFileSync, spawn } from 'node:child_process'
import assert from 'node:assert/strict'
const root = new URL('../', import.meta.url).pathname
const out = root+'docs/shots/pr/feat-sounds/'
await mkdir(out,{recursive:true})
const browser = await chromium.launch({executablePath:'/usr/bin/chromium',headless:true,args:['--no-sandbox','--autoplay-policy=no-user-gesture-required']})
const context = await browser.newContext({viewport:{width:1440,height:1100}})
await context.addInitScript(() => {
  const Native = window.AudioContext
  window.AudioContext = class extends Native { constructor(...args) { super(...args); window.__audio = this; this.capture = this.createMediaStreamDestination() } }
  const connect = AudioNode.prototype.connect
  AudioNode.prototype.connect = function (destination,...args) { if(destination instanceof AudioDestinationNode) connect.call(this,this.context.capture); return connect.call(this,destination,...args) }
  window.__starts=[]
  const start=AudioBufferSourceNode.prototype.start
  AudioBufferSourceNode.prototype.start=function(...args){window.__starts.push({time:performance.now(),duration:this.buffer.duration}); return start.apply(this,args)}
})
const page=await context.newPage(),errors=[]
page.on('response', async r => { if(r.status() >= 400) console.log('HTTP',r.status(),r.url(),(await r.text()).slice(0,200)) })
page.on('pageerror',e=>errors.push(e.message))
await page.goto('http://localhost:5182')
await page.getByLabel('Username',{exact:true}).fill('nicholas');await page.getByLabel('Password',{exact:true}).fill(password);await page.getByRole('button',{name:'Come in'}).click()
await page.locator('a[title="Settings"]').click()
const api = async (path, method='GET', body) => page.evaluate(async ({path,method,body})=>{
  const res=await fetch(path,{method,headers:{'content-type':'application/json','x-csrf-token':localStorage.getItem('den.csrf')},body:body===undefined?undefined:JSON.stringify(body)}); if(!res.ok)throw Error(`${res.status} ${await res.text()}`);return res.json()
},{path,method,body})
await api('/users/me/sounds','PUT',{})
// Same account, viewport and theme; capture the previous settings controls from the base commit.
const files=['apps/web/src/ui/Settings.svelte','apps/web/src/ui/VoiceSettings.svelte']
const originals=await Promise.all(files.map(f=>readFile(root+f)))
try{
 for(const f of files)await writeFile(root+f,execFileSync('git',['show',`${process.env.SOUNDS_BASE || "d05e41a"}:${f}`],{cwd:root}))
 await page.waitForTimeout(1200);await page.goto('http://localhost:5182/settings/voice');await page.getByText('Play join and leave sounds',{exact:true}).waitFor();await page.screenshot({path:out+'before.png'})
}finally{for(let i=0;i<files.length;i++)await writeFile(root+files[i],originals[i])}
await page.waitForTimeout(1000);await page.goto('http://localhost:5182/settings/sounds');await page.getByRole('button',{name:'Test all',exact:true}).waitFor()
const requests=[];page.on('request',r=>{if(/\/sounds\/.*\.wav|\/sounds\/files\//.test(r.url()))requests.push(r.url())})
assert.equal(requests.length,0)
await page.screenshot({path:out+'after.png'})
// Settings writes survive reload, then restore the built-in set for the recording.
await page.getByRole('region',{name:'Direct message',exact:true}).getByLabel('Silent',{exact:true}).check()
await page.waitForTimeout(400);assert.equal((await api('/users/me/sounds')).resolved.dm.sound.type,'silent')
await page.reload();await page.getByRole('region',{name:'Direct message',exact:true}).getByLabel('Silent',{exact:true}).waitFor();assert(await page.getByRole('region',{name:'Direct message',exact:true}).getByLabel('Silent',{exact:true}).isChecked())
await api('/users/me/sounds','PUT',{});await page.reload();await page.getByRole('button',{name:'Test all',exact:true}).waitFor()
await page.getByLabel('Replace Message',{exact:true}).setInputFiles(root+'apps/web/public/sounds/message.wav');await page.getByText('Message replaced.',{exact:true}).waitFor()
assert.equal((await api('/users/me/sounds')).resolved.message.sound.type,'upload')
await page.getByLabel('Pack name',{exact:true}).fill('Night chimes');await page.getByRole('button',{name:'Save as pack',exact:true}).click();await page.getByText('Saved to your account.',{exact:true}).waitFor()
await page.getByRole('button',{name:'Use selected pack for this server',exact:true}).click();await page.getByText('Server pack saved. Each member keeps their own overrides.',{exact:true}).waitFor()
assert.equal((await api('/users/me/sounds')).server_pack.name,'Night chimes')
await page.getByRole('button',{name:'Reset server to Den',exact:true}).click();await page.getByText('Server uses Den sounds.',{exact:true}).waitFor()
await api('/users/me/sounds','PUT',{});await page.reload();await page.getByRole('button',{name:'Test all',exact:true}).waitFor()
await page.getByRole('heading',{name:'Sounds',exact:true}).click()
await page.evaluate(()=>{window.__starts=[];window.__chunks=[];window.__recorder=new MediaRecorder(window.__audio.capture.stream);window.__recorder.ondataavailable=e=>window.__chunks.push(e.data);window.__recorder.start()})
const ffmpeg=spawn('ffmpeg',['-y','-loglevel','error','-f','image2pipe','-framerate','8','-i','pipe:0','-c:v','libx264','-preset','fast','-pix_fmt','yuv420p',out+'test-screen.mp4'])
const began=Date.now();let frames=0,finished=false
await page.getByRole('button',{name:'Test all',exact:true}).click()
const completion=page.getByRole('button',{name:'Test all',exact:true}).waitFor({timeout:25000}).then(()=>finished=true)
while(!finished || Date.now()-began<13000){
 const shot=await page.screenshot({type:'jpeg',quality:80})
 const needed=Math.floor((Date.now()-began)/125)+1
 while(frames<needed){ffmpeg.stdin.write(shot);frames++}
 const playing=page.locator('.event.playing');if(await playing.count())await playing.scrollIntoViewIfNeeded()
 await page.waitForTimeout(70)
}
await completion
ffmpeg.stdin.end();await new Promise((r,j)=>{ffmpeg.on('close',c=>c?j(Error('ffmpeg failed')):r())})
const audio=await page.evaluate(async()=>{await new Promise(r=>{window.__recorder.onstop=r;window.__recorder.stop()});return Array.from(new Uint8Array(await new Blob(window.__chunks).arrayBuffer()))})
await writeFile(out+'test-audio.webm',Buffer.from(audio))
execFileSync('ffmpeg',['-y','-loglevel','error','-i',out+'test-screen.mp4','-i',out+'test-audio.webm','-c:v','copy','-af','aresample=async=1:first_pts=0','-c:a','aac','-shortest','-movflags','+faststart',out+'test-all.mp4'])
const starts=await page.evaluate(()=>window.__starts)
assert.equal(starts.length,11)
for(let i=1;i<starts.length;i++)assert(starts[i].time-starts[i-1].time>=starts[i-1].duration*1000+950)
assert.equal(errors.length,0,errors.join('\n'))
await writeFile(out+'browser-proof.json',JSON.stringify({checks:['no initial audio preload','silence survives reload','replace WAV','save named pack','admin sets server pack','eleven Test all sounds, one-second gaps','screen recording includes actual WebAudio output'],starts,errors},null,2))
await browser.close()
await unlink(out+'test-screen.mp4'); await unlink(out+'test-audio.webm')
console.log('Sound settings and recorded Test all passed')
