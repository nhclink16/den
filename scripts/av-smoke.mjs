import { chromium } from 'playwright-core'
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { mkdir, writeFile } from 'node:fs/promises'
const base = process.env.DEN_SMOKE_URL || 'http://localhost:5178'
const observer = process.env.DEN_SMOKE_OBSERVER || 'av_observer'
const shots = 'docs/shots/pr/feat/av-extensions'
const smokeStarted = performance.now()
await mkdir(shots,{recursive:true})
const gpuArgs=process.env.DEN_SMOKE_VULKAN==='1'?['--use-angle=vulkan','--enable-gpu','--ignore-gpu-blocklist']:[]
const browser = await chromium.launch({executablePath:'/usr/bin/chromium',headless:!process.env.DISPLAY,args:['--no-sandbox','--use-fake-ui-for-media-stream','--use-fake-device-for-media-stream=fps=60','--autoplay-policy=no-user-gesture-required',...gpuArgs,...(process.env.DEN_SMOKE_AUDIO ? [`--use-file-for-fake-audio-capture=${process.env.DEN_SMOKE_AUDIO}`] : [])]})
const errors = [], results = {}
async function until(check, label, timeout=20000) {
 const end=Date.now()+timeout
 while(Date.now()<end) { if(await check()) return; await new Promise(r=>setTimeout(r,100)) }
 throw Error(`Timed out: ${label}`)
}
async function login(username='nicholas') {
 const context=await browser.newContext({viewport:{width:1440,height:1000},permissions:['microphone','camera'],colorScheme:'dark'})
 await context.addInitScript(()=> {
  window.__pcs=[]; window.__tracks=[]; window.__gum=[]
  const PC=window.RTCPeerConnection
  window.RTCPeerConnection=class extends PC { constructor(...args){super(...args);window.__pcs.push(this)} }
  const gum=navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices)
  navigator.mediaDevices.getUserMedia=async (...args)=>{
   const options=args[0],audio=typeof options?.audio==='object'?options.audio:null,deviceId=audio?.deviceId
   window.__gum.push({audio:audio?{deviceId:typeof deviceId==='object'?{exact:deviceId.exact,ideal:deviceId.ideal}:deviceId}:options?.audio,video:Boolean(options?.video)})
   const s=await gum(...args);window.__tracks.push(...s.getTracks());return s
  }
 })
 const p=await context.newPage();p.on('pageerror',e=>errors.push(e.message))
 await p.goto(base)
 await p.getByLabel('Username',{exact:true}).fill(username)
 await p.getByLabel('Password',{exact:true}).fill(process.env.DEN_SMOKE_PASSWORD)
 await p.getByRole('button',{name:'Come in',exact:true}).click()
 await p.locator('a[title="Settings"]').waitFor()
 return p
}
async function voice(p) {await p.locator('a[title="Settings"]').click();await p.getByRole('link',{name:'Voice',exact:true}).click(); await p.getByRole('meter').waitFor()}
async function remoteVideoFrames(p) {return p.evaluate(async()=>{
 const reports=await Promise.all(window.__pcs.filter(pc=>pc.connectionState==='connected').map(pc=>pc.getStats()))
 return reports.flatMap(r=>[...r.values()]).filter(s=>s.type==='inbound-rtp'&&s.kind==='video').reduce((sum,s)=>sum+(s.framesDecoded||0),0)
})}
async function requireRemoteVideo(p,label) {
 const before=await remoteVideoFrames(p)
 await until(async()=>await remoteVideoFrames(p)>before,`remote video after ${label}`,5_000)
 return {label,before,after:await remoteVideoFrames(p)}
}
async function inspect(p) {return p.evaluate(async()=>{
 const url=performance.getEntriesByType('resource').find(e=>new URL(e.name).pathname==='/src/lib/call.svelte.ts').name
 const {call}=await import(url)
 const camera=call.room?.localParticipant.getTrackPublication('camera')?.track
 return {error:call.error,joining:call.joining,state:call.room?.state,identity:call.room?.localParticipant.identity,participants:call.participants.length,cameraOn:call.cameraOn,
  camera:call.cameraTrack?.getSettings(),cameraId:call.cameraTrack?.id,cameraSid:call.room?.localParticipant.getTrackPublication('camera')?.trackSid,
  background:call.cameraSettings.background,backgroundBusy:call.backgroundBusy,blurNotice:call.blurNotice,blurActive:call.blurActive,blurProcessor:camera?.getProcessor?.()?.name,
  blurRadius:call.blur?.inner?.transformer?.options?.blurRadius,blurSourceId:call.blur?.source?.id,outputId:camera?.mediaStreamTrack?.id,
  audio:call.gain.source?.getSettings(),audioId:call.gain.source?.id,processorId:call.gain.processedTrack?.id,sourceState:call.gain.source?.readyState,
  pcs:window.__pcs.map(pc=>pc.connectionState)}
})}
let a, b
try {
 a=await login(); await voice(a)
 await until(()=>a.getByLabel('Camera preview',{exact:true}).evaluate(v=>v.videoWidth>0).catch(()=>false),'settings preview')
 console.log('Preview ready', await a.getByLabel('Resolution',{exact:true}).innerText(),await a.getByLabel('Frame rate',{exact:true}).innerText())
 const cameraPicker=a.getByRole('combobox',{name:'Camera',exact:true})
 const selectedCamera=await cameraPicker.locator('option').evaluateAll(options=>options.map(o=>o.value).find(Boolean))
 if(selectedCamera) {
   await cameraPicker.selectOption(selectedCamera)
   await until(()=>a.getByLabel('Camera preview',{exact:true}).evaluate(v=>v.videoWidth>0),'selected camera preview')
 }
 const lightBlur=a.getByRole('radio',{name:'Light blur',exact:true})
 assert(await lightBlur.isEnabled(),'modern Chromium offers background blur')
 await lightBlur.check()
 await until(()=>lightBlur.isChecked(),'light blur saved')
 await until(()=>a.getByLabel('Camera preview',{exact:true}).evaluate(v=>{
   const shown=v.srcObject?.getVideoTracks()[0]
   return shown?.readyState==='live' && !window.__tracks.includes(shown)
 }),'owned preview uses processed track')
 await until(()=>a.evaluate(()=>performance.getEntriesByType('resource').some(e=>new URL(e.name).pathname.endsWith('/blur/selfie_segmenter.tflite'))),'self-hosted model loaded')
 assert(await a.evaluate(()=>performance.getEntriesByType('resource').filter(e=>/mediapipe|selfie_segmenter|vision_wasm/.test(e.name)).every(e=>new URL(e.name).origin===location.origin)),'blur assets stay same-origin')
 const backgroundSync=await login();await voice(backgroundSync)
 await until(()=>backgroundSync.evaluate(async id=>{
   const url=performance.getEntriesByType('resource').find(e=>new URL(e.name).pathname==='/src/lib/store.svelte.ts').name
   const {instances}=await import(url)
   return instances.active.voice.cameras[id]?.background==='light_blur'
 },selectedCamera),'background preference synced to same account')
 await backgroundSync.context().close()
 await a.screenshot({path:`${shots}/after.png`})
 await a.locator('nav.side').getByRole('button',{name:'Join '+(process.env.DEN_SMOKE_CHANNEL || 'av-verification'),exact:true}).click()
 await until(async()=>(await inspect(a)).state==='connected','call connected')
 if(await a.getByRole('button',{name:'Unmute microphone',exact:true}).count()) await a.getByRole('button',{name:'Unmute microphone',exact:true}).click()
 await until(async()=>(await inspect(a)).processorId,'gain processor publishing')
 await until(async()=>(await inspect(a)).joining===null,'join finished')
 b=await login(observer)
 await b.locator('nav.side').getByRole('button',{name:'Join '+(process.env.DEN_SMOKE_CHANNEL || 'av-verification'),exact:true}).click()
 await until(async()=>(await inspect(b)).participants===2,'remote participant joined')
 await a.getByRole('button',{name:'Turn camera on',exact:true}).click()
 await until(async()=>(await inspect(a)).cameraId,'camera publishing')
 await until(()=>b.locator('[data-local="false"] video').evaluateAll(v=>v.some(e=>e.videoWidth>0)),'remote camera decodes')
 await until(async()=>{const live=await inspect(a);return live.blurProcessor==='den-background-blur'&&live.blurRadius===5},'saved light blur publishes')
 const blurInitial=await inspect(a)
 assert.equal(blurInitial.background,'light_blur')
 assert.notEqual(blurInitial.outputId,blurInitial.blurSourceId)
 assert((blurInitial.camera?.frameRate||0)<=30,'blur caps modern processing at 30 fps')
 const remoteContinuity=[]
 remoteContinuity.push(await requireRemoteVideo(b,'saved light blur'))
 await a.getByRole('button',{name:'Turn background blur off',exact:true}).first().click()
 await until(async()=>!(await inspect(a)).blurProcessor,'toolbar removes saved light blur')
 remoteContinuity.push(await requireRemoteVideo(b,'saved light blur off'))
 await a.getByRole('button',{name:'Blur my background',exact:true}).first().click()
 await until(async()=>{const live=await inspect(a);return live.blurProcessor==='den-background-blur'&&live.blurRadius===5},'toolbar restores saved light blur')
 remoteContinuity.push(await requireRemoteVideo(b,'saved light blur restored'))
 const fullBlur=a.getByRole('radio',{name:'Blur',exact:true})
 await fullBlur.check()
 await until(async()=>{const live=await inspect(a);return live.background==='blur'&&live.blurRadius===10},'switch to full blur')
 const blurSwitched=await inspect(a)
 assert.equal(blurSwitched.identity,blurInitial.identity)
 assert.equal(blurSwitched.cameraSid,blurInitial.cameraSid)
 assert.equal(blurSwitched.blurSourceId,blurInitial.blurSourceId)
 remoteContinuity.push(await requireRemoteVideo(b,'full blur'))
 await a.screenshot({path:`${shots}/blur-active.png`})
 await a.getByRole('radio',{name:'None',exact:true}).check()
 await until(async()=>!(await inspect(a)).blurProcessor,'remove blur')
 const unblurred=await inspect(a)
 assert.equal(unblurred.identity,blurInitial.identity)
 assert.equal(unblurred.cameraSid,blurInitial.cameraSid)
 remoteContinuity.push(await requireRemoteVideo(b,'settings blur off'))
 const quickBlur=a.getByRole('button',{name:'Blur my background',exact:true}).first()
 await quickBlur.click()
 await until(async()=>{const live=await inspect(a);return live.blurProcessor==='den-background-blur'&&live.blurRadius===10},'toolbar restores last blur')
 remoteContinuity.push(await requireRemoteVideo(b,'toolbar full blur'))
 const beforeCameraMute=await inspect(a)
 await a.getByRole('button',{name:'Turn camera off',exact:true}).click()
 await until(async()=>!(await inspect(a)).cameraOn,'camera muted with blur preference retained')
 await a.getByRole('button',{name:'Turn camera on',exact:true}).click()
 await until(async()=>{const live=await inspect(a);return live.cameraOn&&live.blurProcessor==='den-background-blur'&&live.background==='blur'},'camera resumes with blur')
 const resumed=await inspect(a)
 assert.notEqual(resumed.outputId,beforeCameraMute.outputId,'camera resume installs a fresh blur pipeline')
 remoteContinuity.push(await requireRemoteVideo(b,'camera resume'))
 await a.getByRole('button',{name:'Turn background blur off',exact:true}).first().click()
 await until(async()=>!(await inspect(a)).blurProcessor,'toolbar removes blur for camera controls')
 remoteContinuity.push(await requireRemoteVideo(b,'final blur off'))
 results.backgroundBlur={
   ownedPreview:true,sameOriginAssets:true,accountSync:true,
   lightRadius:blurInitial.blurRadius,fullRadius:blurSwitched.blurRadius,
   sameParticipant:blurSwitched.identity===blurInitial.identity,
   samePublication:blurSwitched.cameraSid===blurInitial.cameraSid,
   sameSource:blurSwitched.blurSourceId===blurInitial.blurSourceId,
   savedLightRestored:true,cameraResume:true,freshProcessorAfterResume:true,remoteContinuity,
 }
 const meter=a.getByRole('meter',{name:'Microphone level'})
 const slider=a.getByRole('slider',{name:'Input gain',exact:true})
 const source=await inspect(a)
 results.microphoneProcessing={}
 for(const [label,key] of [['Echo cancellation','echoCancellation'],['Noise suppression','noiseSuppression'],['Auto gain','autoGainControl']]) {
   const box=a.getByRole('checkbox',{name:label,exact:true}), before=await box.isChecked()
   await box.click()
   await until(()=>box.isEnabled(),`${label} completed`)
   const live=await inspect(a)
   const changed=live.audio[key]===!before
   const failure=(await a.getByRole('alert').allTextContents()).some(t=>t.includes('Microphone setting was not saved'))
   assert(changed || failure,`${label} must apply or report failure`)
   assert.equal(live.audioId,source.audioId)
   results.microphoneProcessing[label]=changed?'applied live':'synthetic device rejected live change; UI reported and reverted'
   if(changed) {await box.click();await until(()=>box.isEnabled(),'processing restored')}
 }
 async function callTrackSample(label) {
  const sample=await a.evaluate(async label=>{
   const url=performance.getEntriesByType('resource').find(e=>new URL(e.name).pathname==='/src/lib/call.svelte.ts').name
   const {call}=await import(url)
   const track=call.gain.processedTrack,raw=call.gain.source
   const trackState=t=>t?{id:t.id,readyState:t.readyState,enabled:t.enabled,muted:t.muted,settings:t.getSettings(),constraints:t.getConstraints()}:null
   const ctx=new AudioContext(),contextBefore=ctx.state;await ctx.resume()
   const analyser=ctx.createAnalyser();analyser.fftSize=2048
   const stream=new MediaStream([track]),source=ctx.createMediaStreamSource(stream);source.connect(analyser)
   const contextTimeBefore=ctx.currentTime,started=performance.now(),windows=[]
   let previous
   for(const wait of [250,100,100]) {
    await new Promise(r=>setTimeout(r,wait))
    const data=new Float32Array(analyser.fftSize);analyser.getFloatTimeDomainData(data)
    windows.push({
     atMs:performance.now()-started,
     rms:Math.sqrt(data.reduce((s,n)=>s+n*n,0)/data.length),
     peak:data.reduce((m,n)=>Math.max(m,Math.abs(n)),0),
     nonzeroSamples:data.reduce((n,v)=>n+(Math.abs(v)>1e-7?1:0),0),
     changedSamples:previous?data.reduce((n,v,i)=>n+(v!==previous[i]?1:0),0):null,
    })
    previous=data
   }
   const result={label,browserNowMs:performance.now(),contextBefore,contextDuring:ctx.state,contextTimeBefore,contextTimeAfter:ctx.currentTime,
    streamActive:stream.active,sourceNode:{channelCount:source.channelCount,channelCountMode:source.channelCountMode,inputs:source.numberOfInputs,outputs:source.numberOfOutputs},
    processed:trackState(track),raw:trackState(raw),gainValue:call.gain.gain?.gain?.value??null,windows}
   source.disconnect();await ctx.close();return {...result,contextAfter:ctx.state}
  },label)
  sample.smokeElapsedMs=performance.now()-smokeStarted
  console.log(`Call gain diagnostic ${label} ${JSON.stringify(sample)}`)
  return sample
 }
 results.callGainDiagnostic={baseline:await callTrackSample('baseline')}
 await slider.focus(); await slider.press('Home')
 await until(async()=>+(await meter.getAttribute('aria-valuenow'))===0,'zero gain meter')
 results.callGainDiagnostic.zero=await callTrackSample('zero')
 results.zeroGainRms=results.callGainDiagnostic.zero.windows[0].rms;assert(results.zeroGainRms<0.0001)
 await until(()=>slider.isEnabled(),'gain save complete');await slider.press('End')
 await until(async()=>+(await meter.getAttribute('aria-valuenow'))>0,'gain meter returns')
 results.callGainDiagnostic.double=await callTrackSample('double')
 results.doubleGainRms=results.callGainDiagnostic.double.windows[0].rms;assert(results.doubleGainRms>0.001)
 assert.equal((await inspect(a)).processorId,source.processorId)
 // Same-account second browser receives private mic preferences over the real WebSocket.
 const sync=await login();await voice(sync)
 await until(async()=>(await sync.getByRole('slider',{name:'Input gain',exact:true}).inputValue())==='2','account sync')
 await until(()=>slider.isEnabled(),'gain save complete');await slider.click({position:{x:150,y:14}})
 await until(async()=>(await sync.getByRole('slider',{name:'Input gain',exact:true}).inputValue())!=='2','live account update')
 await sync.context().close()
 // Muting still gates the processed outgoing track.
 await a.getByRole('button',{name:'Mute microphone',exact:true}).click()
 await until(()=>b.locator('[data-local="false"]').getByTestId('muted-mic').count().then(n=>n===1),'remote mute')
 await a.getByRole('button',{name:'Unmute microphone',exact:true}).click()
 await until(()=>b.locator('[data-local="false"]').getByTestId('muted-mic').count().then(n=>n===0),'remote unmute')

 const inputPicker=a.getByRole('combobox',{name:'Microphone',exact:true})
 // Fake-device Chromium may expose only aliases for one physical input. The
 // browser result cannot prove a hardware switch, but the capture request can
 // deterministically prove that an explicit picker choice uses an exact ID.
 const selectedMicrophone=await inputPicker.locator('option').evaluateAll(options=>options.map(option=>option.value).find(value=>value&&!['default','communications'].includes(value)))
 assert(selectedMicrophone,'synthetic microphone has a stable non-default ID')
 const gumBefore=await a.evaluate(()=>window.__gum.length)
 await inputPicker.selectOption(selectedMicrophone)
 await until(()=>a.evaluate(start=>window.__gum.slice(start).some(options=>typeof options.audio==='object'),gumBefore),'microphone reacquisition')
 const selectedCapture=await a.evaluate(start=>window.__gum.slice(start).find(options=>typeof options.audio==='object'),gumBefore)
 assert.equal(selectedCapture.audio.deviceId?.exact,selectedMicrophone,'the call microphone reacquisition uses the exact selected ID')
 assert.equal((await inspect(a)).state,'connected')
 results.microphoneSwitch={exactConstraint:true,hardwareSwitch:'not claimed: fixture exposes one physical microphone'}
 const restoreBefore=await a.evaluate(()=>window.__gum.length)
 await inputPicker.selectOption('')
 await until(()=>a.evaluate(start=>window.__gum.slice(start).some(options=>options.audio?.deviceId==='default'),restoreBefore),'restore default microphone request')
 const initial=await inspect(a)
 let recording=false, capture, encoder
 if(process.env.DEN_SMOKE_RECORD) {
   encoder=spawn('ffmpeg',['-hide_banner','-loglevel','error','-y','-f','image2pipe','-framerate','2','-vcodec','png','-i','pipe:0','-an','-c:v','libx264','-threads','1','-pix_fmt','yuv420p','-movflags','+faststart',`${shots}/resolution-mid-call.mp4`])
   recording=true
   capture=(async()=>{while(recording){const frame=await a.screenshot();if(!encoder.stdin.write(frame))await new Promise(r=>encoder.stdin.once('drain',r));await new Promise(r=>setTimeout(r,200))}})()
   await a.waitForTimeout(1000)
 }
 await a.getByLabel('Resolution',{exact:true}).selectOption('720p')
 await until(async()=>(await inspect(a)).camera?.height===720,'720p applied')
 if(recording) await a.waitForTimeout(1600)
 await a.getByLabel('Resolution',{exact:true}).selectOption('1080p')
 await until(async()=>(await inspect(a)).camera?.height===1080,'1080p applied')
 if(recording) {await a.waitForTimeout(2000);recording=false;await capture;encoder.stdin.end();await new Promise((resolve,reject)=>encoder.on('exit',code=>code===0?resolve():reject(Error('ffmpeg failed'))))}
 for(const rate of ['24','60','30']) {
   await a.getByLabel('Frame rate',{exact:true}).selectOption(rate)
   await until(async()=>(await inspect(a)).camera?.frameRate===+rate,`${rate} fps applied`)
 }
 await a.getByRole('checkbox',{name:'Mirror my preview',exact:true}).uncheck()
 await until(()=>a.getByLabel('Camera preview',{exact:true}).evaluate(v=>getComputedStyle(v).transform==='none'),'local mirror disabled')
 assert.equal(await b.locator('[data-local="false"] video').evaluate(v=>getComputedStyle(v).transform),'none')
 await a.getByRole('checkbox',{name:'Mirror my preview',exact:true}).check()
 const final=await inspect(a)
 assert.equal(final.identity,initial.identity)
 assert.equal(final.cameraId,initial.cameraId)
 assert.equal(final.cameraSid,initial.cameraSid)
 results.resolutionContinuity={sameParticipant:final.identity===initial.identity,sameTrack:final.cameraId===initial.cameraId,samePublication:final.cameraSid===initial.cameraSid,settings:{width:final.camera.width,height:final.camera.height,frameRate:final.camera.frameRate}}
 assert.equal(final.state,'connected')
 results.remoteMedia=await b.evaluate(async()=>{
   const reports=await Promise.all(window.__pcs.filter(pc=>pc.connectionState==='connected').map(pc=>pc.getStats()))
   return reports.flatMap(r=>[...r.values()].filter(s=>s.type==='inbound-rtp').map(s=>({kind:s.kind,bytesReceived:s.bytesReceived,framesDecoded:s.framesDecoded,totalAudioEnergy:s.totalAudioEnergy})))
 })
 assert(results.remoteMedia.some(s=>s.kind==='audio'&&s.bytesReceived>0))
 assert(results.remoteMedia.some(s=>s.kind==='video'&&s.framesDecoded>0))
 await a.getByRole('button',{name:'Pause preview',exact:true}).click()
 assert.equal((await inspect(a)).cameraId,initial.cameraId)
 await a.getByRole('button',{name:'Resume preview',exact:true}).click()
 await until(()=>a.getByLabel('Camera preview',{exact:true}).evaluate(v=>v.videoWidth===1920),'preview resumes')
 await a.screenshot({path:`${shots}/during-call.png`})
 await a.getByRole('link',{name:'Notifications',exact:true}).click()
 assert.equal((await inspect(a)).cameraId,initial.cameraId)
 // Closing Voice settings stops only its owned captures.
 const cleanup=await a.evaluate(()=>window.__tracks.filter(t=>t.readyState==='live').map(t=>({id:t.id,kind:t.kind})))
 assert.equal(cleanup.length,2,JSON.stringify(cleanup))
 await voice(a)
 await until(()=>a.getByLabel('Camera preview',{exact:true}).evaluate(v=>v.videoWidth>0),'preview reopens')
 assert.equal(await a.getByLabel('Resolution',{exact:true}).inputValue(),'1080p')
 assert.equal(await a.getByRole('slider',{name:'brightness',exact:true}).count(),0)
 await b.getByRole('button',{name:'Leave call',exact:true}).click()
 await b.context().close()

 await a.getByRole('button',{name:'Leave call',exact:true}).click()
 await a.reload(); await until(()=>a.getByLabel('Camera preview',{exact:true}).evaluate(v=>v.videoWidth>0),'preview after reload')
 assert.equal(await a.getByLabel('Resolution',{exact:true}).inputValue(),'1080p')
 await a.screenshot({path:`${shots}/after.png`})
 await a.setViewportSize({width:390,height:844}); await a.screenshot({path:`${shots}/mobile.png`})
 assert(await a.locator('.devices').evaluate(e=>e.getBoundingClientRect().right<=innerWidth),'device controls fit mobile viewport')
 results.browserErrors=errors;results.passed=true
 await writeFile(`${shots}/verification.json`,JSON.stringify(results,null,2)+'\n')
 console.log('AV smoke passed',JSON.stringify(results))
 assert.deepEqual(errors,[])
} catch(e) {console.error(e); if(a) {console.log('Failed state',await inspect(a));await a.screenshot({path:`${shots}/failure.png`});console.error('Alerts',await a.getByRole('alert').allTextContents())}process.exitCode=1}
finally{await browser.close()}
