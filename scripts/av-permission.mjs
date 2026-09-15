import { chromium } from 'playwright-core'
import assert from 'node:assert/strict'
const browser=await chromium.launch({executablePath:'/usr/bin/chromium',args:['--no-sandbox','--use-fake-ui-for-media-stream',`--use-fake-device-for-media-stream=fps=${process.env.DEN_SMOKE_FPS || 60}`]})
try {
 const context=await browser.newContext({permissions:['microphone','camera']})
 await context.addInitScript(()=>{
  const get=navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices)
  let rejected=false; window.__tracks=[]
  navigator.mediaDevices.getUserMedia=async options=>{
   if(options.video && !rejected){rejected=true;throw new DOMException('Camera denied for retry test','NotAllowedError')}
   const stream=await get(options);window.__tracks.push(...stream.getTracks());return stream
  }
 })
 const p=await context.newPage();await p.goto(process.env.DEN_SMOKE_URL || 'http://localhost:5178')
 await p.getByLabel('Username',{exact:true}).fill('nicholas');await p.getByLabel('Password',{exact:true}).fill(process.env.DEN_SMOKE_PASSWORD)
 await p.getByRole('button',{name:'Come in',exact:true}).click()
 await p.locator('a[title="Settings"]').click();await p.getByRole('link',{name:'Voice',exact:true}).click()
 await p.getByRole('alert').filter({hasText:'Camera denied for retry test'}).waitFor()
 await p.waitForFunction(()=>!document.querySelector('input[aria-label="Input gain"]')?.disabled)
 assert(await p.getByRole('slider',{name:'Input gain',exact:true}).isEnabled(),'mic works despite camera denial')
 await p.getByRole('button',{name:'Allow microphone and camera',exact:true}).click()
 await p.getByLabel('Camera preview',{exact:true}).waitFor()
 if(process.env.DEN_SMOKE_FPS==='20'){
  await p.waitForFunction(()=>document.querySelector('select[aria-label="Frame rate"]')?.textContent.includes('20 fps'))
  assert(await p.getByRole('combobox',{name:'Frame rate',exact:true}).isDisabled(),'unsupported frame rates are unavailable')
 }
 await p.getByRole('link',{name:'Notifications',exact:true}).click()
 await p.waitForTimeout(500)
 assert(await p.evaluate(()=>window.__tracks.every(t=>t.readyState==='ended')),'closing settings releases owned tracks')
 console.log('Permission retry and owned-track cleanup passed')
} finally {await browser.close()}
