// DEN_SMOKE_URL=https://denchat.app DEN_SMOKE_CREDENTIALS=/private/credentials.json node scripts/m9-smoke.mjs
import { chromium } from 'playwright-core'
import { readFile, mkdir, writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const base = process.env.DEN_SMOKE_URL || 'http://127.0.0.1:17900'
const credentials = JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || '/mnt/storage/den-m9-local/credentials.json', 'utf8'))
const username = process.env.DEN_SMOKE_USER || credentials.username || 'nicholas'
const password = credentials.users?.[username] || credentials.password
const shots = process.env.DEN_SMOKE_SHOTS || new URL('../docs/shots/', import.meta.url).pathname
await mkdir(shots, { recursive: true })
let token
async function api(method, path, body) {
  const r = await fetch(base + path, { method, headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) }, body: body === undefined ? undefined : JSON.stringify(body) })
  assert(r.ok, `${method} ${path}: ${r.status}`); return r.status === 204 ? undefined : r.json()
}
const session = await api('POST','/auth/login',{ username,password }); token=session.token
const original = await api('GET','/users/me/appearance')
const browser=await chromium.launch({ executablePath: '/usr/bin/chromium',args:['--no-sandbox'] })
const errors=[]
async function context() {
 const c=await browser.newContext({viewport:{width:1440,height:900},colorScheme:'dark'})
 // Each context gets its own server login, no shared storage cache.
 const s=await api('POST','/auth/login',{username,password})
 await c.addCookies([{name:'den_session',value:s.token,url:base,httpOnly:true,sameSite:'Strict'}])
 await c.addInitScript(csrf=>localStorage.setItem('den.csrf',csrf),s.csrf_token)
 return c
}
const accent=p=>p.evaluate(()=>document.documentElement.style.getPropertyValue('--accent'))
const until=async fn=>{for(let n=0;n<120;n++){if(await fn()) return;await new Promise(r=>setTimeout(r,100))}throw Error('Timed out')}
async function selected(p,name) {
 await p.getByRole('button',{name:`Use ${name} theme`,exact:true}).click()
 await until(async()=>await p.getByRole('button',{name:`Use ${name} theme`,exact:true}).getAttribute('aria-pressed')==='true')
 await until(async()=>!(await p.getByRole('status').allTextContents()).includes('Saving…'))
}
try {
 await api('PUT','/users/me/appearance',{mode:'dark',light_theme:'den-light',dark_theme:'den',custom_themes:[]})
 const c=await context(),page=await c.newPage(); page.on('pageerror',e=>errors.push(e.message))
 await page.goto(base+'/settings/appearance'); await page.getByRole('heading',{name:'Appearance',exact:true}).waitFor()
 assert.equal(await page.locator('.theme-card').count(),9)
 await selected(page,'Tide'); assert.equal(await accent(page),'#5fd3c6')
 await until(async()=>(await api('GET','/users/me/appearance')).dark_theme==='tide')
 // Hold app modules so this screenshot proves the inline cache works before hydration.
 let releaseModules
 const modulesHeld = new Promise(resolve => { releaseModules = resolve })
 await page.route('**/assets/*.js',async route=>{await modulesHeld;await route.continue()})
 await page.reload({waitUntil:'commit'});await page.waitForTimeout(50)
 assert.equal(await accent(page),'#5fd3c6')
 assert.equal(await page.locator('#app > *').count(),0)
 assert.equal(await page.evaluate(()=>getComputedStyle(document.documentElement).backgroundColor),'rgb(15, 21, 24)')
 const capture = await c.newCDPSession(page)
 const firstPaint = await capture.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: false })
 await writeFile(`${shots}/m9-first-paint-50ms.png`, Buffer.from(firstPaint.data, 'base64'))
 await capture.detach()
 releaseModules()
 await page.unrouteAll({ behavior: 'wait' });await page.getByRole('heading',{name:'Appearance',exact:true}).waitFor()
 await page.getByRole('button',{name:'System',exact:true}).click()
 await page.emulateMedia({colorScheme:'light'});await until(async()=>await accent(page)==='#c77d1f')
 await page.emulateMedia({colorScheme:'dark'});await until(async()=>await accent(page)==='#5fd3c6')
 assert.equal(await page.locator('.theme-card[aria-pressed=true]').count(),2)
 await page.getByRole('button',{name:'Customize',exact:true}).click()
 await page.getByLabel('accent hex',{exact:true}).fill('#78dcca');assert.equal(await accent(page),'#78dcca')
 await page.getByRole('button',{name:'Round',exact:true}).click();await page.getByRole('button',{name:'Compact',exact:true}).click()
 assert.equal(await page.evaluate(()=>document.documentElement.style.getPropertyValue('--r')),'10px')
 assert.equal(await page.evaluate(()=>document.documentElement.style.getPropertyValue('--density')),'0.8')
 await page.getByLabel('body font',{exact:true}).selectOption('Atkinson Hyperlegible')
 assert((await page.evaluate(()=>document.documentElement.style.getPropertyValue('--body'))).includes('Atkinson Hyperlegible'))
 await page.locator('.pane').evaluate(el=>el.scrollTop=0)
 await page.screenshot({path:`${shots}/m9-editor-desktop.png`})
 await page.getByRole('button',{name:'Save as new theme',exact:true}).click()
 await page.getByLabel('Theme name',{exact:true}).fill('M9 sea glass')
 await page.getByRole('button',{name:'Save theme',exact:true}).click()
 await until(async()=>(await api('GET','/users/me/appearance')).custom_themes.length===1)
 const c2=await context(),second=await c2.newPage();await second.goto(base+'/settings/appearance')
 await second.getByRole('button',{name:'Use M9 sea glass theme',exact:true}).waitFor();assert.equal(await accent(second),'#78dcca')
 await second.reload();await second.getByRole('button',{name:'Use M9 sea glass theme',exact:true}).waitFor();assert.equal(await accent(second),'#78dcca')
 const download=page.waitForEvent('download');await page.getByRole('button',{name:'Export',exact:true}).click()
 const file=await download;assert.equal(file.suggestedFilename(),'M9 sea glass.den-theme.json')
 const exported=JSON.parse(await readFile(await file.path(),'utf8'))
 await page.getByLabel('Import theme file',{exact:true}).setInputFiles({name:file.suggestedFilename(),mimeType:'application/json',buffer:Buffer.from(JSON.stringify(exported))})
 await until(async()=>(await api('GET','/users/me/appearance')).custom_themes.length===2)
 const imported=(await api('GET','/users/me/appearance')).custom_themes.at(-1)
 assert.deepEqual({...imported,id:exported.id},exported)
 await until(async()=>await second.getByRole('button',{name:'Use M9 sea glass theme',exact:true}).count()===2)
 await page.getByLabel('Import theme file',{exact:true}).setInputFiles({name:'bad.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify({...exported,colors:{...exported.colors,accent:'red'}}))})
 await until(async()=>(await page.getByRole('alert').allTextContents()).some(t=>t.includes('Invalid theme')))
 assert.equal((await api('GET','/users/me/appearance')).custom_themes.length,2)
 await page.getByRole('button',{name:'Reset',exact:true}).click()
 await page.getByRole('button',{name:'Dark',exact:true}).click();await selected(page,'Den')
 await until(async()=>await accent(second)==='#e8a44a') // live session event
 await page.getByRole('button',{name:'Customize',exact:true}).click() // close editor
 await page.locator('.pane').evaluate(el=>el.scrollTop=0)
 await page.screenshot({path:`${shots}/m9-appearance-desktop.png`})
 await page.setViewportSize({width:390,height:844})
 assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth))
 assert(await page.locator('.pane').evaluate(el=>el.scrollWidth<=el.clientWidth))
 await page.screenshot({path:`${shots}/m9-appearance-mobile.png`})
 await page.getByRole('heading',{name:'Fonts',exact:true}).scrollIntoViewIfNeeded()
 await page.screenshot({path:`${shots}/m9-appearance-mobile-controls.png`})
 await page.locator('.pane').evaluate(el=>el.scrollTop=0)
 await page.setViewportSize({width:1440,height:900})
 // A self-DM keeps screenshot text separate from friends' conversations.
 const dm=await api('POST','/dms',{member_ids:[session.user.id]})
 const message=await api('POST',`/channels/${dm.id}/messages`,{content:'M9 theme check\n\nEvening, everyone. The room is ready.\n\n`den health` • Text, calls, and a place to catch up.'})
 try {
  for(const [id,name] of [['den','Den'],['paper','Paper'],['tide','Tide'],['terminal','Terminal']]) {
   await selected(page,name);await page.goto(base+`/c/${dm.id}`);await page.getByText('Evening, everyone. The room is ready.',{exact:false}).waitFor()
   await page.waitForTimeout(400);await page.screenshot({path:`${shots}/m9-chat-${id}.png`})
   await page.goto(base+'/settings/appearance');await page.getByRole('heading',{name:'Appearance',exact:true}).waitFor()
  }
 } finally { await api('DELETE',`/messages/${message.id}`) }
 // Invalid Google family gets a bounded, visible failure and real fallback.
 await page.getByLabel('Use any Google Font for body',{exact:true}).fill('Den No Such Font XYZ')
 await page.getByLabel('Use any Google Font for body',{exact:true}).press('Tab')
 await until(async()=>(await page.getByRole('alert').allTextContents()).includes("Couldn't load that font"))
 assert.equal(await page.evaluate(()=>document.documentElement.style.getPropertyValue('--body')),'system-ui, sans-serif')
 await page.getByRole('button',{name:'Reset',exact:true}).click()
 assert.deepEqual(errors,[])
 console.log('M9 passed: nine palettes, instant Tide, cached first paint at 50 ms, system light/dark, live fonts/radius/density, custom theme in an independent reloaded session, JSON round trip, invalid import, private live sync, font failure, desktop/mobile screenshots.')
} finally { await api('PUT','/users/me/appearance',original);await browser.close() }
