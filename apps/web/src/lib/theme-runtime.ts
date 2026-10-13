import { isDesktop } from './desktop'
import { mediaUrl } from './native'
import { wallpaperImage } from './wallpapers'
import type { Appearance, AppearanceBackground, Theme, ThemeColors } from './types'
import { colorRoles, controlBorder, deriveHalf, lab, resolveAccent } from './theme-color'
import builtins from '../../../../crates/den-core/src/themes.json'

export const builtinThemes = builtins as Theme[]
export const defaultAppearance: Appearance = { mode: 'system', light_theme: 'neutral', dark_theme: 'neutral', custom_themes: [], contrast: 100, accent: null }
/** "System" is the device's own UI font: SF on Apple devices, Segoe UI on Windows. */
export const systemFont = 'System'
export function fontStack(family: string, role: 'display' | 'body' | 'mono') {
  // Named, not `system-ui`: on Linux that is often DejaVu Sans, far wider than the rest.
  if (family === systemFont) return role === 'mono' ? 'ui-monospace, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace' : '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Noto Sans", Ubuntu, Cantarell, "Helvetica Neue", Arial, sans-serif'
  return `"${family}", ${role === 'mono' ? '"Den Terminal Mono", monospace' : 'system-ui, sans-serif'}`
}
export function appearanceCacheKey(origin?: string) { return (isDesktop() ? `den.appearance:${origin || localStorage.getItem('den.native.origin') || 'https://denchat.app'}` : 'den.appearance') }
/** The theme chosen for one appearance. Light and dark are picked independently. */
export function themeFor(a: Appearance, half: 'light'|'dark'): Theme {
  const id = half === 'light' ? a.light_theme : a.dark_theme
  return [...builtinThemes, ...a.custom_themes].find(t => t.id === id) || builtinThemes[0]!
}
export function activeTheme(a: Appearance, systemLight?: boolean): Theme { return themeFor(a, appearanceHalf(a, systemLight)) }
export function appearanceHalf(a: Appearance, systemLight = matchMedia('(prefers-color-scheme: light)').matches): 'light'|'dark' { return a.mode === 'light' || (a.mode === 'system' && systemLight) ? 'light' : 'dark' }
export function applyTheme(t: Theme, half: 'light'|'dark' = 'dark', a?: Appearance) {
  const root = document.documentElement, s = root.style, colors={...t[half],accent:resolveAccent(t[half],a?.accent,half)}
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
  for (const [key,value] of Object.entries(t.fonts)) s.setProperty(`--${key}`, fontStack(value, key as keyof Theme['fonts']))
  const radii = { sharp: [2,6], soft: [6,12], round: [10,18] }[t.radius]
  s.setProperty('--r', `${radii[0]}px`); s.setProperty('--r-lg', `${radii[1]}px`)
  // Chips and avatars follow Corners too, or a Sharp theme ends up full of pills.
  s.setProperty('--r-pill', t.radius === 'sharp' ? `${radii[0]}px` : '999px')
  s.setProperty('--r-avatar', { sharp: '14%', soft: '35%', round: '50%' }[t.radius])
  // Bubbles follow Corners unless the person picked a shape. The tight corner is
  // the one that points at the speaker, and the joins inside one person's run.
  const bubbles = !a?.bubble_shape || a.bubble_shape === 'corners' ? t.radius : a.bubble_shape
  const bubble = { sharp: [4,2], soft: [10,4], round: [20,6], none: [0,0] }[bubbles] ?? [20,6]
  s.setProperty('--r-bubble', `${bubble[0]}px`); s.setProperty('--r-tight', `${bubble[1]}px`)
  root.dataset.bubbles = bubbles
  s.setProperty('--density', t.density === 'compact' ? '0.8' : '1')
  s.setProperty('--leading', t.density === 'compact' ? '1.35' : '1.45')
  s.backgroundColor = colors.bg; s.color = colors.ink; s.colorScheme = half; root.dataset.theme = t.id
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', colors.bg)
  applyFavicon(colors)
  applyContrast(colors, a?.contrast ?? 100)
  applyBackground(a?.background ?? null, half, colors)
  applySidebarBackground(a?.sidebar_background ?? null, half, colors)
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
/** Same as den-core's rename_clashing_themes: a custom theme whose ID a newer
 *  built-in took keeps its look under `<id> <n>`, and the choices follow it. */
export function renameClashingThemes(a: Appearance) {
  const builtin = (id: string) => builtinThemes.some(t => t.id === id)
  for (const t of a.custom_themes) {
    if (!builtin(t.id)) continue
    const old = t.id
    let n = 1
    while (builtin(`${old} ${n}`) || a.custom_themes.some(c => c.id === `${old} ${n}`)) n++
    t.id = `${old} ${n}`
    if (a.light_theme === old) a.light_theme = t.id
    if (a.dark_theme === old) a.dark_theme = t.id
  }
}
export function cachedAppearance(origin?: string): Appearance {
  try {
    const a = JSON.parse(localStorage.getItem(appearanceCacheKey(origin)) || 'null')
    if (a && ['light','dark','system'].includes(a.mode) && Array.isArray(a.custom_themes)) {
      if (a.theme && !a.dark_theme) { a.dark_theme = a.theme; a.light_theme = a.theme }
      a.light_theme = a.light_theme === 'den-light' ? 'den' : (a.light_theme || defaultAppearance.light_theme)
      a.dark_theme = a.dark_theme === 'den-light' ? 'den' : (a.dark_theme || defaultAppearance.dark_theme)
      a.contrast = typeof a.contrast === 'number' ? a.contrast : 100
      renameClashingThemes(a)
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
  // Wallpapers wait for the app: painting one needs modules this inline copy lacks.
  try { const a=cachedAppearance(); applyTheme(activeTheme(a),appearanceHalf(a),{...a,background:null,sidebar_background:null}) } catch { applyTheme(builtinThemes[0]!) }
}

/** Paints the wallpaper layer. Scope decides which region shows it; app.css does the rest. */
export function applyBackground(b: AppearanceBackground | null | undefined, half: 'light'|'dark', colors?: ThemeColors) {
  const root = document.documentElement
  if (paint('bg', b, half, colors)) root.dataset.bgScope = b!.scope; else delete root.dataset.bgScope
}

/** The sidebar's own wallpaper, when it has one. It wins over the main one there. */
export function applySidebarBackground(b: AppearanceBackground | null | undefined, half: 'light'|'dark', colors?: ThemeColors) {
  const root = document.documentElement
  if (paint('sb', b, half, colors)) root.dataset.sidebarBg = ''; else delete root.dataset.sidebarBg
}

/** Sets `--<prefix>-image` and its adjustments; false when there is nothing to paint. */
function paint(prefix: 'bg' | 'sb', b: AppearanceBackground | null | undefined, half: 'light'|'dark', colors?: ThemeColors): boolean {
  const s = document.documentElement.style
  if (!b || !b.source) { s.removeProperty(`--${prefix}-image`); return false }
  // Presets mix every role, so a partial palette would emit `undefined` into a
  // color-mix() and invalidate the whole background-image.
  const c = colors || (colorRoles.reduce((acc, role) => {
    acc[role] = s.getPropertyValue(`--${role}`).trim()
    return acc
  }, {} as Record<string, string>) as unknown as ThemeColors)
  const image = b.source.type === 'builtin'
    ? wallpaperImage(b.source.name, c)
    : `url("${uploadedBackgroundUrl(b.source.id, false, matchMedia('(prefers-reduced-motion: reduce)').matches)}")`
  if (image === 'none') { s.removeProperty(`--${prefix}-image`); return false }
  s.setProperty(`--${prefix}-image`, image)
  s.setProperty(`--${prefix}-blur`, `${Math.max(0, Math.min(40, b.blur))}px`)
  s.setProperty(`--${prefix}-dim`, String(Math.max(0, Math.min(80, b.dim)) / 100))
  s.setProperty(`--${prefix}-saturate`, String(Math.max(50, Math.min(150, b.saturate)) / 100))
  s.setProperty(`--${prefix}-size`, b.fit === 'tile' ? 'auto' : b.fit)
  s.setProperty(`--${prefix}-repeat`, b.fit === 'tile' ? 'repeat' : 'no-repeat')
  // Photographs need the scrim to match the appearance, not the other way round.
  s.setProperty(`--${prefix}-scrim`, half === 'light' ? '255,255,255' : '0,0,0')
  return true
}

/** A library image by content ID. The ID changes with the bytes, so it caches safely.
 *  `still` asks for a GIF's first frame (any other picture comes back as it is). */
export function uploadedBackgroundUrl(id: string, preview = false, still = false) { return mediaUrl(`/users/me/backgrounds/${id}${preview ? '/preview' : ''}${still ? '?still=1' : ''}`) }
