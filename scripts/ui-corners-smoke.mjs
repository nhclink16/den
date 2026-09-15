// Explicit disconnected-call UI fixture: verifies theme geometry, not LiveKit behavior.
import {chromium} from 'playwright-core'
import {mkdir,writeFile} from 'node:fs/promises'
import assert from 'node:assert/strict'
const base=process.env.DEN_SMOKE_URL||'http://localhost:5183',before=process.env.BEFORE==='1',phase=before?'before':'after'
const shots=new URL('../docs/shots/pr/fix/ui-appearance/',import.meta.url).pathname;await mkdir(shots,{recursive:true})
const login=await fetch(base+'/auth/login',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({username:'nicholas',password:process.env.DEN_SMOKE_PASSWORD})}).then(r=>r.json())
const browser=await chromium.launch({executablePath:'/usr/bin/chromium',headless:true,args:['--no-sandbox']})
const context=await browser.newContext({viewport:{width:1440,height:900},colorScheme:'light'});await context.addCookies([{name:'den_session',value:login.token,url:base,httpOnly:true,sameSite:'Lax'}]);await context.addInitScript(csrf=>localStorage.setItem('den.csrf',csrf),login.csrf_token)
const page=await context.newPage(),metrics={}
try{
 await page.goto(base);await page.locator('.composer textarea').waitFor()
 await page.evaluate(async()=>{const {store}=await import('/src/lib/store.svelte.ts'),{router}=await import('/src/lib/router.svelte.ts'),{call}=await import('/src/lib/call.svelte.ts'),{themes}=await import('/src/lib/theme.svelte.ts');await themes.select(themes.all.find(t=>t.id==='den'),'light');await themes.mode('light');const room=store.textChannels.find(c=>c.kind==='text');router.go('/c/'+room.id);call.origin=store.origin;call.instanceName='Den';call.channel=room;call.participants=[{id:'ui-review',userId:store.me.id,name:store.me.display_name||store.me.username,local:true,muted:true,quality:'excellent',speaking:false,screens:[]}]})
 for(const radius of ['sharp','round']){
  await page.evaluate(async radius=>{const {themes}=await import('/src/lib/theme.svelte.ts');themes.preview({...JSON.parse(JSON.stringify(themes.active)),radius});(await import('/src/lib/call.svelte.ts')).call.expanded=false},radius)
  await page.locator('.composer textarea').fill('/');await page.waitForTimeout(200)
  metrics[radius]=await page.locator('.controls button,.attach,.sendbtn,.dictate').evaluateAll(es=>es.map(e=>({label:e.getAttribute('aria-label'),radius:getComputedStyle(e).borderRadius})))
  if(!before)assert(metrics[radius].every(v=>v.radius===(radius==='sharp'?'2px':'10px')))
  await page.screenshot({path:`${shots}corners-${radius}-${phase}.png`})
  await page.evaluate(async()=>(await import('/src/lib/call.svelte.ts')).call.expanded=true);await page.waitForTimeout(200)
  await page.screenshot({path:`${shots}corners-${radius}-expanded-${phase}.png`})
  if(!before)assert.equal(await page.locator('.toolbar').evaluate(e=>getComputedStyle(e).borderRadius),radius==='sharp'?'6px':'18px')
 }
 console.log(JSON.stringify(metrics,null,2));await writeFile(`${shots}corners-${phase}.json`,JSON.stringify(metrics,null,2)+'\n')
}finally{await browser.close()}
