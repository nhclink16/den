// Run with the disposable M9 server on 17900 and Vite on 17901.
import { chromium } from 'playwright-core'
import { readFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const session=JSON.parse(await readFile('/mnt/storage/den-m9-local/session.json','utf8'))
const api=async(method,path,body)=>{
 const r=await fetch('http://127.0.0.1:17900'+path,{method,headers:{authorization:`Bearer ${session.token}`,'content-type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)})
 assert(r.ok,`${method} ${path}: ${r.status}`);return r.status===204?undefined:r.json()
}
const dm=await api('POST','/dms',{member_ids:[session.user.id]})
const object=await api('POST',`/channels/${dm.id}/objects`,{kind:'canvas',name:'M9 embedded theme check',state:{}})
const browser=await chromium.launch({executablePath:'/usr/bin/chromium',args:['--no-sandbox']})
try {
 const page=await browser.newPage({viewport:{width:1200,height:900}})
 await page.route('http://localhost:17901/',route=>route.fulfill({contentType:'text/html',body:'<style>@font-face{font-family:"Den Terminal Mono";src:url(/fonts/ibm-plex-mono-latin-400-normal.woff2)}body{margin:0;background:var(--bg);color:var(--ink)}#terminal{width:100%;height:280px}#canvas{width:100%;height:560px;position:relative}</style><div id="terminal"></div><div id="canvas" class="canvas-host"></div>'}))
 await page.route('**/objects/**',async route=>{
  const response=await route.fetch({url:'http://127.0.0.1:17900'+new URL(route.request().url()).pathname,headers:{authorization:`Bearer ${session.token}`}})
  await route.fulfill({response})
 })
 await page.goto('http://localhost:17901/')
 const errors=[];page.on('pageerror',e=>errors.push(e.message))
 await page.evaluate(async({object,user})=>{
  const runtime=await import('/src/lib/theme-runtime.ts');window.themeRuntime=runtime
  runtime.applyTheme(runtime.builtinThemes.find(t=>t.id==='tide'))
  const {store}=await import('/src/lib/store.svelte.ts');store.me=user;store.users=new Map([[user.id,user]])
  const terminal=await import('/src/plugins/terminal/renderer.ts')
  window.terminal=await terminal.mount(document.querySelector('#terminal'),80,16,()=>{})
  window.terminal.term.write('M9 live terminal theme\r\nThe same session follows the room colors.')
  const canvas=await import('/src/plugins/canvas/editor.ts')
  window.closeCanvas=await canvas.mountCanvas(document.querySelector('#canvas'),object,true,()=>{})
 },{object,user:session.user})
 await page.waitForFunction(()=>document.querySelector('#canvas').denEditor)
 for(const [id,bg,dark] of [['paper','#fafaf7',false],['terminal','#000000',true]]) {
  await page.evaluate(id=>window.themeRuntime.applyTheme(window.themeRuntime.builtinThemes.find(t=>t.id===id)),id)
  await page.waitForFunction(({bg,dark})=>window.terminal.term.options.theme.background===bg&&document.querySelector('#canvas').denEditor.user.getIsDarkMode()===dark,{bg,dark})
  assert.equal(await page.evaluate(()=>window.terminal.term.options.theme.foreground),id==='paper'?'#1f1f1d':'#d0d0d0')
  await page.waitForFunction(bg => { const c=document.querySelector('#terminal canvas');const p=c.getContext('2d').getImageData(10,80,1,1).data;return [...p].slice(0,3).map(v=>v.toString(16).padStart(2,'0')).join('')===bg.slice(1) }, bg)
  await page.screenshot({path:`/mnt/storage/den-m9-embedded-${id}.png`})
 }
 assert.deepEqual(errors,[])
 await page.evaluate(()=>{window.closeCanvas();window.terminal.destroy()})
 console.log('M9 embedded clients passed: mounted Ghostty terminal and tldraw canvas follow Paper and Terminal without remounting.')
} finally {await browser.close();await api('DELETE',`/messages/${object.message_id}`)}
