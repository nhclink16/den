// Exercise the real popTile function in a browser without requiring a LiveKit call.
import { chromium } from 'playwright-core'
import ts from '../apps/web/node_modules/typescript/lib/typescript.js'
import { readFile } from 'node:fs/promises'
import assert from 'node:assert/strict'
const source = await readFile(new URL('../apps/web/src/lib/call-popout.ts', import.meta.url),'utf8')
const code=ts.transpileModule(source.replace('export async function','async function'),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText
const browser=await chromium.launch({executablePath:'/usr/bin/chromium',args:['--no-sandbox']})
try {
 const context=await browser.newContext(),page=await context.newPage()
 await page.setContent('<style>body{background:var(--bg);color:var(--ink)}</style><div id="home"><div id="content">Live tile</div></div>')
 await page.addScriptTag({content:code})
 await page.evaluate(()=>{document.documentElement.style.cssText='--bg:#0f1518;--bg-2:#151d21;--ink:#dfe8ec;--accent:#5fd3c6;color-scheme:dark';document.documentElement.dataset.theme='tide'})
 const opened=context.waitForEvent('page')
 await page.evaluate(()=>window.popTile(document.querySelector('#content'),document.querySelector('#home'),()=>{window.returned=true}))
 const popup=await opened;await popup.waitForLoadState()
 assert.equal(await popup.evaluate(()=>document.documentElement.style.getPropertyValue('--accent')),'#5fd3c6')
 await page.evaluate(()=>{document.documentElement.style.setProperty('--accent','#2f6fed');document.documentElement.style.colorScheme='light';document.documentElement.dataset.theme='paper'})
 await popup.waitForFunction(()=>document.documentElement.dataset.theme==='paper')
 assert.equal(await popup.evaluate(()=>document.documentElement.style.getPropertyValue('--accent')),'#2f6fed')
 assert.equal(await popup.evaluate(()=>document.documentElement.style.colorScheme),'light')
 await popup.getByRole('button',{name:'Bring back'}).click()
 await page.waitForFunction(()=>window.returned===true)
 assert.equal(await page.locator('#home > #content').innerText(),'Live tile')
 console.log('M9 pop-out passed: initial tokens, live theme and color-scheme changes, original content restored.')
} finally {await browser.close()}
