// Disposable local server + Vite only. BEFORE=1 records the review's original behavior.
import {chromium} from 'playwright-core'
import {mkdir,writeFile,mkdtemp} from 'node:fs/promises'
import {execFileSync} from 'node:child_process'
import assert from 'node:assert/strict'
const base=process.env.DEN_SMOKE_URL||'http://localhost:5183', before=process.env.BEFORE==='1', phase=before?'before':'after'
const shots=new URL('../docs/shots/pr/fix/ui-navigation/',import.meta.url).pathname
await mkdir(shots,{recursive:true});const scratch=await mkdtemp('/mnt/storage/ui-navigation-')
const login=await fetch(base+'/auth/login',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({username:'nicholas',password:process.env.DEN_SMOKE_PASSWORD})}).then(async r=>{assert(r.ok);return r.json()})
const browser=await chromium.launch({executablePath:'/usr/bin/chromium',headless:true,args:['--no-sandbox']})
const context=await browser.newContext({viewport:{width:1440,height:900},colorScheme:'dark',recordVideo:{dir:scratch,size:{width:1440,height:900}}})
await context.addCookies([{name:'den_session',value:login.token,url:base,httpOnly:true,sameSite:'Lax'}]);await context.addInitScript(csrf=>localStorage.setItem('den.csrf',csrf),login.csrf_token)
const page=await context.newPage(), errors=[];page.on('pageerror',e=>errors.push(e.message));const metrics={}
const go=async path=>{await page.evaluate(async path=>(await import('/src/lib/router.svelte.ts')).router.go(path),path);await page.waitForTimeout(250)}
const shot=async name=>{await page.waitForTimeout(150);await page.screenshot({path:`${shots}${name}-${phase}.png`})}
try {
 await page.goto(base);await page.locator('.composer textarea').waitFor()
 await page.evaluate(async()=>{const {themes}=await import('/src/lib/theme.svelte.ts');await themes.mode('dark');await themes.select(themes.all.find(t=>t.id==='den'),'dark')})
 const channel=await page.evaluate(async()=>(await import('/src/lib/store.svelte.ts')).store.textChannels.find(c=>c.kind==='text').id)
 await go('/c/'+channel)
 await page.getByRole('textbox',{name:'Search this room'}).focus()
 await page.keyboard.press('Control+k');await page.getByRole('dialog',{name:'Go to'}).waitFor()
 await page.keyboard.press('ArrowDown');await shot('palette')
 metrics.modal=await page.getByRole('dialog',{name:'Go to'}).evaluate(el=>el.matches(':modal'))
 metrics.activeOption=await page.locator('.palette input').getAttribute('aria-activedescendant')
 for(let i=0;i<16;i++)await page.keyboard.press('Tab')
 metrics.paletteContainsFocus=await page.getByRole('dialog',{name:'Go to'}).evaluate(el=>el.contains(document.activeElement))
 await page.keyboard.press('Escape');metrics.restored=await page.getByRole('textbox',{name:'Search this room'}).evaluate(el=>el===document.activeElement)
 if(!before){assert(metrics.modal);assert(metrics.activeOption);assert(metrics.paletteContainsFocus);assert(metrics.restored)}
 await go('/settings/account');await page.setViewportSize({width:390,height:844});await page.waitForTimeout(300);await shot('settings-account')
 metrics.activeSettings=await page.locator('.toc a.active').evaluate(el=>{const b=el.getBoundingClientRect();return {visible:b.x>=0&&b.right<=innerWidth,current:el.getAttribute('aria-current')}})
 if(!before){assert(metrics.activeSettings.visible);assert.equal(metrics.activeSettings.current,'page');assert.equal(await page.evaluate(()=>document.documentElement.scrollLeft),0)}
 await page.getByRole('button',{name:'Menu',exact:true}).click();await page.waitForTimeout(200);await shot('drawer')
 metrics.drawerFocus=await page.locator('.sidebar').evaluate(el=>el.contains(document.activeElement))
 for(let i=0;i<24;i++)await page.keyboard.press('Tab')
 metrics.drawerTrapped=await page.locator('.sidebar').evaluate(el=>el.contains(document.activeElement))
 await page.keyboard.press('Escape');metrics.drawerClosed=await page.locator('.sidebar').count()===0
 metrics.menuRestored=await page.getByRole('button',{name:'Menu',exact:true}).evaluate(el=>el===document.activeElement)
 if(!before){assert(metrics.drawerFocus);assert(metrics.drawerTrapped);assert(metrics.drawerClosed);assert(metrics.menuRestored)}
 if(!metrics.drawerClosed)await page.locator('.scrim').click({position:{x:380,y:500}})
 await go('/find?q=evening');await shot('mobile-search');metrics.queryWidth=(await page.getByRole('textbox',{name:'Search messages'}).boundingBox()).width
 if(!before)assert(metrics.queryWidth>240)
 await page.setViewportSize({width:1440,height:900});
 const dm=await page.evaluate(async()=>{const {store}=await import('/src/lib/store.svelte.ts');return (await store.openDm([store.me.id])).id});await go('/c/'+dm);await shot('dm-search')
 metrics.dmSearch=await page.locator('header .search input').getAttribute('placeholder');if(!before)assert(!metrics.dmSearch.includes('#'))
 await go('/c/'+channel)
 await page.evaluate(async()=>{const {store}=await import('/src/lib/store.svelte.ts');store.users=new Map([...store.users, ['review-offline',{...store.me,id:'review-offline',username:'sam',display_name:'Sam',role:'member'}]]);const {call}=await import('/src/lib/call.svelte.ts');call.audioBlocked=true})
 await shot('presence-audio');metrics.audioExplanation=await page.locator('.call-status').innerText();
 if(!before){assert(metrics.audioExplanation.includes('browser blocked'));await page.getByRole('img',{name:'Offline',exact:true}).first().waitFor()}
 await page.emulateMedia({reducedMotion:'reduce'});metrics.iterations=await page.evaluate(()=>{const el=document.createElement('div');el.style.animationIterationCount='infinite';document.body.append(el);const value=getComputedStyle(el).animationIterationCount;el.remove();return value});if(!before)assert.equal(metrics.iterations,'1')
 assert.deepEqual(errors,[]);console.log(JSON.stringify(metrics,null,2));await writeFile(`${shots}verification-${phase}.json`,JSON.stringify(metrics,null,2)+'\n')
} finally {
 await context.close();const video=await page.video().path();await browser.close()
 execFileSync('ffmpeg',['-y','-i',video,'-an','-c:v','libx264','-preset','fast','-crf','25','-pix_fmt','yuv420p',`${shots}keyboard-${phase}.mp4`],{stdio:'ignore'})
}
