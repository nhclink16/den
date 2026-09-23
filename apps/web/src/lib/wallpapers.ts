// Den's preset wallpapers. Each is drawn from the active palette, so it belongs to
// whatever theme you run, and each is a thing from a den's world rather than an
// abstract blur: a lamp, a doorway, the hills outside, a blanket, the trees at
// night, paper. Pure functions: an SVG or gradient string per palette, no assets.

type Colors = { bg: string; bg2: string; bg3: string; line: string; ink: string; ink2: string; ink3: string; accent: string; success: string; danger: string }

export const wallpapers = ['lamplight', 'doorway', 'contours', 'plaid', 'clearing', 'paper'] as const
export type Wallpaper = typeof wallpapers[number]
export const wallpaperNames: Record<Wallpaper, string> = {
  lamplight: 'Lamplight', doorway: 'Doorway', contours: 'Contours', plaid: 'Plaid', clearing: 'Clearing', paper: 'Paper',
}
/** Names saved before the redesign still open the nearest new one. */
export const legacyWallpapers: Record<string, Wallpaper> = {
  aurora: 'clearing', dunes: 'contours', harbor: 'doorway', 'ember-sky': 'lamplight', 'slate-mist': 'plaid', grain: 'paper',
}

// --- colour ------------------------------------------------------------------
const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16))
const hex = (c: number[]) => '#' + c.map((v) => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, '0')).join('')
/** `a` moved `t` of the way to `b`. */
export const mix = (a: string, b: string, t: number) => { const x = rgb(a), y = rgb(b); return hex(x.map((v, i) => v + (y[i]! - v) * t)) }
const luminance = (c: string) => { const [r, g, b] = rgb(c).map((v) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4 }); return 0.2126 * r! + 0.7152 * g! + 0.0722 * b! }
const pale = (c: Colors) => luminance(c.bg) > 0.35

// A seeded generator, so stars and hills land in the same place on every device.
function random(seed: number) { return () => { seed = (seed * 1664525 + 1013904223) >>> 0; return seed / 2 ** 32 } }

const W = 1600, H = 1000
const svg = (body: string, extra = '') =>
  `url("data:image/svg+xml,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 ${W} ${H}' preserveAspectRatio='xMidYMid slice'${extra}>${body}</svg>`)}")`

// --- the six -----------------------------------------------------------------

/** A lamp just out of frame, top right: a soft cone of light and the pool it leaves on the floor. */
function lamplight(c: Colors) {
  const light = pale(c) ? mix(c.accent, '#ffffff', 0.72) : mix(c.accent, '#ffffff', 0.15)
  const shade = pale(c) ? mix(c.ink, c.bg, 0.9) : '#000000'
  return svg(`<defs>
    <linearGradient id='w' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='${c.bg2}'/><stop offset='1' stop-color='${mix(c.bg, shade, pale(c) ? 0.05 : 0.25)}'/></linearGradient>
    <linearGradient id='c' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='${light}' stop-opacity='${pale(c) ? 0.9 : 0.5}'/><stop offset='1' stop-color='${light}' stop-opacity='0'/></linearGradient>
    <radialGradient id='p'><stop offset='0' stop-color='${light}' stop-opacity='${pale(c) ? 0.8 : 0.42}'/><stop offset='1' stop-color='${light}' stop-opacity='0'/></radialGradient>
    <radialGradient id='v' cx='.72' cy='.2' r='1'><stop offset='.35' stop-color='${shade}' stop-opacity='0'/><stop offset='1' stop-color='${shade}' stop-opacity='${pale(c) ? 0.12 : 0.55}'/></radialGradient>
    <filter id='s' x='-50%' y='-50%' width='200%' height='200%'><feGaussianBlur stdDeviation='38'/></filter>
  </defs>
  <rect width='${W}' height='${H}' fill='url(#w)'/>
  <polygon points='1150,-10 1330,-10 1620,1010 760,1010' fill='url(#c)' filter='url(#s)'/>
  <ellipse cx='1190' cy='975' rx='470' ry='95' fill='url(#p)' filter='url(#s)'/>
  <circle cx='1240' cy='-30' r='120' fill='${light}' opacity='${pale(c) ? 0.9 : 0.55}' filter='url(#s)'/>
  <rect width='${W}' height='${H}' fill='url(#v)'/>`)
}

/** The doorway from Den's mark, lit from inside, throwing light across a dark floor. */
function doorway(c: Colors) {
  // Muted in the dark: the door sits behind conversation, so it glows rather than blazes.
  const light = pale(c) ? mix(c.accent, '#ffffff', 0.55) : mix(c.accent, c.bg, 0.3)
  const wall = pale(c) ? mix(c.bg, c.ink, 0.06) : mix(c.bg, '#000000', 0.18)
  const floor = pale(c) ? mix(c.bg, c.ink, 0.12) : mix(c.bg, '#000000', 0.42)
  const x = 1120, w = 130, top = 620, base = 860
  const arch = `M${x},${base} V${top + w / 2} A${w / 2},${w / 2} 0 0 1 ${x + w},${top + w / 2} V${base} Z`
  const panels = Array.from({ length: 9 }, (_, i) => `<line x1='${80 + i * 180}' y1='0' x2='${80 + i * 180}' y2='${base}' stroke='${c.ink}' stroke-opacity='.035' stroke-width='2'/>`).join('')
  return svg(`<defs>
    <linearGradient id='d' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='${light}' stop-opacity='.75'/><stop offset='1' stop-color='${light}'/></linearGradient>
    <linearGradient id='f' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='${light}' stop-opacity='${pale(c) ? 0.7 : 0.5}'/><stop offset='1' stop-color='${light}' stop-opacity='0'/></linearGradient>
    <filter id='g' x='-100%' y='-100%' width='300%' height='300%'><feGaussianBlur stdDeviation='50'/></filter>
    <filter id='t' x='-20%' y='-20%' width='140%' height='140%'><feGaussianBlur stdDeviation='14'/></filter>
  </defs>
  <rect width='${W}' height='${base}' fill='${wall}'/>${panels}
  <rect y='${base}' width='${W}' height='${H - base}' fill='${floor}'/>
  <line x1='0' y1='${base}' x2='${W}' y2='${base}' stroke='${c.ink}' stroke-opacity='.08' stroke-width='3'/>
  <path d='${arch}' fill='${light}' opacity='${pale(c) ? 0.5 : 0.35}' filter='url(#g)'/>
  <polygon points='${x},${base} ${x + w},${base} ${x + w + 380},${H} ${x - 300},${H}' fill='url(#f)' filter='url(#t)'/>
  <path d='${arch}' fill='url(#d)'/>`)
}

/** A survey map of the hills outside. Every fifth line is an index contour, drawn heavier, the way real maps do. */
function contours(c: Colors) {
  const rand = random(7)
  const line = pale(c) ? c.ink : mix(c.ink, c.bg, 0.25)
  const hills = [{ x: 1320, y: 760, rings: 26, step: 34 }, { x: 260, y: 170, rings: 11, step: 30 }]
  let body = ''
  for (const hill of hills) {
    const phase = [rand() * 6.28, rand() * 6.28, rand() * 6.28]
    for (let k = 1; k <= hill.rings; k++) {
      const points: string[] = []
      for (let i = 0; i <= 96; i++) {
        const a = (i / 96) * Math.PI * 2
        // Wobble grows with distance, so the summit is round and the slopes wander.
        const wobble = 1 + (0.08 * Math.sin(3 * a + phase[0]!) + 0.05 * Math.sin(5 * a + phase[1]!) + 0.03 * Math.sin(8 * a + phase[2]! + k * 0.15)) * Math.min(1, k / 6)
        const r = k * hill.step * wobble
        points.push(`${(hill.x + Math.cos(a) * r * 1.25).toFixed(1)},${(hill.y + Math.sin(a) * r).toFixed(1)}`)
      }
      const index = k % 5 === 0
      body += `<polyline points='${points.join(' ')}' fill='none' stroke='${index ? c.accent : line}' stroke-opacity='${index ? 0.28 : pale(c) ? 0.1 : 0.12}' stroke-width='${index ? 2.4 : 1.3}'/>`
    }
  }
  return svg(`<rect width='${W}' height='${H}' fill='${c.bg}'/>${body}`)
}

/** A wool blanket: two stripe colours over a fine twill, at the quiet end of the palette. */
function plaid(c: Colors) {
  const a = c.accent, g = c.success, i = c.ink
  const band = (color: string, alpha: number) => `${color}${Math.round(alpha * 255).toString(16).padStart(2, '0')}`
  const k = pale(c) ? 1 : 1.25
  const stripes = (deg: number) => `repeating-linear-gradient(${deg}deg, transparent 0 46px, ${band(a, 0.1 * k)} 46px 78px, transparent 78px 88px, ${band(i, 0.07 * k)} 88px 92px, transparent 92px 100px, ${band(g, 0.07 * k)} 100px 108px, transparent 108px 150px)`
  return [
    `repeating-linear-gradient(45deg, ${band(i, 0.035)} 0 1px, transparent 1px 4px)`,
    stripes(90), stripes(0),
    `linear-gradient(${c.bg}, ${mix(c.bg, c.bg2, 0.6)})`,
  ].join(', ')
}

/** The tree line around the den: stars over pines at night, first light in a pale theme. */
function clearing(c: Colors) {
  const rand = random(11)
  const light = pale(c)
  const sky0 = light ? mix(c.bg, c.bg3, 0.5) : mix(c.bg, '#000000', 0.35)
  const sky1 = light ? mix(c.accent, '#ffffff', 0.6) : mix(c.bg2, c.accent, 0.18)
  const trees = light ? mix(c.ink, c.bg, 0.55) : mix(c.bg, '#000000', 0.55)
  const far = light ? mix(c.ink, c.bg, 0.78) : mix(c.bg2, '#000000', 0.25)
  // Pines: a trunk-less stack of three tiers, each narrower than the one below.
  const pines = (base: number, height: number, spacing: number, seed: () => number) => {
    let d = ''
    for (let x = -spacing; x <= W + spacing; x += spacing * (0.45 + seed() * 0.8)) {
      const h = height * (0.5 + seed() * 0.7), w = h * (0.34 + seed() * 0.08), top = base - h
      for (let t = 0; t < 3; t++) {
        const y0 = top + (h * t) / 3.2, y1 = top + (h * (t + 1.6)) / 3.2, half = (w / 2) * (0.55 + t * 0.3)
        d += `M${(x - half).toFixed(1)},${y1.toFixed(1)} L${x.toFixed(1)},${y0.toFixed(1)} L${(x + half).toFixed(1)},${y1.toFixed(1)} Z `
      }
      d += `M${(x - w * 0.08).toFixed(1)},${(base - h * 0.2).toFixed(1)} h${(w * 0.16).toFixed(1)} V${base} h${(-w * 0.16).toFixed(1)} Z `
    }
    return d + `M0,${base - 2} H${W} V${H} H0 Z`
  }
  let stars = ''
  if (!light) for (let n = 0; n < 170; n++) {
    const x = rand() * W, y = rand() * 700, bright = rand() > 0.93
    stars += `<circle cx='${x.toFixed(0)}' cy='${y.toFixed(0)}' r='${bright ? 2.2 : 0.6 + rand() * 1.1}' fill='${bright ? mix(c.accent, '#ffffff', 0.6) : '#ffffff'}' opacity='${(0.25 + rand() * 0.55).toFixed(2)}'/>`
  }
  return svg(`<defs>
    <linearGradient id='k' x1='0' y1='0' x2='0' y2='1'><stop offset='0' stop-color='${sky0}'/><stop offset='.85' stop-color='${sky1}'/></linearGradient>
    <filter id='b' x='-20%' y='-50%' width='140%' height='200%'><feGaussianBlur stdDeviation='60'/></filter>
  </defs>
  <rect width='${W}' height='${H}' fill='url(#k)'/>
  ${light ? '' : `<ellipse cx='800' cy='260' rx='900' ry='90' transform='rotate(-18 800 260)' fill='#ffffff' opacity='.05' filter='url(#b)'/>`}${stars}
  <path d='${pines(900, 110, 40, random(3))}' fill='${far}'/>
  <path d='${pines(975, 180, 60, random(5))}' fill='${trees}'/>`)
}

/** Paper: long fibres and a fine tooth, tinted to the page. */
function paper(c: Colors) {
  const tint = pale(c) ? c.ink : '#ffffff'
  return svg(`<defs>
    <filter id='f'><feTurbulence type='fractalNoise' baseFrequency='0.0035 0.05' numOctaves='3' seed='4'/><feColorMatrix type='matrix' values='0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 -1.4 1.1'/></filter>
    <filter id='n'><feTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' seed='9'/><feColorMatrix type='matrix' values='0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 -1 .9'/></filter>
    <radialGradient id='v' r='.9'><stop offset='.5' stop-color='${c.bg}' stop-opacity='0'/><stop offset='1' stop-color='${pale(c) ? c.ink : '#000000'}' stop-opacity='${pale(c) ? 0.06 : 0.3}'/></radialGradient>
  </defs>
  <rect width='${W}' height='${H}' fill='${mix(c.bg, c.bg2, 0.5)}'/>
  <rect width='${W}' height='${H}' fill='${tint}' opacity='${pale(c) ? 0.07 : 0.13}' filter='url(#f)'/>
  <rect width='${W}' height='${H}' fill='${tint}' opacity='${pale(c) ? 0.08 : 0.12}' filter='url(#n)'/>
  <rect width='${W}' height='${H}' fill='url(#v)'/>`)
}

const painters: Record<Wallpaper, (c: Colors) => string> = { lamplight, doorway, contours, plaid, clearing, paper }

/** The CSS background-image for a preset, or `none` for a name this build does not know. */
export function wallpaperImage(name: string, c: Colors): string {
  const key = (wallpapers as readonly string[]).includes(name) ? name as Wallpaper : legacyWallpapers[name]
  return key ? painters[key](c) : 'none'
}
