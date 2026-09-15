// Local isolated data only; creates and removes its own room. BEFORE=1 captures the original UI.
import {chromium} from 'playwright-core'
import {mkdir,writeFile,mkdtemp,readFile} from 'node:fs/promises'
import {execFileSync} from 'node:child_process'
import assert from 'node:assert/strict'
const base=process.env.DEN_SMOKE_URL||'http://localhost:5183',before=process.env.BEFORE==='1',phase=before?'before':'after'
const shots=new URL('../docs/shots/pr/fix/ui-conversations/',import.meta.url).pathname;await mkdir(shots,{recursive:true});const scratch=await mkdtemp('/mnt/storage/ui-conversations-')
let token,room,browser,context,page,videoStart=0,videoEnd=0,recordedAt=0;const api=async(method,path,body)=>{const r=await fetch(base+path,{method,headers:{'content-type':'application/json',...(token?{authorization:`Bearer ${token}`}:{})},body:body?JSON.stringify(body):undefined});assert(r.ok,`${method} ${path}: ${r.status}`);return r.status===204?null:r.json()}
const metrics={}
try{
 const login=await api('POST','/auth/login',{username:'nicholas',password:process.env.DEN_SMOKE_PASSWORD});token=login.token
 room=await api('POST','/channels',{name:'reading-review',kind:'text',position:0})
 for(let i=0;i<12;i++)await api('POST',`/channels/${room.id}/messages`,{content:`Reading note ${i+1}. The next evening we met in the same room. A short paragraph should stay comfortable to read, with enough space between the lines to follow the conversation.`})
 execFileSync('ffmpeg',['-y','-f','lavfi','-i','testsrc2=size=640x480','-frames:v','1',`${scratch}/image.png`],{stdio:'ignore'});const data=await readFile(`${scratch}/image.png`)
 const upload=await api('POST','/uploads',{channel_id:room.id,filename:'color-study.png',content_type:'image/png',size:data.length})
 const chunk=await fetch(base+`/uploads/${upload.id}`,{method:'PATCH',headers:{authorization:`Bearer ${token}`,'upload-offset':'0'},body:data});assert(chunk.ok);await api('POST',`/uploads/${upload.id}/complete`,{})
 await api('POST',`/channels/${room.id}/messages`,{content:'A color study for the next room.',upload_ids:[upload.id]})
 browser=await chromium.launch({executablePath:'/usr/bin/chromium',headless:true,args:['--no-sandbox']});context=await browser.newContext({viewport:{width:1440,height:900},colorScheme:'dark',recordVideo:{dir:scratch,size:{width:1440,height:900}}});await context.addCookies([{name:'den_session',value:token,url:base,httpOnly:true,sameSite:'Lax'}]);await context.addInitScript(csrf=>localStorage.setItem('den.csrf',csrf),login.csrf_token)
 page=await context.newPage();recordedAt=Date.now();let release;const hold=new Promise(r=>release=r);await page.route(`**/uploads/${upload.id}/**`,async route=>{await hold;await route.continue()})
 await page.goto(base+`/c/${room.id}`,{waitUntil:'domcontentloaded'});await page.locator('.msg').first().waitFor()
 await page.evaluate(async()=>{const {themes}=await import('/src/lib/theme.svelte.ts');await themes.select(themes.all.find(t=>t.id==='den'),'dark');await themes.mode('dark')})
 await page.locator('.list').evaluate(el=>el.scrollTop=el.scrollHeight);await page.waitForTimeout(300)
 videoStart=(Date.now()-recordedAt)/1000;await page.waitForTimeout(600);
 metrics.loadingHeight=await page.locator('img.media').evaluate(el=>el.getBoundingClientRect().height);await page.screenshot({path:`${shots}image-loading-${phase}.png`})
 release();await page.locator('img.media').evaluate(el=>el.decode());await page.waitForTimeout(300)
 metrics.loadedHeight=await page.locator('img.media').evaluate(el=>el.getBoundingClientRect().height);metrics.bottomGap=await page.locator('.list').evaluate(el=>el.scrollHeight-el.scrollTop-el.clientHeight)
 await page.screenshot({path:`${shots}image-loaded-${phase}.png`});await page.waitForTimeout(600);videoEnd=(Date.now()-recordedAt)/1000;if(!before){assert(metrics.loadingHeight>100);assert(Math.abs(metrics.loadingHeight-metrics.loadedHeight)<1);assert(metrics.bottomGap<2)}
 // A second delayed decode must not pull a reader back down after they scroll up.
 let releaseReading;const readingHold=new Promise(r=>releaseReading=r)
 await page.route('**/*reading-check=1',async route=>{await readingHold;await route.continue()})
 await page.locator('img.media').evaluate(el=>{el.src += '?reading-check=1'})
 await page.locator('.list').evaluate(el=>el.scrollTop=150);await page.waitForTimeout(150)
 const readingTop=await page.locator('.list').evaluate(el=>el.scrollTop)
 releaseReading();await page.locator('img.media').evaluate(el=>el.decode());await page.waitForTimeout(150)
 metrics.readingScrollDelta=await page.locator('.list').evaluate((el,top)=>el.scrollTop-top,readingTop)
 if(!before)assert(Math.abs(metrics.readingScrollDelta)<1)
 await page.locator('.list').evaluate(el=>el.scrollTop=0);await page.waitForTimeout(100)
 metrics.density={};for(const density of ['comfortable','compact']){await page.evaluate(async density=>{const {themes}=await import('/src/lib/theme.svelte.ts');themes.preview({...JSON.parse(JSON.stringify(themes.active)),density})},density);await page.waitForTimeout(150);metrics.density[density]=await page.locator('.msg .text').first().evaluate(el=>getComputedStyle(el).lineHeight);await page.screenshot({path:`${shots}density-${density}-${phase}.png`})}
 if(!before)assert(parseFloat(metrics.density.compact)<parseFloat(metrics.density.comfortable))
 await page.evaluate(async()=>{const {themes}=await import('/src/lib/theme.svelte.ts');themes.reset();const {applyBackground,builtinThemes}=await import('/src/lib/theme-runtime.ts');applyBackground({source:{type:'builtin',name:'aurora'},scope:'app',blur:0,dim:0,saturate:100,fit:'cover'},'dark',builtinThemes[0].dark)})
 await page.waitForTimeout(200);await page.screenshot({path:`${shots}wallpaper-${phase}.png`})
 metrics.membersLayers=await page.locator('aside.members').evaluate(el=>[el,...el.querySelectorAll('*')].filter(n=>getComputedStyle(n).backdropFilter!=='none').length)
 if(!before)assert.equal(metrics.membersLayers,1)
 await page.locator('.msg').nth(1).hover();await page.screenshot({path:`${shots}labels-${phase}.png`})
 metrics.timestamp=await page.locator('.stamp').first().evaluate(el=>getComputedStyle(el).fontSize);if(!before)assert(parseFloat(metrics.timestamp)>=11)
 await page.setViewportSize({width:390,height:844});await page.locator('.list').evaluate(el=>el.scrollTop=el.scrollHeight);await page.waitForTimeout(150)
 metrics.mobileImageRight=await page.locator('img.media').evaluate(el=>el.getBoundingClientRect().right)
 await page.screenshot({path:`${shots}image-mobile-${phase}.png`})
 if(!before)assert(metrics.mobileImageRight<=390)
 // Disconnected call fixture: display typography only, no network/media quality claim.
 await page.setViewportSize({width:1440,height:900})
 await page.evaluate(async()=>{
  const {call}=await import('/src/lib/call.svelte.ts'),{store}=await import('/src/lib/store.svelte.ts')
  const source=await fetch('/src/lib/call.svelte.ts').then(r=>r.text())
  const {LocalVideoTrack}=await import(source.match(/from\s+["']([^"']*livekit-client[^"']*)["']/)[1])
  const canvas=document.createElement('canvas');canvas.width=640;canvas.height=360
  const ctx=canvas.getContext('2d');ctx.fillStyle='#374b51';ctx.fillRect(0,0,640,360);ctx.fillStyle='#eee';ctx.font='28px sans-serif';ctx.fillText('Call label fixture',185,180)
  const track=new LocalVideoTrack(canvas.captureStream().getVideoTracks()[0]);let frames=0
  track.getSenderStats=async()=>[{streamId:'fixture',framesSent:++frames,frameHeight:1080,framesPerSecond:30}]
  call.origin=store.origin;call.instanceName='Den';call.channel=store.textChannels.find(c=>c.name==='reading-review');call.expanded=true
  call.participants=[{id:'label-fixture',userId:store.me.id,name:'nicholas',local:true,muted:true,quality:'excellent',speaking:false,screens:[],camera:track}]
 })
 await page.getByTestId('sent-quality').waitFor();await page.waitForTimeout(100)
 metrics.qualityLabel=await page.getByTestId('sent-quality').evaluate(el=>({text:el.textContent,size:getComputedStyle(el).fontSize}))
 if(!before){assert.equal(metrics.qualityLabel.size,'11px');assert.equal(metrics.qualityLabel.text.trim(),'1080p · 30')}
 await page.screenshot({path:`${shots}call-label-${phase}.png`})
 console.log(JSON.stringify(metrics,null,2));await writeFile(`${shots}verification-${phase}.json`,JSON.stringify(metrics,null,2)+'\n')
}finally{
 if(context){await context.close();const video=await page.video().path();if(videoEnd>videoStart)execFileSync('ffmpeg',['-y','-ss',String(videoStart),'-to',String(videoEnd),'-i',video,'-an','-c:v','libx264','-preset','fast','-crf','25','-pix_fmt','yuv420p',`${shots}image-loading-${phase}.mp4`],{stdio:'ignore'})}
 await browser?.close();if(room)await api('DELETE',`/channels/${room.id}`)
}
