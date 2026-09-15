// Run against an isolated Vite/server. BEFORE=1 captures the original appearance UI.
import {chromium} from 'playwright-core'
import {mkdir,writeFile} from 'node:fs/promises'
import assert from 'node:assert/strict'
const base=process.env.DEN_SMOKE_URL||'http://localhost:5183', before=process.env.BEFORE==='1', phase=before?'before':'after'
const shots=new URL('../docs/shots/pr/fix/ui-appearance/',import.meta.url).pathname;await mkdir(shots,{recursive:true})
const login=await fetch(base+'/auth/login',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({username:'nicholas',password:process.env.DEN_SMOKE_PASSWORD})}).then(async r=>{assert(r.ok);return r.json()})
const browser=await chromium.launch({executablePath:'/usr/bin/chromium',headless:true,args:['--no-sandbox']})
const context=await browser.newContext({viewport:{width:1440,height:1000},colorScheme:'light'});await context.addCookies([{name:'den_session',value:login.token,url:base,httpOnly:true,sameSite:'Lax'}]);await context.addInitScript(csrf=>localStorage.setItem('den.csrf',csrf),login.csrf_token)
const page=await context.newPage(),metrics={},errors=[];page.on('pageerror',e=>errors.push(e.message))
const go=async path=>{await page.evaluate(async path=>(await import('/src/lib/router.svelte.ts')).router.go(path),path);await page.waitForTimeout(200)}
const shot=async name=>{await page.waitForTimeout(200);await page.screenshot({path:`${shots}${name}-${phase}.png`})}
try {
 await page.goto(base+'/settings/appearance');await page.getByRole('heading',{name:'Appearance',exact:true}).waitFor()
 await page.evaluate(async()=>{const {themes}=await import('/src/lib/theme.svelte.ts');await themes.select(themes.all.find(t=>t.id==='den'),'light');await themes.select(themes.all.find(t=>t.id==='terminal'),'dark');await themes.mode('light')})
 await page.waitForTimeout(1000);await shot('appearance-desktop')
 metrics.appearanceHeading=await page.locator('.intro h2').evaluate(el=>getComputedStyle(el).fontSize)
 metrics.badge=await page.getByRole('button',{name:'Use Terminal for dark appearance',exact:true}).locator('.badge').evaluate(el=>({ink:getComputedStyle(el).color,background:getComputedStyle(el).backgroundColor}))
 metrics.previewFonts=await page.getByRole('button',{name:'Use Terminal for dark appearance',exact:true}).locator('.preview').evaluate(el=>({body:getComputedStyle(el).fontFamily,mono:getComputedStyle(el.querySelector('.composer')).fontFamily}))
 if(!before){assert(metrics.previewFonts.body.includes('JetBrains Mono'));assert(metrics.previewFonts.mono.includes('JetBrains Mono'));assert.equal(metrics.badge.ink,'rgb(0, 0, 0)')}
 await page.setViewportSize({width:390,height:844});await page.locator('.pane').evaluate(el=>el.scrollTop=0);await shot('appearance-mobile')
 metrics.themesTop=(await page.getByRole('heading',{name:'Themes',exact:true}).boundingBox()).y
 if(!before)assert(metrics.themesTop<550)
 await page.getByRole('button',{name:'Create theme',exact:true}).click();await page.getByRole('region',{name:'Theme editor'}).scrollIntoViewIfNeeded();await shot('editor-mobile')
 if(!before)await page.getByRole('textbox',{name:'Page background hex',exact:true}).waitFor()
 await page.setViewportSize({width:1440,height:1000});await page.getByRole('region',{name:'Theme editor'}).scrollIntoViewIfNeeded();await shot('editor-desktop')
 await go('/settings/machines');metrics.machineHeading=await page.locator('.pane h2').evaluate(el=>getComputedStyle(el).fontSize);await shot('machines')
 await go('/settings/rooms');await shot('rooms');metrics.formHeights=await page.locator('.server-name .inline').evaluate(el=>Array.from(el.children).map(c=>c.getBoundingClientRect().height))
 if(!before){assert.equal(metrics.appearanceHeading,'26px');assert.equal(metrics.machineHeading,'26px');assert(Math.abs(metrics.formHeights[0]-metrics.formHeights[1])<1)}
 metrics.contrast=await page.evaluate(async()=>{const r=await import('/src/lib/theme-runtime.ts');return r.builtinThemes.flatMap(t=>['light','dark'].map(half=>{r.applyTheme(t,half);const line=document.documentElement.style.getPropertyValue('--line-strong')||t[half].line;return {theme:t.id,half,minimum:Math.min(...['bg','bg2','bg3'].map(k=>r.contrast(line,t[half][k])))}}))})
 if(!before)assert(metrics.contrast.every(c=>c.minimum>=3))
 assert.deepEqual(errors,[]);console.log(JSON.stringify(metrics,null,2));await writeFile(`${shots}verification-${phase}.json`,JSON.stringify(metrics,null,2)+'\n')
}finally{await browser.close()}
