import type { Appearance, Theme } from './types'
import builtins from '../../../../crates/den-core/src/themes.json'

export const builtinThemes = builtins as Theme[]
export const defaultAppearance: Appearance = { mode: 'system', light_theme: 'den-light', dark_theme: 'den', custom_themes: [] }
export function appearanceCacheKey(origin?: string) { return (window.__TAURI__ ? `den.appearance:${origin || localStorage.getItem('den.native.origin') || 'https://denchat.app'}` : 'den.appearance') }
export function activeTheme(a: Appearance, systemLight = matchMedia('(prefers-color-scheme: light)').matches): Theme {
  const light = a.mode === 'light' || (a.mode === 'system' && systemLight)
  return [...builtinThemes, ...a.custom_themes].find(t => t.id === (light ? a.light_theme : a.dark_theme)) || builtinThemes[light ? 1 : 0]!
}
export function applyTheme(t: Theme) {
  const root = document.documentElement, s = root.style
  for (const [key,value] of Object.entries(t.colors)) s.setProperty(`--${key}`, value)
  s.setProperty('--accent-dim', `color-mix(in srgb, ${t.colors.accent} 55%, ${t.colors.bg})`)
  s.setProperty('--accent-glow', `color-mix(in srgb, ${t.colors.accent} 18%, transparent)`)
  s.setProperty('--selection', 'var(--accent-dim)')
  s.setProperty('--mention-bg', 'var(--accent-glow)')
  for (const [key,value] of Object.entries(t.fonts)) s.setProperty(`--${key}`, `"${value}", ${key === 'mono' ? '"Den Terminal Mono", monospace' : 'system-ui, sans-serif'}`)
  const radii = { sharp: [2,6], soft: [6,12], round: [10,18] }[t.radius]
  s.setProperty('--r', `${radii[0]}px`); s.setProperty('--r-lg', `${radii[1]}px`)
  s.setProperty('--density', t.density === 'compact' ? '0.8' : '1')
  s.backgroundColor = t.colors.bg; s.color = t.colors.ink
  s.colorScheme = t.appearance; root.dataset.theme = t.id
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', t.colors.bg)
}
export function cachedAppearance(origin?: string): Appearance {
  try { const a = JSON.parse(localStorage.getItem(appearanceCacheKey(origin)) || 'null'); if (a && ['light','dark','system'].includes(a.mode) && Array.isArray(a.custom_themes)) return a } catch { /* unavailable storage */ }
  return structuredClone(defaultAppearance)
}
export function firstPaint() {
  try { applyTheme(activeTheme(cachedAppearance())) } catch { applyTheme(builtinThemes[0]!) }
}
