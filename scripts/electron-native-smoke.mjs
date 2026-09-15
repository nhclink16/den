import { chromium } from 'playwright-core'
import { readFile, writeFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const credentials=JSON.parse(await readFile(process.env.DEN_SMOKE_CREDENTIALS || '/mnt/storage/den-electron-acceptance/credentials.json','utf8'))
const platform=process.env.DEN_SMOKE_PLATFORM || 'linux'
const browser=await chromium.connectOverCDP(process.env.DEN_ELECTRON_CDP || 'http://127.0.0.1:19226')
const page=browser.contexts()[0].pages()[0],errors=[],sockets=[]
page.setDefaultTimeout(15000)
page.on('pageerror',e=>errors.push(e.message));page.on('websocket',s=>sockets.push(s))
const bases=['http://127.0.0.1:17010','http://127.0.0.1:17011']
const until=async fn=>{for(let n=0;n<150;n++){if(await fn())return;await page.waitForTimeout(200)}throw Error('Timed out')}
const api=async(i,method,path,body,token=credentials.users[i].admin.token)=>{const r=await fetch(bases[i]+path,{method,headers:{'content-type':'application/json',authorization:`Bearer ${token}`},body:body===undefined?undefined:JSON.stringify(body)});assert(r.ok,`${method} ${path}: ${r.status}`);return r.status===204?null:r.json()}
try {
  if(await page.getByRole('button',{name:'Come in',exact:true}).count()) {
    await page.evaluate(origin=>localStorage.setItem('den.native.origin',origin),bases[0]);await page.reload()
    await page.getByLabel('Username',{exact:true}).fill('electron_test');await page.getByLabel('Password',{exact:true}).fill(credentials.password);await page.getByRole('button',{name:'Come in',exact:true}).click()
  }
  await page.getByRole('button',{name:'Switch server'}).waitFor({timeout:30000})
  const remembered=await page.evaluate(()=>window.denDesktop.invoke('instances_get'))
  if(!remembered.includes(bases[1])) {
    await page.getByRole('button',{name:'Switch server'}).click();await page.getByRole('button',{name:'Add a server'}).click();await page.getByLabel('Server URL').fill(bases[1]);await page.getByRole('button',{name:'Continue',exact:true}).click()
    await page.getByRole('heading',{name:'Second Den',exact:true}).waitFor();await page.getByLabel('Username',{exact:true}).fill('electron_test');await page.getByLabel('Password',{exact:true}).fill(credentials.password);await page.getByRole('button',{name:'Log in',exact:true}).click();await until(async()=>/Second Den/.test(await page.getByRole('button',{name:'Switch server'}).innerText()))
  }
  await page.keyboard.press('Control+Shift+Digit1');await until(async()=>/Electron lab/.test(await page.getByRole('button',{name:'Switch server'}).innerText()))
  await page.locator('nav.side a').filter({hasText:'general'}).click()
  await page.setViewportSize({width:1200,height:800})
  if(platform!=='linux') await page.screenshot({path:`docs/shots/pr/desktop-electron/${platform}-after.png`})
  await page.keyboard.press('Control+Shift+Digit2');await until(async()=>/Second Den/.test(await page.getByRole('button',{name:'Switch server'}).innerText()))
  await page.getByRole('link',{name:/^Inbox/}).click()
  const messages=[]
  for(let i=0;i<2;i++) {const channels=await api(i,'GET','/channels'),channel=channels.find(c=>c.kind==='text');messages.push([i,await api(i,'POST',`/channels/${channel.id}/messages`,{content:`@electron_test Native ${platform} acceptance ${Date.now()}`},credentials.users[i].peer.token)])}
  await until(async()=>{const text=await page.locator('section.inbox').innerText();return /Electron lab/i.test(text)&&/Second Den/i.test(text)})
  await page.screenshot({path:`docs/shots/pr/desktop-electron/${platform}-inbox.png`})
  await page.getByRole('button',{name:'Switch server'}).click();await page.screenshot({path:`docs/shots/pr/desktop-electron/${platform}-switcher.png`});await page.getByRole('button',{name:'Close server switcher'}).click()
  await page.keyboard.press('Control+k');await page.getByPlaceholder('Jump to a room, a person, or an action').fill('general');assert.equal(await page.locator('.palette .hint').filter({hasText:/Electron lab|Second Den/}).count(),2);await page.keyboard.press('Escape')
  await page.reload();await page.getByRole('button',{name:'Switch server'}).waitFor();await until(()=>bases.every(origin=>sockets.some(s=>s.url().startsWith(origin.replace('http','ws')+'/ws?ticket='))))
  const storage=await page.evaluate(()=>JSON.stringify(localStorage));for(const user of credentials.users) assert(!storage.includes(user.admin.token))
  const scope=await page.evaluate(async()=>({unknown:await window.denDesktop.invoke('not_a_command').then(()=>false,()=>true),escape:await window.denDesktop.invoke('api_request',{origin:'http://127.0.0.1:17010',method:'GET',path:'//evil.test/'}).then(()=>false,()=>true),node:typeof window.require}))
  assert.deepEqual(scope,{unknown:true,escape:true,node:'undefined'})
  assert.deepEqual(errors,[])
  console.log(`PASS ${platform}: real native login, encrypted sessions, two ticket sockets, switching, merged inbox, cross-server palette, reload restoration, denied commands/origin escapes`)
  await writeFile(`docs/shots/pr/desktop-electron/${platform}-native.json`,JSON.stringify({scope,errors,ticketSockets:bases.length,platform},null,2)+'\n')
  // Keep the private test messages for Nicholas to inspect in the installed morning build.
} finally {await browser.close()}
