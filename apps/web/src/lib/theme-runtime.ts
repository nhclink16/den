import { isDesktop } from './desktop'
import { mediaUrl } from './native'
import { wallpaperImage } from './wallpapers'
import type { Appearance, AppearanceBackground, Theme, ThemeColors } from './types'
import builtins from '../../../../crates/den-core/src/themes.json'

export const builtinThemes = builtins as Theme[]
export const defaultAppearance: Appearance = { mode: 'system', light_theme: 'den', dark_theme: 'den', custom_themes: [], contrast: 100 }
export function appearanceCacheKey(origin?: string) { return (isDesktop() ? `den.appearance:${origin || localStorage.getItem('den.native.origin') || 'https://denchat.app'}` : 'den.appearance') }
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
/** The theme chosen for one appearance. Light and dark are picked independently. */
export function themeFor(a: Appearance, half: 'light'|'dark'): Theme {
  const id = half === 'light' ? a.light_theme : a.dark_theme
  return [...builtinThemes, ...a.custom_themes].find(t => t.id === id) || builtinThemes[0]!
}
export function activeTheme(a: Appearance, systemLight?: boolean): Theme { return themeFor(a, appearanceHalf(a, systemLight)) }
export function appearanceHalf(a: Appearance, systemLight = matchMedia('(prefers-color-scheme: light)').matches): 'light'|'dark' { return a.mode === 'light' || (a.mode === 'system' && systemLight) ? 'light' : 'dark' }
/** Controls need a stronger edge than the hairline used between panels. */
export function controlBorder(colors: ThemeColors): string {
  const surfaces = [colors.bg, colors.bg2, colors.bg3]
  const start = lab(colors.line)[0]!
  const direction = lab(colors.ink)[0]! > start ? 1 : -1
  for (let step = 0; step <= 100; step++) {
    const color = withLightness(colors.line, start + direction * step / 100)
    if (surfaces.every(bg => contrast(color, bg) >= 3)) return color
  }
  // Unusual custom palettes may span both extremes; use the more visible edge.
  const minimum = (color: string) => Math.min(...surfaces.map(bg => contrast(color, bg)))
  return minimum('#000000') > minimum('#ffffff') ? '#000000' : '#ffffff'
}
export function applyTheme(t: Theme, half: 'light'|'dark' = 'dark', a?: Appearance) {
  const root = document.documentElement, s = root.style, colors=t[half]
  for (const key of colorRoles) s.setProperty(`--${key}`, colors[key])
  s.setProperty('--line-strong', controlBorder(colors))
  s.setProperty('--accent-dim', `color-mix(in srgb, ${colors.accent} 55%, ${colors.bg})`)
  s.setProperty('--accent-glow', `color-mix(in srgb, ${colors.accent} 18%, transparent)`)
  // Per-person tints (avatars) sit at a lightness chosen from the actual background,
  // not the half's name, so a custom pale "dark" theme still gets readable initials.
  const pale = lab(colors.bg)[0]! > .5
  s.setProperty('--tint-bg-l', pale ? '.86' : '.4'); s.setProperty('--tint-ink-l', pale ? '.42' : '.88')
  // Black shadows at dark-theme strength look like soot on a pale page, and a dark
  // accent mixed into cream reads as a stain, so pale themes get lighter versions.
  s.setProperty('--shadow', pale ? 'rgba(40, 30, 10, .12)' : 'rgba(0, 0, 0, .34)')
  s.setProperty('--shadow-lg', pale ? 'rgba(40, 30, 10, .2)' : 'rgba(0, 0, 0, .46)')
  s.setProperty('--pool', pale ? `color-mix(in srgb, ${colors.accent} 14%, #fff)` : `color-mix(in srgb, ${colors.accent} 22%, transparent)`)
  s.setProperty('--selection', 'var(--accent-dim)'); s.setProperty('--mention-bg', 'var(--accent-glow)')
  for (const [key,value] of Object.entries(t.fonts)) s.setProperty(`--${key}`, `"${value}", ${key === 'mono' ? '"Den Terminal Mono", monospace' : 'system-ui, sans-serif'}`)
  const radii = { sharp: [2,6], soft: [6,12], round: [10,18] }[t.radius]
  s.setProperty('--r', `${radii[0]}px`); s.setProperty('--r-lg', `${radii[1]}px`)
  // Chips and avatars follow Corners too, or a Sharp theme ends up full of pills.
  s.setProperty('--r-pill', t.radius === 'sharp' ? `${radii[0]}px` : '999px')
  s.setProperty('--r-avatar', { sharp: '14%', soft: '35%', round: '50%' }[t.radius])
  s.setProperty('--density', t.density === 'compact' ? '0.8' : '1')
  s.setProperty('--leading', t.density === 'compact' ? '1.35' : '1.45')
  s.backgroundColor = colors.bg; s.color = colors.ink; s.colorScheme = half; root.dataset.theme = t.id
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', colors.bg)
  applyFavicon(colors)
  applyContrast(colors, a?.contrast ?? 100)
  applyBackground(a?.background ?? null, half, colors)
}

/** Contrast pulls the two dimmer ink tones toward or away from the background. */
function applyContrast(c: ThemeColors, percent: number) {
  const s = document.documentElement.style
  const amount = Math.max(80, Math.min(120, percent))
  if (amount === 100) { s.setProperty('--ink2', c.ink2); s.setProperty('--ink3', c.ink3); s.setProperty('--line', c.line); return }
  const mix = (from: string, toward: string, pct: number) => `color-mix(in srgb, ${from} ${100 - pct}%, ${toward})`
  const d = Math.abs(amount - 100) * 0.55
  const toward = amount > 100 ? c.ink : c.bg
  s.setProperty('--ink2', mix(c.ink2, toward, d))
  s.setProperty('--ink3', mix(c.ink3, toward, d))
  s.setProperty('--line', mix(c.line, amount > 100 ? c.ink3 : c.bg, d))
}

/** The tab icon is the Den mark drawn in the active theme, so a Den tab looks like your Den. */
export function applyFavicon(c: ThemeColors) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">`
    + `<rect width="64" height="64" rx="14" fill="${c.bg}"/>`
    + `<path fill="${c.ink}" fill-rule="evenodd" d="M44 4h12v52H30C16 56 8 48 8 36s9-22 22-22c6 0 10 2 14 5V4ZM22 56V36a10 10 0 0 1 20 0v20H22Z"/>`
    + `<path fill="${c.accent}" d="M26 56V36a6 6 0 0 1 12 0v20H26Z"/></svg>`
  let link = document.querySelector<HTMLLinkElement>('link#den-favicon')
  if (!link) {
    link = document.createElement('link')
    link.id = 'den-favicon'; link.rel = 'icon'; link.type = 'image/svg+xml'
    document.head.append(link)
  }
  link.href = `data:image/svg+xml,${encodeURIComponent(svg)}`
}
export function cachedAppearance(origin?: string): Appearance {
  try {
    const a = JSON.parse(localStorage.getItem(appearanceCacheKey(origin)) || 'null')
    if (a && ['light','dark','system'].includes(a.mode) && Array.isArray(a.custom_themes)) {
      if (a.theme && !a.dark_theme) { a.dark_theme = a.theme; a.light_theme = a.theme }
      a.light_theme = a.light_theme === 'den-light' ? 'den' : (a.light_theme || 'den')
      a.dark_theme = a.dark_theme === 'den-light' ? 'den' : (a.dark_theme || 'den')
      a.contrast = typeof a.contrast === 'number' ? a.contrast : 100
      if (false) {
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

export function applyBackground(b: AppearanceBackground | null | undefined, half: 'light'|'dark', colors?: ThemeColors) {
  const root = document.documentElement, s = root.style
  if (!b || !b.source) { delete root.dataset.bgScope; s.removeProperty('--bg-image'); return }
  // Presets mix every role, so a partial palette would emit `undefined` into a
  // color-mix() and invalidate the whole background-image.
  const c = colors || (colorRoles.reduce((acc, role) => {
    acc[role] = s.getPropertyValue(`--${role}`).trim()
    return acc
  }, {} as Record<string, string>) as unknown as ThemeColors)
  const image = b.source.type === 'builtin'
    ? wallpaperImage(b.source.name, c)
    : `url("${uploadedBackgroundUrl(b.source.id)}")`
  if (image === 'none') { delete root.dataset.bgScope; s.removeProperty('--bg-image'); return }
  root.dataset.bgScope = b.scope
  s.setProperty('--bg-image', image)
  s.setProperty('--bg-blur', `${Math.max(0, Math.min(40, b.blur))}px`)
  s.setProperty('--bg-dim', String(Math.max(0, Math.min(80, b.dim)) / 100))
  s.setProperty('--bg-saturate', String(Math.max(50, Math.min(150, b.saturate)) / 100))
  s.setProperty('--bg-size', b.fit === 'tile' ? 'auto' : b.fit)
  s.setProperty('--bg-repeat', b.fit === 'tile' ? 'repeat' : 'no-repeat')
  // Photographs need the scrim to match the appearance, not the other way round.
  s.setProperty('--bg-scrim', half === 'light' ? '255,255,255' : '0,0,0')
}

/** Cache-busted so a replaced image shows immediately. */
/** A library image by content ID. The ID changes with the bytes, so it caches safely. */
export function uploadedBackgroundUrl(id: string, preview = false) { return mediaUrl(`/users/me/backgrounds/${id}${preview ? '/preview' : ''}`) }
