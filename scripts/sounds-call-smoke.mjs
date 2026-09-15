const password = process.env.DEN_PASSWORD
if (!password) throw Error('Set DEN_PASSWORD to the isolated dev account password')
import { chromium } from 'playwright-core'
import assert from 'node:assert/strict'
import { writeFile } from 'node:fs/promises'
const browser=await chromium.launch({executablePath:'/usr/bin/chromium',headless:true,args:['--no-sandbox','--autoplay-policy=no-user-gesture-required','--use-fake-device-for-media-stream','--use-fake-ui-for-media-stream']})
async function client(username){const context=await browser.newContext();const page=await context.newPage();await page.goto('http://localhost:5182');await page.getByLabel('Username',{exact:true}).fill(username);await page.getByLabel('Password',{exact:true}).fill(password);await page.getByRole('button',{name:'Come in'}).click();await page.locator('a[title="Settings"]').waitFor();await page.evaluate(async()=>{
 const loaded=path=>performance.getEntriesByType('resource').find(e=>new URL(e.name).pathname===path).name
 const {sounds}=await import(loaded('/src/lib/sounds.ts'));window.__call=(await import(loaded('/src/lib/call.svelte.ts'))).call;window.__soundEvents=[]
 const play=sounds.play.bind(sounds);sounds.play=async(event,owner)=>{window.__soundEvents.push(event);return play(event,owner)}
});return page}
const a=await client('nicholas'),b=await client('av_observer')
const channel=await a.evaluate(async()=>{const r=await fetch('/channels',{method:'POST',headers:{'content-type':'application/json','x-csrf-token':localStorage.getItem('den.csrf')},body:JSON.stringify({name:'sounds-call-'+Date.now(),category_id:null,position:101})});return r.json()})
await a.evaluate(async c=>window.__call.join(c),channel);await a.waitForFunction(()=>window.__soundEvents.includes('call_join'))
await b.evaluate(async c=>window.__call.join(c),channel);await a.waitForFunction(()=>window.__soundEvents.includes('someone_joined'))
await b.evaluate(async()=>{const canvas=document.createElement('canvas');canvas.width=320;canvas.height=240;canvas.getContext('2d').fillRect(0,0,320,240);const stream=canvas.captureStream(1);await window.__call.room.localParticipant.publishTrack(stream.getVideoTracks()[0],{source:'screen_share'});window.__screen=stream})
await a.waitForFunction(()=>window.__soundEvents.includes('screen_share_started'))
await b.evaluate(async()=>window.__call.leave());await a.waitForFunction(()=>window.__soundEvents.includes('someone_left'));await a.evaluate(async()=>window.__call.leave())
const eventsA=await a.evaluate(()=>window.__soundEvents),eventsB=await b.evaluate(()=>window.__soundEvents)
for(const e of ['call_join','call_leave','someone_joined','someone_left','screen_share_started'])assert(eventsA.includes(e),e)
assert(eventsB.includes('call_join'));assert(eventsB.includes('call_leave'))
await writeFile(new URL('../docs/shots/pr/feat-sounds/call-proof.json',import.meta.url),JSON.stringify({checks:['two real LiveKit clients in an isolated room','own join/leave','remote join/leave','published screen share'],eventsA,eventsB},null,2));await browser.close();console.log('Live call sound events passed')
