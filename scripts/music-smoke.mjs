// Run only against an isolated dev server: this smoke edits and consumes its queue.
// Two accounts (nicholas/bob), a credentials JSON containing password and room,
// and a paused queue seeded with the three URLs documented in 06-music-notes.md.
import { chromium } from 'playwright-core'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'
const base=process.env.DEN_SMOKE_URL || 'http://localhost:5184'
const credentials=JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || '/mnt/storage/den-music-dev/credentials.json','utf8'))
const shots=new URL('../docs/shots/pr/feat/music-queue/',import.meta.url).pathname
await mkdir(shots,{recursive:true})
const browser=await chromium.launch({executablePath:'/usr/bin/chromium',headless:!process.env.DISPLAY,args:['--no-sandbox','--use-fake-ui-for-media-stream','--use-fake-device-for-media-stream','--autoplay-policy=no-user-gesture-required','--window-position=0,0','--window-size=1440,1000',...(process.env.DEN_SMOKE_AUDIO ? [`--use-file-for-fake-audio-capture=${process.env.DEN_SMOKE_AUDIO}`]:[])]})
const errors=[],results={}
const until=async(fn,label,ms=90000)=>{const end=Date.now()+ms;while(Date.now()<end){if(await fn())return;await new Promise(r=>setTimeout(r,200))}throw Error(`Timed out: ${label}`)}
async function login(user){
 const c=await browser.newContext({viewport:{width:1440,height:900},permissions:['microphone','camera']})
 await c.addInitScript(()=>{localStorage.setItem('den.voice',JSON.stringify({sounds:false,micOn:false,musicDucking:true}));window.__pcs=[];const PC=window.RTCPeerConnection;window.RTCPeerConnection=class extends PC{constructor(...args){super(...args);window.__pcs.push(this)}}})
 const p=await c.newPage();p.on('pageerror',e=>errors.push(e.message));p.setDefaultTimeout(20000)
 await p.goto(base);await p.getByLabel('Username',{exact:true}).fill(user);await p.getByLabel('Password',{exact:true}).fill(credentials.password);await p.getByRole('button',{name:'Come in',exact:true}).click();await p.getByLabel('Join hangout',{exact:true}).click();await p.getByTestId('call-dock').waitFor();await until(()=>p.locator('summary[aria-label="Music queue"]').count().then(n=>n>0),'music button')
 if(await p.getByRole('button',{name:'Play sound on this device',exact:true}).count()) await p.getByRole('button',{name:'Play sound on this device',exact:true}).click()
 return p
}
const moduleCall=p=>p.evaluate(async()=>{const url=performance.getEntriesByType('resource').find(e=>new URL(e.name).pathname==='/src/lib/call.svelte.ts').name;window.__call=(await import(url)).call})
const inspect=p=>p.evaluate(()=>({participants:window.__call.participants.map(p=>({id:p.id,name:p.name,music:p.music})),ducked:window.__call.ducked,outputMuted:window.__call.outputMuted,error:window.__call.error,audio:[...document.querySelectorAll('audio')].map(e=>({id:e.dataset.userId,volume:e.volume,paused:e.paused,currentTime:e.currentTime}))}))
let a,b,recording
try{
 a=await login('nicholas');b=await login('bob');await moduleCall(a);await moduleCall(b);console.log('Joined both listeners')
 await a.locator('summary[aria-label="Music queue"]').click();await b.locator('summary[aria-label="Music queue"]').click()
 const panel=a.getByTestId('music-panel'),other=b.getByTestId('music-panel')
 await until(()=>panel.locator('ol li').count().then(n=>n>=3),'prepared queue')
 await until(()=>panel.locator('ol').innerText().then(t=>!t.includes('Loading track')),'resolved metadata')
 await panel.getByRole('button',{name:'Play',exact:true}).click()
 await until(async()=>(await inspect(a)).participants.some(p=>p.music),'DJ joins')
 await until(async()=>(await inspect(a)).audio.some(e=>e.id.startsWith('den-dj-')&&!e.paused&&e.currentTime>0),'DJ audio plays')
 await until(()=>other.locator('.now h3').innerText().then(t=>t.includes('Never Gonna')),'second listener queue sync')
 const before=await inspect(a);await new Promise(r=>setTimeout(r,1500));const after=await inspect(a)
 assert(after.audio.find(e=>e.id.startsWith('den-dj-')).currentTime>before.audio.find(e=>e.id.startsWith('den-dj-')).currentTime)
 results.dj=after.participants.find(p=>p.music);console.log('Receiving live DJ audio')
 const progress=await a.evaluate(async room=>await (await fetch(`/rooms/${room}/music`)).json(),credentials.room)
 assert(progress.position_seconds>=1 && progress.updated_at>1e12, 'server playback progress uses seconds with a millisecond sample timestamp')
 await a.bringToFront();await a.screenshot({path:`${shots}/after.png`})
 // Existing participant volume component, including local mute and restoration.
 await panel.getByText('Your listening volume',{exact:true}).click()
 const volume=panel.getByRole('slider',{name:/volume/})
 await volume.focus();await volume.press('Home');for(let i=0;i<35;i++)await volume.press('ArrowRight')
 await until(async()=>Math.abs((await inspect(a)).audio.find(e=>e.id.startsWith('den-dj-')).volume-.35)<.001,'saved DJ volume')
 await panel.getByRole('button',{name:'Mute for me',exact:true}).click();assert.equal((await inspect(a)).audio.find(e=>e.id.startsWith('den-dj-')).volume,0)
 await panel.getByRole('button',{name:'Mute for me',exact:true}).click()
 if(process.env.DEN_SMOKE_AUDIO){
  await b.locator('summary[aria-label="Music queue"]').press('Escape');await b.getByRole('button',{name:'Unmute microphone',exact:true}).click()
  await until(async()=>(await inspect(a)).ducked,'real remote voice ducks music')
  const ducked=(await inspect(a)).audio.find(e=>e.id.startsWith('den-dj-')).volume
  assert(Math.abs(ducked-.35*Math.pow(10,-12/20))<.001);results.duckedVolume=ducked
  await panel.getByRole('checkbox',{name:'Lower music while people talk'}).uncheck();assert(Math.abs((await inspect(a)).audio.find(e=>e.id.startsWith('den-dj-')).volume-.35)<.001)
  await panel.getByRole('checkbox',{name:'Lower music while people talk'}).check();await b.getByRole('button',{name:'Mute microphone',exact:true}).click();await until(async()=>!(await inspect(a)).ducked,'duck releases after speech')
 }
 console.log('Volume and ducking passed')
 await panel.getByText('Your listening volume',{exact:true}).click()
 // Keyboard reorder and then the native drag path; both mutate the same queue.
 const ids=()=>panel.locator('ol li').evaluateAll(es=>es.map(e=>e.dataset.trackId))
 const order=await ids();await panel.locator('ol li').first().getByRole('button',{name:/Move .* down/}).click();await until(async()=>(await ids())[1]===order[0],'keyboard reorder')
 await panel.locator('ol li').nth(1).dragTo(panel.locator('ol li').first());await until(async()=>(await ids())[0]===order[0],'drag reorder')
 if(process.env.DISPLAY){
  await a.bringToFront()
  recording=spawn('ffmpeg',['-y','-loglevel','error','-f','x11grab','-video_size','1440x1000','-framerate','15','-i',process.env.DISPLAY,'-an','-c:v','libx264','-threads','2','-preset','ultrafast','-pix_fmt','yuv420p',`${shots}/queue-panel.mp4`],{stdio:['pipe','ignore','inherit']});await new Promise(r=>setTimeout(r,1500))
 }
 await panel.getByRole('button',{name:'Pause',exact:true}).click();await until(()=>panel.getByRole('button',{name:'Play',exact:true}).isVisible(),'pause state')
 const seek=panel.getByRole('slider',{name:'Playback position',exact:true});await seek.evaluate(e => { e.value = '45'; e.dispatchEvent(new Event('change', { bubbles: true })) });await until(()=>panel.locator('.times').innerText().then(t=>t.includes('0:45')),'seek while paused')
 await panel.getByRole('button',{name:'Play',exact:true}).click();await until(async()=>{const q=await a.evaluate(async room=>await (await fetch(`/rooms/${room}/music`)).json(),credentials.room);return q.queue.some(t=>t.state==='playing')&&(await inspect(a)).audio.some(e=>e.id.startsWith('den-dj-')&&!e.paused)},'resume publication')
 await panel.locator('ol li').first().getByRole('button',{name:/Move .* down/}).click()
 await new Promise(r=>setTimeout(r,2000))
 if(recording){recording.stdin.write('q');await new Promise(r=>recording.on('exit',r));recording=null}
 console.log('Pause, seek, reorder passed')
 results.received=await b.evaluate(async()=>{const reports=await Promise.all(window.__pcs.map(pc=>pc.getStats()));return reports.flatMap(r=>[...r.values()].filter(s=>s.type==='inbound-rtp'&&s.kind==='audio').map(s=>({bytesReceived:s.bytesReceived,totalAudioEnergy:s.totalAudioEnergy,codec:[...r.values()].find(c=>c.id===s.codecId)?.mimeType})))})
 assert(results.received.some(r=>r.bytesReceived>0&&r.totalAudioEnergy>0&&r.codec==='audio/opus'),'real received Opus audio has energy')
 await a.locator('summary[aria-label="Music queue"]').press('Escape')
 await a.evaluate(room=>{history.pushState({},'',`/c/${room}`);dispatchEvent(new PopStateEvent('popstate'))},credentials.room)
 await a.getByLabel('YouTube URL',{exact:true}).fill('https://youtu.be/jNQXAC9IVRw');await a.getByText('Queue this video for everyone in the call?',{exact:true}).waitFor();await a.screenshot({path:`${shots}/voice-after.png`})
 await a.getByRole('button',{name:'Queue track',exact:true}).click();await a.getByText('Added to the queue.',{exact:true}).waitFor()
 await a.setViewportSize({width:390,height:844});await a.locator('summary[aria-label="Music queue"]').click();await until(()=>panel.evaluate(e=>e.parentElement.matches(':popover-open')),'mobile queue opens');await a.screenshot({path:`${shots}/mobile-after.png`})
 const box=await panel.boundingBox();assert(box.x>=0&&box.x+box.width<=390,'mobile panel fits')
 await a.locator('summary[aria-label="Music queue"]').press('Escape');await a.setViewportSize({width:1440,height:900});await a.reload();await a.getByLabel('Join hangout',{exact:true}).click();await moduleCall(a)
 await until(async()=>(await inspect(a)).audio.some(e=>e.id.startsWith('den-dj-')&&Math.abs(e.volume-.35)<.001),'DJ listener volume survives rejoin')
 console.log('Mobile and rejoin passed')
 // A skip cancels the old publisher; finishing the next track advances again.
 await a.locator('summary[aria-label="Music queue"]').click()
 const oldTitle=await panel.locator('.now h3').innerText()
 await panel.getByRole('button',{name:'Skip track',exact:true}).click()
 await until(async()=>await panel.locator('.now h3').count() && (await panel.locator('.now h3').innerText())!==oldTitle,'skip changes track')
 await until(async()=>{const q=await a.evaluate(async room=>await (await fetch(`/rooms/${room}/music`)).json(),credentials.room);return q.queue.some(t=>t.state==='playing')&&(await inspect(a)).audio.some(e=>e.id.startsWith('den-dj-')&&!e.paused&&e.currentTime>0)},'next publisher plays')
 const q=await a.evaluate(async room=>await (await fetch(`/rooms/${room}/music`)).json(),credentials.room)
 const current=q.queue.find(t=>t.state==='playing')
 assert(current)
 await panel.getByRole('slider',{name:'Playback position',exact:true}).evaluate((e,seconds)=>{e.value=String(seconds);e.dispatchEvent(new Event('change',{bubbles:true}))},Math.floor(current.duration)-2)
 await until(async()=>{const q=await a.evaluate(async room=>await (await fetch(`/rooms/${room}/music`)).json(),credentials.room);return !q.queue.some(t=>t.id===current.id)&&q.queue.some(t=>t.state==='playing')},'track EOF advances to next publisher')
 assert.equal((await inspect(a)).audio.filter(e=>e.id.startsWith('den-dj-')).length,1,'one DJ audio element remains after transport changes')
 results.skipAndNaturalAdvance=true
 results.errors=errors;assert.deepEqual(errors,[])
 await writeFile(`${shots}/verification.json`,JSON.stringify(results,null,2)+'\n')
 console.log('Music smoke passed',JSON.stringify(results))
}catch(e){if(a){console.log('State',await inspect(a).catch(()=>({})));await a.screenshot({path:`${shots}/failure.png`})}throw e}
finally{if(recording)recording.kill();await browser.close()}
