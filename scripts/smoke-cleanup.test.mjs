// Integration check against a disposable server, including failure and KEEP paths.
import { SmokeCleanup, keepSmoke } from './smoke-cleanup.mjs'
import { readFile } from 'node:fs/promises'
import { chromium } from 'playwright-core'
import assert from 'node:assert/strict'
const base = process.env.DEN_SMOKE_URL
assert(base && ['127.0.0.1','localhost'].includes(new URL(base).hostname), 'Use a disposable local server')
const session = JSON.parse(await readFile(process.env.DEN_SMOKE_SESSION, 'utf8'))
const plain = new SmokeCleanup(base, session.token)
const room = (await plain.api('GET','/channels')).find(c=>c.name==='general')
const path = `/channels/${room.id}/messages`
const original = await plain.api('POST', path, {content:'Existing message: cleanup must preserve me'})
let cleanup, browser
try {
  cleanup = await SmokeCleanup.start(base, session.token)
  browser = await chromium.launch({executablePath:'/usr/bin/chromium',args:['--no-sandbox']})
  const context = await browser.newContext(); cleanup.watch(context)
  const page = await context.newPage()
  await page.route(base+'/',route=>route.fulfill({contentType:'text/html',body:'Cleanup failure fixture'}))
  await page.goto(base)
  try {
    await page.evaluate(async ({path,token}) => {
      const r = await fetch(path,{method:'POST',headers:{authorization:`Bearer ${token}`,'content-type':'application/json'},body:JSON.stringify({content:'Browser fixture before intentional failure'})})
      if (!r.ok) throw Error('fixture creation failed')
      await r.json()
    },{path,token:session.token})
    // Node and browser creation receipts must both survive the thrown assertion.
    for (const [path,body] of [[`/channels/${room.id}/objects`,{kind:'canvas',name:'smoke',state:{}}],['/uploads',{channel_id:room.id,filename:'unfinished.bin',content_type:'application/octet-stream',size:10}]]) {
      const r = await fetch(base+path,{method:'POST',headers:{authorization:`Bearer ${session.token}`,'content-type':'application/json'},body:JSON.stringify(body)})
      assert(r.ok)
    }
    throw Error('intentional smoke failure')
  } catch(e) { assert.equal(e.message,'intentional smoke failure') }
  finally { await cleanup.finish(browser) }
  assert(await plain.api('GET',`/messages/${original.id}`))
  assert.equal(cleanup.messages.size,2)
  for (const id of cleanup.messages) assert.equal(!!await plain.api('GET',`/messages/${id}`,undefined,true),keepSmoke)
  for (const id of cleanup.uploads) assert.equal(!!await plain.api('GET',`/uploads/${id}`,undefined,true),keepSmoke)
  console.log(`PASS cleanup on failure; existing message preserved; KEEP=${keepSmoke ? 1 : 0} honored.`)
} finally {
  await browser?.close()
  // Remove the retained test of KEEP itself so this check is repeatable.
  for (const id of cleanup?.messages || []) await plain.api('DELETE',`/messages/${id}`,undefined,true)
  for (const id of cleanup?.uploads || []) await plain.api('DELETE',`/uploads/${id}`,undefined,true)
  await plain.api('DELETE',`/messages/${original.id}`,undefined,true)
}
