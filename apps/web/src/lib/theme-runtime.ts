import type { Appearance, Theme, ThemeColors } from './types'
import builtins from '../../../../crates/den-core/src/themes.json'

export const builtinThemes = builtins as Theme[]
export const defaultAppearance: Appearance = { mode: 'system', theme: 'den', custom_themes: [] }
export function appearanceCacheKey(origin?: string) { return (window.__TAURI__ ? `den.appearance:${origin || localStorage.getItem('den.native.origin') || 'https://denchat.app'}` : 'den.appearance') }
export const colorRoles = ['bg','bg2','bg3','line','ink','ink2','ink3','accent','success','danger'] as const
// Same OKLab matrices and quantization as den-core/theme_color.rs. Keeping a/b
// fixed while replacing L preserves OKLCH hue/chroma before sRGB gamut clipping.
export function linear(hex: string): number[] { return hex.slice(1).match(/../g)!.map(v => parseInt(v,16)/255).map(v => v <= .04045 ? v/12.92 : ((v+.055)/1.055)**2.4) }
export function contrast(a: string,b: string) {
  const lum = (v: string) => linear(v).reduce((s,c,i) => s+c*[.2126,.7152,.0722][i]!,0)
  const x=lum(a),y=lum(b); return (Math.max(x,y)+.05)/(Math.min(x,y)+.05)
}
export function lab(hex: string): number[] {
  const [r,g,b] = linear(hex) as [number,number,number]
  const l=Math.cbrt(.4122214708*r+.5363325363*g+.0514459929*b), m=Math.cbrt(.2119034982*r+.6806995451*g+.1073969566*b), s=Math.cbrt(.0883024619*r+.2817188376*g+.6299787005*b)
  return [.2104542553*l+.793617785*m-.0040720468*s,1.9779984951*l-2.428592205*m+.4505937099*s,.0259040371*l+.7827717662*m-.808675766*s]
}
export function withLightness(color: string, lightness: number) {
  const [,a,b] = lab(color) as [number,number,number], L=Math.max(0,Math.min(1,lightness))
  const l=(L+.3963377774*a+.2158037573*b)**3, m=(L-.1055613458*a-.0638541728*b)**3, s=(L-.0894841775*a-1.291485548*b)**3
  return '#'+[4.0767416621*l-3.3077115913*m+.2309699292*s,-1.2684380046*l+2.6097574011*m-.3413193965*s,-.0041960863*l-.7034186147*m+1.707614701*s].map(v => {
    v=Math.max(0,Math.min(1,v)); return Math.round((v<=.0031308?12.92*v:1.055*v**(1/2.4)-.055)*255).toString(16).padStart(2,'0')
  }).join('')
}
export function deriveHalf(source: ThemeColors): ThemeColors {
  const out={...source,generated:true}
  for (const [bg,ink] of [['bg','ink'],['bg2','ink2'],['bg3','ink3']] as const) {
    out[bg]=withLightness(source[bg],lab(source[ink])[0]!); out[ink]=withLightness(source[ink],lab(source[bg])[0]!)
  }
  const direction=lab(out.bg)[0]!>.5?-1:1
  out.line=withLightness(source.line,lab(out.bg)[0]!+direction*.12)
  for (const role of ['accent','success','danger'] as const) {
    const start=lab(source[role])[0]!
    for (let step=0;step<=1000;step++) { out[role]=withLightness(source[role],start+direction*step/1000); if(contrast(out[role],out.bg)>=4.5) break }
    if(contrast(out[role],out.bg)<4.5) out[role]=contrast('#000000',out.bg)>=4.5?'#000000':'#ffffff'
  }
  return out
}
export function activeTheme(a: Appearance): Theme { return [...builtinThemes,...a.custom_themes].find(t=>t.id===a.theme) || builtinThemes[0]! }
export function appearanceHalf(a: Appearance, systemLight = matchMedia('(prefers-color-scheme: light)').matches): 'light'|'dark' { return a.mode === 'light' || (a.mode === 'system' && systemLight) ? 'light' : 'dark' }
export function applyTheme(t: Theme, half: 'light'|'dark' = 'dark') {
  const root = document.documentElement, s = root.style, colors=t[half]
  for (const key of colorRoles) s.setProperty(`--${key}`, colors[key])
  s.setProperty('--accent-dim', `color-mix(in srgb, ${colors.accent} 55%, ${colors.bg})`)
  s.setProperty('--accent-glow', `color-mix(in srgb, ${colors.accent} 18%, transparent)`)
  s.setProperty('--selection', 'var(--accent-dim)'); s.setProperty('--mention-bg', 'var(--accent-glow)')
  for (const [key,value] of Object.entries(t.fonts)) s.setProperty(`--${key}`, `"${value}", ${key === 'mono' ? '"Den Terminal Mono", monospace' : 'system-ui, sans-serif'}`)
  const radii = { sharp: [2,6], soft: [6,12], round: [10,18] }[t.radius]
  s.setProperty('--r', `${radii[0]}px`); s.setProperty('--r-lg', `${radii[1]}px`)
  s.setProperty('--density', t.density === 'compact' ? '0.8' : '1')
  s.backgroundColor = colors.bg; s.color = colors.ink; s.colorScheme = half; root.dataset.theme = t.id
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', colors.bg)
}
export function cachedAppearance(origin?: string): Appearance {
  try {
    const a = JSON.parse(localStorage.getItem(appearanceCacheKey(origin)) || 'null')
    if (a && ['light','dark','system'].includes(a.mode) && Array.isArray(a.custom_themes)) {
      if (!a.theme) {
        a.theme=a.dark_theme || a.light_theme || 'den'; if(a.theme==='den-light') a.theme='den'
        a.custom_themes=a.custom_themes.map((t: Theme & { appearance?: 'light'|'dark'; colors?: ThemeColors }) => {
          if(!t.colors || !t.appearance) return t
          const {colors,appearance,...rest}=t
          return {...rest,[appearance]:colors,[appearance==='dark'?'light':'dark']:deriveHalf(colors)}
        })
        delete a.dark_theme; delete a.light_theme
      }
      return a
    }
  } catch { /* unavailable or malformed cache */ }
  return structuredClone(defaultAppearance)
}
export function firstPaint() {
  try { const a=cachedAppearance(); applyTheme(activeTheme(a),appearanceHalf(a)) } catch { applyTheme(builtinThemes[0]!) }
}
