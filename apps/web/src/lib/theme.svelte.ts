import { api } from './api'
import type { Appearance, Theme, ThemeFonts } from './types'
import { activeTheme, applyTheme, appearanceKey, builtinThemes, cachedAppearance, defaultAppearance } from './theme-runtime'
export { builtinThemes }
export const fontFamilies = [...new Set([...builtinThemes.flatMap(t => Object.values(t.fonts)), 'Inter', 'Instrument Sans', 'Space Grotesk', 'Nunito', 'Lora', 'Fraunces', 'Commit Mono'])].sort()
export const validName = (v: string) => [...v].length >= 1 && [...v].length <= 40 && !!v.trim() && /^[\p{L}\p{N} _-]+$/u.test(v)
export function validateTheme(value: unknown): Theme {
  const t = value as Theme
  const keys = (v: unknown, expected: string[]) => !!v && typeof v === 'object' && Object.keys(v).sort().join() === expected.sort().join()
  if (!keys(t, ['id','name','appearance','colors','fonts','radius','density']) || !validName(t.id) || !validName(t.name)
    || !['light','dark'].includes(t.appearance) || !['sharp','soft','round'].includes(t.radius) || !['compact','comfortable'].includes(t.density)
    || !keys(t.colors, ['bg','bg2','bg3','line','ink','ink2','ink3','accent','success','danger']) || !Object.values(t.colors).every(v => typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v))
    || !keys(t.fonts, ['display','body','mono']) || !Object.values(t.fonts).every(v => typeof v === 'string' && validName(v))) throw Error('Invalid theme. Use six-digit hex colors and names of 1–40 letters, numbers, spaces, hyphens or underscores.')
  return structuredClone(t)
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
  get active() { return this.draft || activeTheme(this.appearance, this.systemLight) }
  constructor() {
    this.apply()
    matchMedia('(prefers-color-scheme: light)').addEventListener('change', e => { this.systemLight = e.matches; this.apply() })
  }
  receive(a: Appearance, force = false) {
    if (this.pending && !force) return
    this.appearance = a
    try { localStorage.setItem(appearanceKey, JSON.stringify(a)) } catch { /* cache is optional */ }
    this.apply()
  }
  apply() { applyTheme(this.active); void this.loadFonts(this.active.fonts) }
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
  save(a: Appearance) {
    const next = JSON.parse(JSON.stringify(a)) as Appearance
    this.error = ''; this.draft = null; this.pending++; this.receive(next, true); this.saving = true
    const result = this.queue.catch(() => {}).then(async () => {
      try {
        const saved = await api.put<Appearance>('/users/me/appearance', next)
        if (this.pending === 1) this.receive(saved, true)
        return true
      } catch (e) {
        this.error = (e as Error).message
        if (this.pending === 1) {
          try { this.receive(await api.get<Appearance>('/users/me/appearance'), true) } catch { /* keep local preview while offline */ }
        }
        return false
      }
    }).finally(() => { this.pending--; this.saving = this.pending > 0 })
    this.queue = result
    return result
  }
  select(t: Theme) { return this.save({ ...this.appearance, [t.appearance === 'light' ? 'light_theme' : 'dark_theme']: t.id, mode: this.appearance.mode === 'system' ? 'system' : t.appearance }) }
  preview(t: Theme) { this.draft = t; this.apply() }
  reset() { this.draft = null; this.apply() }
  async add(t: Theme) {
    const custom = [...this.appearance.custom_themes, validateTheme(t)]
    if (custom.length > 12 || new TextEncoder().encode(JSON.stringify(custom)).length > 16384) throw Error('Use at most 12 custom themes and 16 KiB total.')
    const saved = await this.save({ ...this.appearance, custom_themes: custom, [t.appearance === 'light' ? 'light_theme' : 'dark_theme']: t.id, mode: this.appearance.mode === 'system' ? 'system' : t.appearance })
    if (!saved) throw Error(this.error)
  }
  async remove(id: string) {
    const a = this.appearance
    await this.save({ ...a, custom_themes: a.custom_themes.filter(t => t.id !== id), light_theme: a.light_theme === id ? defaultAppearance.light_theme : a.light_theme, dark_theme: a.dark_theme === id ? defaultAppearance.dark_theme : a.dark_theme })
  }
}
export const themes = new Themes()
