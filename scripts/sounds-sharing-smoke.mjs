const password = process.env.DEN_PASSWORD
if (!password) throw Error('Set DEN_PASSWORD to the isolated dev account password')
import { chromium } from 'playwright-core'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { writeFile, mkdtemp, rm } from 'node:fs/promises'
import { execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'
const root=new URL('../',import.meta.url).pathname,out=root+'docs/shots/pr/feat-sounds/'
const scratch=await mkdtemp(join(tmpdir(),'den-sounds-')), bundle=join(scratch,'Andy-bells.den-sounds.zip')
execFileSync('python3',['-c',`import zipfile,json,sys
with zipfile.ZipFile(sys.argv[1],'w') as z:
 z.writestr('pack.json',json.dumps({'id':'andy-bells','name':'Andy’s bells','sounds':{'mention':{'type':'upload','id':'chime'}}}))
 z.write(sys.argv[2],'chime')`,bundle,root+'apps/web/public/sounds/mention.wav'])
const browser=await chromium.launch({executablePath:'/usr/bin/chromium',headless:true,args:['--no-sandbox','--autoplay-policy=no-user-gesture-required']})
const context=await browser.newContext({viewport:{width:1440,height:1100}})
await context.addInitScript(()=>{
 window.__starts=0;window.__active=0;window.__peak=0
 const start=AudioBufferSourceNode.prototype.start,stop=AudioBufferSourceNode.prototype.stop
 AudioBufferSourceNode.prototype.start=function(...args){window.__starts++;window.__active++;window.__peak=Math.max(window.__peak,window.__active);this.__counted=true;this.addEventListener('ended',()=>{if(this.__counted){window.__active--;this.__counted=false}});return start.apply(this,args)}
 AudioBufferSourceNode.prototype.stop=function(...args){if(this.__counted){window.__active--;this.__counted=false}return stop.apply(this,args)}
})
const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));page.on('console',m=>{if(m.type()==='log')console.log(m.text())})
await page.goto('http://localhost:5182');await page.getByLabel('Username',{exact:true}).fill('nicholas');await page.getByLabel('Password',{exact:true}).fill(password);await page.getByRole('button',{name:'Come in'}).click();await page.locator('a[title="Settings"]').waitFor()
const api=async(path,method='GET',body)=>page.evaluate(async({path,method,body})=>{const res=await fetch(path,{method,headers:{'content-type':'application/json','x-csrf-token':localStorage.getItem('den.csrf')},body:body===undefined?undefined:JSON.stringify(body)});if(!res.ok)throw Error(`${res.status} ${await res.text()}`);return res.json()},{path,method,body})
await api('/users/me/sounds','PUT',{overrides:{dm:{type:'silent'}}})
const channel=await api('/channels','POST',{name:'sounds-'+Date.now(),category_id:null,position:100})
await page.goto('http://localhost:5182/c/'+channel.id);await page.locator('textarea').waitFor()
await page.locator('input[type=file]').setInputFiles(bundle)
await page.waitForFunction(()=>document.querySelector('.pending')?.textContent?.includes('100') || !!document.querySelector('.pending .ready'),null,{timeout:1000}).catch(()=>{})
await page.locator('textarea').fill('A partial pack, shared in the usual way.');await page.waitForTimeout(2500);await page.locator('textarea').press('Enter')
const card=page.getByRole('article',{name:'Sound pack',exact:true});await card.getByRole('button',{name:'Add to my sounds'}).waitFor()
await card.getByText('Andy’s bells',{exact:true}).waitFor()
const before=await api('/users/me/sounds')
await card.getByRole('button',{name:'Play',exact:true}).click();await card.getByRole('button',{name:'Play',exact:true}).waitFor();assert.deepEqual(await api('/users/me/sounds'),before)
await card.getByRole('button',{name:'Add to my sounds'}).click();await card.getByRole('status').waitFor();const installed=await api('/users/me/sounds');assert.equal(installed.resolved.dm.sound.type,'silent');assert.equal(installed.resolved.mention.sound.type,'upload')
await page.locator('input[type=file]').setInputFiles(root+'apps/web/public/sounds/upload_complete.wav');await page.waitForTimeout(2500);await page.locator('textarea').fill('A single sound for the designer.');await page.locator('textarea').press('Enter')
const single=page.getByRole('article',{name:'Sound',exact:true});await single.getByRole('button',{name:'Use for…',exact:true}).waitFor();await single.getByRole('combobox').selectOption('upload_complete');await single.getByRole('button',{name:'Use for…',exact:true}).click();await single.getByRole('status').waitFor();assert.equal((await api('/users/me/sounds')).resolved.upload_complete.sound.type,'upload')
await page.screenshot({path:out+'sharing.png'})
await api('/users/me/notification-preferences','PUT',{mentions:true,dms:true,subscribed_channel_ids:[channel.id]})
const login=await fetch('http://127.0.0.1:7018/auth/login',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({username:'av_observer',password:password})}).then(r=>r.json())
async function message(){const r=await fetch(`http://127.0.0.1:7018/channels/${channel.id}/messages`,{method:'POST',headers:{'content-type':'application/json',authorization:`Bearer ${login.token}`},body:JSON.stringify({content:'A followed-room notification '+Date.now()})});assert.equal(r.status,200)}
await page.bringToFront();await page.locator('textarea').click();await page.waitForTimeout(400);const count=await page.evaluate(()=>window.__starts);await message();await page.waitForTimeout(700);assert.equal(await page.evaluate(()=>window.__starts),count,'Focused room must stay quiet')
await page.locator('a[title="Settings"]').click();await page.getByRole('link',{name:'Sounds',exact:true}).click();await page.getByRole('button',{name:'Test all',exact:true}).waitFor();const away=await page.evaluate(()=>window.__starts);await message();await page.waitForFunction(n=>window.__starts>n,away)
await context.emulateMedia?.({reducedMotion:'reduce'})
await page.emulateMedia({reducedMotion:'reduce'})
await page.evaluate(async()=>{
 window.__peak=0
 const path=performance.getEntriesByType('resource').find(e=>new URL(e.name).pathname==='/src/lib/sounds.ts').name
 const {sounds}=await import(path)
 await Promise.all([sounds.url(location.origin,'/sounds/call_join.wav'),sounds.url(location.origin,'/sounds/call_leave.wav'),sounds.url(location.origin,'/sounds/mention.wav')])
})
assert.equal(await page.evaluate(()=>window.__peak),1,'Reduced motion must not overlap')
await page.setViewportSize({width:390,height:844});await page.screenshot({path:out+'mobile.png'});assert(await page.evaluate(()=>{const pane=document.querySelector('.settings');return pane.scrollWidth<=pane.clientWidth && pane.getBoundingClientRect().right<=innerWidth}))
assert.equal(errors.length,0,errors.join('\n'))
await writeFile(out+'sharing-proof.json',JSON.stringify({checks:['normal channel ZIP upload renders named poster card','audition does not change preferences','partial pack preserves silenced DM','single sound installs to selected event','focused followed room silent','away followed room sounds without notification permission','reduced motion peak one sound','390px settings container has no horizontal overflow'],errors},null,2))
await browser.close();await rm(scratch,{recursive:true});console.log('Sharing, notifications and reduced motion passed')
