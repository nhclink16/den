// Palette maths with no imports but data, so Node tests and the inline
// first-paint script can both run it. The Rust twin is den-core/theme_color.rs.
import type { ThemeColors } from './types'
import accents from '../../../../crates/den-core/src/accents.json' with { type: 'json' }

export const colorRoles = ['bg','bg2','bg3','line','ink','ink2','ink3','accent','success','danger'] as const
/** Accent presets: the same list den-core validates against. */
export const accentPresets = accents as { id: string; name: string; light: string; dark: string }[]
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
/** `color` moved in lightness, away from the first surface, until it reads at
 *  4.5:1 on every surface given; black or white when no lightness does. */
export function readable(color: string, surfaces: string[]) {
  const start = lab(color)[0]!, direction = lab(surfaces[0]!)[0]! > .5 ? -1 : 1
  const worst = (c: string) => Math.min(...surfaces.map(bg => contrast(c, bg)))
  let out = color
  for (let step = 0; step <= 1000; step++) { out = withLightness(color, start + direction * step / 1000); if (worst(out) >= 4.5) return out }
  return worst('#000000') >= worst('#ffffff') ? '#000000' : '#ffffff'
}
/** The accent the person chose, readable on this half's surfaces. Null keeps the
 *  theme's own, which the theme's author already checked. */
export function resolveAccent(colors: ThemeColors, choice: string | null | undefined, half: 'light'|'dark'): string {
  if (!choice) return colors.accent
  const preset = accentPresets.find(p => p.id === choice)
  const wanted = preset ? preset[half] : /^#[0-9a-f]{6}$/i.test(choice) ? choice.toLowerCase() : null
  return wanted ? readable(wanted, [colors.bg, colors.bg2, colors.bg3]) : colors.accent
}
export function deriveHalf(source: ThemeColors): ThemeColors {
  const out={...source,generated:true}
  for (const [bg,ink] of [['bg','ink'],['bg2','ink2'],['bg3','ink3']] as const) {
    out[bg]=withLightness(source[bg],lab(source[ink])[0]!); out[ink]=withLightness(source[ink],lab(source[bg])[0]!)
  }
  const direction=lab(out.bg)[0]!>.5?-1:1
  out.line=withLightness(source.line,lab(out.bg)[0]!+direction*.12)
  for (const role of ['accent','success','danger'] as const) out[role]=readable(source[role],[out.bg])
  return out
}
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
