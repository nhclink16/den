import { readFileSync } from 'node:fs'
const themes = JSON.parse(readFileSync(new URL('../crates/den-core/src/themes.json', import.meta.url)))
const luminance = hex => hex.slice(1).match(/../g).map(v => parseInt(v,16)/255).map(v => v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4).reduce((s,v,i)=>s+v*[0.2126,0.7152,0.0722][i],0)
for (const t of themes) for (const [fg,bg] of [['ink','bg'],['ink2','bg2']]) {
 const a=luminance(t.colors[fg]), b=luminance(t.colors[bg]); const ratio=(Math.max(a,b)+0.05)/(Math.min(a,b)+0.05)
 console.log(`${t.name} ${fg}/${bg}: ${ratio.toFixed(2)}:1`)
 if (ratio<4.5) throw Error(`${t.name} ${fg}/${bg} fails 4.5:1`)
}
