import { apiFor } from './api'
import { activeOrigin } from './native'
import type { Appearance, Theme, ThemeFonts } from './types'
import { themeFor, appearanceHalf, deriveHalf, colorRoles, applyTheme, appearanceCacheKey, builtinThemes, cachedAppearance, defaultAppearance } from './theme-runtime'
export { builtinThemes }
export const fontFamilies = [...new Set([...builtinThemes.flatMap(t => Object.values(t.fonts)), 'Inter', 'Instrument Sans', 'Space Grotesk', 'Nunito', 'Lora', 'Fraunces', 'Commit Mono'])].sort()
export const validName = (v: string) => [...v].length >= 1 && [...v].length <= 40 && !!v.trim() && /^[\p{L}\p{N} _-]+$/u.test(v)
export function validateTheme(value: unknown): Theme {
  if (!value || typeof value !== 'object') throw Error('Invalid theme.')
  const t = structuredClone(value) as Theme & { appearance?: 'light'|'dark'; colors?: Theme['light'] }
  const keys = (v: unknown, expected: string[]) => !!v && typeof v === 'object' && Object.keys(v).sort().join() === [...expected].sort().join()
  const colors = (v: unknown) => !!v && typeof v === 'object' && keys(v, [...colorRoles, ...('generated' in v ? ['generated'] : [])]) && colorRoles.every(k => /^#[0-9a-f]{6}$/i.test((v as Theme['light'])[k])) && (!('generated' in v) || typeof v.generated === 'boolean')
  if (t.colors && ['light','dark'].includes(t.appearance || '')) {
    if(!keys(t,['id','name','appearance','colors','fonts','radius','density']) || !colors(t.colors)) throw Error('Invalid theme.')
    t[t.appearance!] = t.colors; t[t.appearance === 'dark' ? 'light' : 'dark'] = deriveHalf(t.colors)
    delete t.colors; delete t.appearance
  }
  // Imports may provide one authored half; built-ins always provide both.
  if (!t.light && colors(t.dark)) t.light=deriveHalf(t.dark)
  if (!t.dark && colors(t.light)) t.dark=deriveHalf(t.light)
  if (!keys(t, ['id','name','light','dark','fonts','radius','density']) || typeof t.id!=='string' || typeof t.name!=='string' || !validName(t.id) || !validName(t.name)
    || !['sharp','soft','round'].includes(t.radius) || !['compact','comfortable'].includes(t.density)
    || !colors(t.light) || !colors(t.dark) || !keys(t.fonts, ['display','body','mono']) || !Object.values(t.fonts).every(v => typeof v === 'string' && validName(v))) throw Error('Invalid theme. Use six-digit hex colors and names of 1–40 letters, numbers, spaces, hyphens or underscores.')
  return t
}
class Themes {
  appearance = $state<Appearance>(cachedAppearance())
  draft = $state<Theme | null>(null)
  error = $state('')
  saving = $state(false)
  private systemLight = $state(matchMedia('(prefers-color-scheme: light)').matches)
  private fontRun = 0
  private queue: Promise<unknown> = Promise.resolve()
  private pending = 0
  get all() { return [...builtinThemes, ...this.appearance.custom_themes] }
  get active() { return this.draft || themeFor(this.appearance, this.half) }
  /** The theme currently assigned to one appearance, ignoring any live draft. */
  chosen(half: 'light'|'dark') { return themeFor(this.appearance, half) }
  get half() { return appearanceHalf(this.appearance, this.systemLight) }
  constructor() {
    this.apply()
    matchMedia('(prefers-color-scheme: light)').addEventListener('change', e => { this.systemLight = e.matches; this.apply() })
  }
  receive(a: Appearance, force = false) {
    if (this.pending && !force) return
    this.appearance = a
    try { localStorage.setItem(appearanceCacheKey(), JSON.stringify(a)) } catch { /* cache is optional */ }
    this.apply()
  }
  apply() { applyTheme(this.active, this.half, this.appearance); void this.loadFonts(this.active.fonts) }
  async loadFonts(fonts: ThemeFonts) {
    const families = [...new Set(Object.values(fonts))].filter(f => f !== 'IBM Plex Mono')
    const url = `https://fonts.googleapis.com/css2?${families.map(f => `family=${encodeURIComponent(f)}:wght@400;500;600;700`).join('&')}&display=swap`
    let link = document.querySelector<HTMLLinkElement>('#den-fonts')
    if (link?.href === url) return
    const run = ++this.fontRun
    link?.remove()
    if (!families.length) return
    link = document.createElement('link'); link.id = 'den-fonts'; link.rel = 'stylesheet'; link.href = url
    let timer: ReturnType<typeof setTimeout>
    const loaded = new Promise<boolean>(resolve => { link!.onload = () => resolve(true); link!.onerror = () => resolve(false) })
    document.head.append(link)
    const ok = await Promise.race([loaded.then(async ok => ok && (await Promise.all(families.map(f => document.fonts.load(`16px "${f}"`)))).every(f => f.length > 0)).catch(() => false), new Promise<boolean>(resolve => { timer = setTimeout(() => resolve(false), 3000) })])
    clearTimeout(timer!)
    if (run !== this.fontRun) return
    if (!ok) {
      link.remove(); this.error = "Couldn't load that font"
      for (const [role, family] of Object.entries(fonts)) if (family !== 'IBM Plex Mono') document.documentElement.style.setProperty(`--${role}`, role === 'mono' ? '"Den Terminal Mono", monospace' : 'system-ui, sans-serif')
    } else if (this.error === "Couldn't load that font") this.error = ''
  }
  save(a: Appearance, keepDraft = false) {
    const origin = activeOrigin(), api = apiFor(origin)
    const next = JSON.parse(JSON.stringify(a)) as Appearance
    this.error = ''; if (!keepDraft) this.draft = null; this.pending++; this.receive(next, true); this.saving = true
    const result = this.queue.catch(() => {}).then(async () => {
      try {
        const saved = await api.put<Appearance>('/users/me/appearance', next)
        if (this.pending === 1 && activeOrigin() === origin) this.receive(saved, true)
        return true
      } catch (e) {
        if (activeOrigin() === origin) this.error = (e as Error).message
        if (this.pending === 1 && activeOrigin() === origin) {
          try { this.receive(await api.get<Appearance>('/users/me/appearance'), true) } catch { /* keep local preview while offline */ }
        }
        return false
      }
    }).finally(() => { this.pending--; this.saving = this.pending > 0 })
    this.queue = result
    return result
  }
  /** Assign a theme to one appearance. Light and dark are independent. */
  select(t: Theme, half: 'light'|'dark' = this.half) {
    return this.save({ ...this.appearance, [half === 'light' ? 'light_theme' : 'dark_theme']: t.id })
  }
  contrast(percent: number) { return this.save({ ...this.appearance, contrast: percent }, true) }
  background(b: Appearance['background']) { return this.save({ ...this.appearance, background: b ?? null }, true) }
  mode(mode: Appearance['mode']) { return this.save({...this.appearance,mode},true) }
  preview(t: Theme) { this.draft = t; this.apply() }
  reset() { this.draft = null; this.apply() }
  async add(t: Theme) {
    const custom = [...this.appearance.custom_themes, validateTheme(t)]
    if (custom.length > 12 || new TextEncoder().encode(JSON.stringify(custom)).length > 16384) throw Error('Use at most 12 custom themes and 16 KiB total.')
    const saved = await this.save({ ...this.appearance, custom_themes: custom, [this.half === 'light' ? 'light_theme' : 'dark_theme']: t.id })
    if (!saved) throw Error(this.error)
  }
  async remove(id: string) {
    const a = this.appearance
    await this.save({
      ...a,
      custom_themes: a.custom_themes.filter(t => t.id !== id),
      light_theme: a.light_theme === id ? defaultAppearance.light_theme : a.light_theme,
      dark_theme: a.dark_theme === id ? defaultAppearance.dark_theme : a.dark_theme,
    })
  }
}
export const themes = new Themes()
