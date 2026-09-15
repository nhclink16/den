import { chromium } from 'playwright-core'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'
import { mkdir } from 'node:fs/promises'
const base = process.env.DEN_SMOKE_URL || 'http://localhost:5182'
const phase = process.env.DEN_STREAMS_PHASE || 'after'
const shots = new URL('../docs/shots/pr/fix-call-streams/', import.meta.url).pathname
await mkdir(shots, { recursive: true })
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: !process.env.DISPLAY, args: ['--no-sandbox','--use-fake-ui-for-media-stream','--use-fake-device-for-media-stream','--autoplay-policy=no-user-gesture-required','--enable-usermedia-screen-capturing','--auto-select-desktop-capture-source=Entire screen','--window-size=1440,1000','--window-position=0,0'] })
const errors = []
const wait = async (fn, label) => { for (let i=0;i<200;i++) { if(await fn())return;await new Promise(r=>setTimeout(r,100)) } throw Error(label) }
async function login(user) {
 const context=await browser.newContext({viewport:{width:1440,height:900},permissions:['microphone','camera']})
 context.setDefaultTimeout(15000)
 await context.addInitScript(() => {
  if(!localStorage.getItem('den.voice'))localStorage.setItem('den.voice',JSON.stringify({sounds:false,cameraOn:true,micOn:true}))
  window.__streamsCaptures=[]
  const capture=navigator.mediaDevices.getDisplayMedia.bind(navigator.mediaDevices);let n=0
  navigator.mediaDevices.getDisplayMedia=async(...args)=>{
   const s=await capture(...args), t=s.getVideoTracks()[0], settings=t.getSettings.bind(t)
   Object.defineProperty(t,'label',{value:++n%2 ? 'window:37438947' : 'Helium — docs'})
   t.getSettings=()=>({...settings(),displaySurface:n%2?'window':'browser'})
   // Chromium fake display capture has no audio on this host. Publish an
   // oscillator through a real WebRTC audio track to verify paired cleanup.
   if(!s.getAudioTracks().length){const ctx=new AudioContext(),osc=ctx.createOscillator(),dest=ctx.createMediaStreamDestination();osc.connect(dest);osc.start();s.addTrack(dest.stream.getAudioTracks()[0]);t.addEventListener('ended',()=>ctx.close())}
   window.__streamsCaptures.push(s)
   return s
  }
 })
 const page=await context.newPage();page.on('pageerror',e=>errors.push(e.message))
 await page.goto(base);await page.getByLabel('Username',{exact:true}).fill(user);await page.getByLabel('Password',{exact:true}).fill(process.env.DEN_SMOKE_PASSWORD)
 await page.getByRole('button',{name:'Come in',exact:true}).click();await page.getByLabel('Join hangout',{exact:true}).waitFor()
 await page.getByRole('link',{name:'general',exact:true}).click()
 return page
}
const screens=p=>p.getByTestId('screen-tile')
const shot=async(p,name)=>{await p.screenshot({path:`${shots}${name}-${phase}.png`,animations:'disabled'})}
async function record(name, action) {
 if(!process.env.DISPLAY){await action();return}
 const child=spawn('ffmpeg',['-y','-loglevel','error','-probesize','32','-analyzeduration','0','-f','x11grab','-video_size','1440x1000','-framerate','15','-i',process.env.DISPLAY,'-an','-c:v','libx264','-preset','ultrafast','-threads','2','-progress','pipe:1','-stats_period','0.1','-pix_fmt','yuv420p',`${shots}${name}.mp4`],{stdio:['pipe','pipe','inherit']})
 await new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error('Recorder produced no frames')),15000);child.stdout.on('data',data=>{if(/frame=[1-9]/.test(data.toString())){clearTimeout(timer);resolve()}});child.on('error',reject)})
 try{await new Promise(r=>setTimeout(r,1200));await action();await new Promise(r=>setTimeout(r,1800))}finally{child.stdin.write('q');await new Promise(r=>child.on('exit',r))}
}
async function join(p){await p.getByLabel('Join hangout',{exact:true}).click();await p.getByTestId('call-dock').waitFor();await wait(async()=>!await p.getByTitle('Camera (V)',{exact:true}).first().isDisabled(),'joined')}
try {
 const a=await login('nicholas'),b=await login('bob');await join(a);await join(b)
 await wait(async()=>await a.getByTestId('call-tile').count()===2,'two people')
 await b.getByLabel('Share screen',{exact:true}).first().click()
 await wait(async()=>await screens(a).count()===1,'remote share')
 await wait(async()=>await screens(a).locator('video').count()===1 && await a.locator('video').evaluateAll(es=>es.every(e=>e.videoWidth>0)), 'remote frames before screenshots')
 await a.bringToFront()
 const member=a.locator('.member-audio'), remote=screens(a).filter({has:a.locator('[aria-label="Bob audio options"]')})
 const audio=()=>a.locator('audio').evaluateAll(es=>es.map(e=>({volume:e.volume,paused:e.paused,id:e.dataset.userId})))
 const level=async(slider,n)=>{await slider.focus();await slider.press('Home');for(let i=0;i<n;i++)await slider.press('ArrowRight')}
 if(phase==='after'){
  await remote.getByLabel('Bob audio options').click()
  const panel=remote.locator('.volume-panel')
  await level(panel.getByRole('slider'),35)
  await wait(async()=>{const es=await audio();return es.length>=2&&es.every(e=>e.volume===.35&&!e.paused)},'mic and share audio at 35%')
  await shot(a,'01-volume-tile')
  await panel.getByRole('button',{name:'Mute for me'}).click()
  assert((await audio()).every(e=>e.volume===0),'local mute silences every received audio track')
  assert.equal(await b.getByLabel('Mute microphone',{exact:true}).count(),1,'remote microphone is still enabled')
  await panel.getByRole('button',{name:'Mute for me'}).click()
  assert((await audio()).every(e=>e.volume===.35),'unmute restores volume')
  await panel.getByRole('slider').press('Escape')
  await member.getByLabel('Bob audio options').click()
  assert.equal(await member.getByRole('slider').inputValue(),'35','member row shares tile state')
  await level(member.getByRole('slider'),0)
  await shot(a,'01-volume-member')
  assert((await audio()).every(e=>e.volume===0),'zero slider silences audio')
  await level(member.getByRole('slider'),35)
  await member.getByRole('slider').press('Escape')
  const voice=a.getByLabel('Join hangout',{exact:true}),general=a.getByRole('link',{name:'general',exact:true})
  assert((await voice.boundingBox()).y<(await general.boundingBox()).y,'voice above text')
 }else{await shot(a,'01-volume-tile');await shot(a,'01-volume-member')}
 await shot(a,'05-sidebar')
 if(phase==='after')await a.evaluate(async()=>{
  const url=performance.getEntriesByType('resource').find(e=>new URL(e.name).pathname==='/src/lib/call.svelte.ts').name
  const {call}=await import(url),p=call.room.localParticipant,publish=p.publishTrack.bind(p)
  // Hold the paired audio publication after video appears, reproducing a slow
  // publish without replacing capture or the real LiveKit publication.
  const gate=new Promise(resolve=>{window.__releaseShareAudio=resolve})
  p.publishTrack=async(track,options)=>{if(options?.name?.endsWith('-audio'))await gate;return publish(track,options)}
 })
 await a.getByLabel('Share screen',{exact:true}).first().click()
 if(phase==='after'){
  await a.getByLabel('Share another',{exact:true}).first().waitFor()
  assert(await a.getByLabel('Share another',{exact:true}).first().isDisabled(),'another share waits for paired publication')
  await a.evaluate(()=>window.__releaseShareAudio())
 }
 await a.getByLabel('Share another',{exact:true}).first().click()
 await wait(async()=>await screens(a).count()===3,'three shares')
 await wait(async()=>await a.locator('video').evaluateAll(es=>es.every(e=>e.videoWidth>0)),'decoded videos')
 if(phase==='after'){assert(await a.getByTestId('call-dock').evaluate(dock=>{const outer=dock.getBoundingClientRect();return [...dock.querySelectorAll('.room,.controls > button,.controls > details > summary')].every(e=>{const r=e.getBoundingClientRect();return r.width>0&&r.x>=outer.x&&r.right<=outer.right})}), 'room name and all dock controls fit with multiple shares');await a.getByLabel('Your shares',{exact:true}).first().click();await a.locator('.shares [popover]').first().waitFor({state:'visible'});const box=await a.locator('.shares [popover]').first().boundingBox();assert(box.x>=0&&box.x+box.width<=1440,'dock share menu fits viewport')}
 await shot(a,'02-share-stop')
 if(phase==='after')await a.getByLabel('Your shares',{exact:true}).first().press('Escape')
 await shot(a,'03-share-names')
 if(phase==='before')await shot(a,'04-dock')
 if(phase==='before')console.log('Baseline: six matched screenshots captured; two local shares and one remote share decoded.')
 else {
  const labels=await screens(a).locator('.label').allTextContents()
  assert(labels.every(x=>!x.includes('37438947')),'numeric source handle hidden')
  assert(labels.some(x=>x.includes('Window'))&&labels.some(x=>x.includes('Helium — docs')),'fallback and real title')
  const handle=a.getByRole('separator',{name:'Call strip height'}),strip=a.getByTestId('call-strip')
  await record('04-drag-dock',async()=>{
   const r=await handle.boundingBox();await a.mouse.move(r.x+r.width/2,r.y+4);await a.mouse.down();await a.mouse.move(r.x+r.width/2,r.y+224,{steps:40});await a.mouse.up()
  })
  const resized=(await strip.boundingBox()).height;assert(resized>350,'pointer grows strip')
  await shot(a,'04-dock')
  await handle.press('ArrowUp');assert.equal((await strip.boundingBox()).height,resized-20)
  await handle.press('ArrowDown');assert.equal((await strip.boundingBox()).height,resized)
  await handle.dblclick();assert.equal((await strip.boundingBox()).height,160,'double click resets')
  await handle.press('Home');assert.equal((await strip.boundingBox()).height,100,'minimum height')
  assert(await strip.locator('.tile').evaluateAll(es=>es.every(e=>{const r=e.getBoundingClientRect(),p=e.closest('.call-view').getBoundingClientRect();return r.top>=p.top&&r.bottom<=p.bottom})), 'camera and share tiles fit the minimum height')
  await remote.getByLabel('Bob audio options').click();await remote.getByRole('slider').waitFor({state:'visible'})
  await remote.getByRole('slider').press('Escape');await handle.press('Enter')
  await handle.press('End');assert((await strip.boundingBox()).height<750,'maximum leaves chat space')
  await handle.press('Enter')
  await a.getByLabel('Expand call',{exact:true}).click()
  await record('02-stop-one-share',async()=>{
   const remaining=screens(a).filter({has:a.getByLabel('Stop sharing Helium — docs',{exact:true})})
   const time=await remaining.locator('video').evaluate(e=>e.currentTime)
   await screens(a).filter({has:a.getByLabel('Stop sharing Window',{exact:true})}).getByLabel('Stop sharing Window',{exact:true}).click()
   await wait(async()=>await screens(a).count()===2&&await screens(b).count()===2,'single share removed at publisher and viewer')
   await wait(async()=>await remaining.locator('video').evaluate(e=>e.currentTime)>time,'remaining share continues playing')
   const state=await a.evaluate(()=>window.__streamsCaptures.map(s=>s.getTracks().map(t=>t.readyState)))
   assert(state[0].every(x=>x==='ended')&&state[1].every(x=>x==='live'),'only chosen capture and paired audio ended')
  })
  await a.getByTestId('call-grid').getByLabel('Your shares',{exact:true}).click()
  await a.getByTestId('call-grid').locator('.shares').getByLabel('Stop sharing Helium — docs',{exact:true}).click()
  await wait(async()=>await screens(a).count()===1&&await screens(b).count()===1,'dock stops remaining local share')
  await a.getByLabel('Collapse call',{exact:true}).click()
  await handle.press('ArrowDown');await handle.press('ArrowDown')
  // Reload and rejoin gives new LiveKit identities and fresh audio elements.
  await a.reload();await a.getByLabel('Join hangout',{exact:true}).waitFor();await join(a)
  await wait(async()=>{const es=await audio();return es.length>=2&&es.every(e=>e.volume===.35)},'saved volume on new subscriptions')
  assert.equal((await strip.boundingBox()).height,200,'room height persists across reload')
  await a.getByLabel('Mute sound on this device',{exact:true}).click()
  await wait(async()=>!(await audio()).length,'global deafen removes audio')
  await a.getByLabel('Play sound on this device',{exact:true}).click()
  await wait(async()=>{const es=await audio();return es.length>=2&&es.every(e=>e.volume===.35)},'saved volume survives deafen cycle')
  await a.getByLabel('Share screen',{exact:true}).first().click();await a.getByLabel('Share another',{exact:true}).first().click()
  await wait(async()=>await screens(a).count()===3,'two shares for main stop')
  await a.getByLabel('Stop sharing screen',{exact:true}).first().click();await wait(async()=>await screens(a).count()===1,'main share button stops all local shares')
  console.log('Passed desktop controls, persistence and audio checks')
  await a.setViewportSize({width:390,height:844})
  await shot(a,'06-mobile')
  // Existing wallpaper uses scale(1.06), so root scrollWidth includes its
  // hidden 12px overhang. Check the actual shell and controls instead.
  assert(await a.locator('.main,.call-view,.call-view > .resize,.dock,.head').evaluateAll(es=>es.every(e=>{const r=e.getBoundingClientRect();return r.x>=-1&&r.right<=innerWidth+1})), 'mobile shell and call controls fit')
  await a.getByLabel('Call strip height').press('Home')
  await remote.getByLabel('Bob audio options').click()
  await remote.getByRole('slider').waitFor({state:'visible'});const popup=await remote.locator('.volume-panel').boundingBox();assert(popup.x>=0&&popup.x+popup.width<=390,'mobile volume fits')
  await shot(a,'06-mobile-volume')
  console.log('PASS: volume + mute + persistence + deafen; paired single-share stop and main stop; sanitized source labels; pointer/keyboard/reset/persisted dock; voice-first sidebar; mobile controls.')
 }
 assert.deepEqual(errors,[])
} catch(error) { for(const [i,c] of browser.contexts().entries()){const p=c.pages()[0];if(p){await p.screenshot({path:`${shots}failure-${i}.png`}).catch(()=>{});console.error('Visible error',await p.locator('.call-status,[role=alert]').allTextContents().catch(()=>[]))}} throw error } finally { await browser.close() }
