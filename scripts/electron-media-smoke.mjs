// Run against the packaged desktop app, already signed into a private test server.
import{chromium}from 'playwright-core'
import{readFile,writeFile}from'node:fs/promises'
import assert from'node:assert/strict'
const b=await chromium.connectOverCDP(process.env.DEN_ELECTRON_CDP || 'http://127.0.0.1:19226'),p=b.contexts()[0].pages()[0]
const pixels=Array.from(await readFile('apps/desktop/icons/128x128.png'))
const report=await p.evaluate(async ({ pixels, origin })=>{
 const api=async(method,path,body,headers={'content-type':'application/json'})=>{const r=await window.denDesktop.invoke('api_request',{origin,method,path,body:body==null?null:headers['content-type']==='application/json'?Array.from(new TextEncoder().encode(JSON.stringify(body))):body,headers});if(r.status>=300)throw Error(`${method} ${path}: ${r.status}`);return JSON.parse(new TextDecoder().decode(new Uint8Array(r.body)))}
 const channels=await api('GET','/channels'),channel=channels.find(c=>c.kind==='text')
 const u=await api('POST','/uploads',{channel_id:channel.id,filename:'doorway.png',content_type:'image/png',size:pixels.length})
 await api('PATCH',`/uploads/${u.id}`,pixels,{'content-type':'application/octet-stream','upload-offset':'0'})
 await api('POST',`/uploads/${u.id}/complete`)
 const url=`den-media://app/uploads/${u.id}/file?origin=${encodeURIComponent(origin)}`
 const response=await fetch(url,{headers:{range:'bytes=0-15'}})
 const image=new Image();image.src=url;await image.decode()
 return {status:response.status,contentRange:response.headers.get('content-range'),bytes:Array.from(new Uint8Array(await response.arrayBuffer())),width:image.naturalWidth,height:image.naturalHeight}
},{ pixels, origin: process.env.DEN_SMOKE_URL || 'http://127.0.0.1:17010' })
assert.equal(report.status,206);assert.equal(report.bytes.length,16);assert.equal(report.width,128)
await writeFile('docs/shots/pr/desktop-electron/linux-upload.json',JSON.stringify(report,null,2)+'\n')
console.log('PASS native upload, authenticated image decode, 206 range',report.status,report.width,report.contentRange)
await b.close()
