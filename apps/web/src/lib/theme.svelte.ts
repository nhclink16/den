import { apiFor } from './api'
import { activeOrigin } from './native'
import type { Appearance, Theme, ThemeFonts } from './types'
import { themeFor, appearanceHalf, applyTheme, appearanceCacheKey, builtinThemes, cachedAppearance, defaultAppearance, fontStack, systemFont } from './theme-runtime'
import { deriveHalf, colorRoles } from './theme-color'
import { forgetOldAvatarChoice } from './avatar-carry'
export { builtinThemes }
export const fontFamilies = [...new Set([...builtinThemes.flatMap(t => Object.values(t.fonts)), 'Inter', 'Instrument Sans', 'Space Grotesk', 'Nunito', 'Lora', 'Fraunces', 'Commit Mono'])].sort()
/** The families that come from Google Fonts: not the device's own, not the bundled mono. */
export const webFonts = (families: string[]) => [...new Set(families)].filter(f => f !== systemFont && f !== 'IBM Plex Mono')
export const validName = (v: string) => [...v].length >= 1 && [...v].length <= 40 && !!v.trim() && /^[\p{L}\p{N} _-]+$/u.test(v)
/** Where a new theme goes. One theme for both light and dark is replaced on both,
 *  or a device on the other half keeps showing the old one; a split choice keeps its other half. */
export function themeSlots(a: Pick<Appearance, 'light_theme' | 'dark_theme'>, id: string, half: 'light'|'dark') {
  if (a.light_theme === a.dark_theme) return { light_theme: id, dark_theme: id }
  return half === 'light' ? { light_theme: id } : { dark_theme: id }
}
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
  get half() { return this.editingHalf ?? appearanceHalf(this.appearance, this.systemLight) }
  /** The phone's theme editor shows the half it edits on the whole page, as there is no app behind it. */
  private editingHalf = $state<'light'|'dark'|null>(null)
  showHalf(half: 'light'|'dark'|null) { if (half !== this.editingHalf) { this.editingHalf = half; this.apply() } }
  constructor() {
    this.apply()
    matchMedia('(prefers-color-scheme: light)').addEventListener('change', e => { this.systemLight = e.matches; this.apply() })
    // GIF wallpapers hold still while the system asks for less motion.
    matchMedia('(prefers-reduced-motion: reduce)').addEventListener('change', () => this.apply())
  }
  receive(a: Appearance, force = false) {
    if (this.pending && !force) return
    this.appearance = a
    try { localStorage.setItem(appearanceCacheKey(), JSON.stringify(a)) } catch { /* cache is optional */ }
    this.apply()
  }
  apply() { applyTheme(this.active, this.half, this.appearance); void this.loadFonts(this.active.fonts) }
  async loadFonts(fonts: ThemeFonts) {
    const families = webFonts(Object.values(fonts))
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
      for (const [role, family] of Object.entries(fonts)) if (webFonts([family]).length) document.documentElement.style.setProperty(`--${role}`, role === 'mono' ? '"Den Terminal Mono", monospace' : fontStack(systemFont, 'body'))
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
  /** A preset id, a `#rrggbb`, or null for each theme's own accent. */
  accent(choice: string | null) { return this.save({ ...this.appearance, accent: choice }, true) }
  /** What sits before a room's name in lists: its icon, a #, or nothing. */
  roomIcons(mode: NonNullable<Appearance['room_icons']>) { return this.save({ ...this.appearance, room_icons: mode }, true) }
  /** How messages are drawn, and whose bubbles carry a person's colour. */
  bubbles(patch: Pick<Appearance, 'bubble_shape'> | Pick<Appearance, 'bubble_colors'>) { return this.save({ ...this.appearance, ...patch }, true) }
  /** How pictures are drawn, and how being online shows on them. */
  avatars(patch: Pick<Appearance, 'avatar_shape'> | Pick<Appearance, 'presence_style'>) {
    for (const field of Object.keys(patch) as (keyof typeof patch)[]) forgetOldAvatarChoice(localStorage, field)
    return this.save({ ...this.appearance, ...patch }, true)
  }
  contrast(percent: number) { return this.save({ ...this.appearance, contrast: percent }, true) }
  background(b: Appearance['background']) { return this.save({ ...this.appearance, background: b ?? null }, true) }
  /** The sidebar's own wallpaper. A main one limited to the sidebar would then show
   *  nowhere, so it moves back to everywhere else. */
  sidebarBackground(b: Appearance['sidebar_background']) {
    const main = this.appearance.background
    const background = b && main?.scope === 'sidebar' ? { ...main, scope: 'app' as const } : main ?? null
    return this.save({ ...this.appearance, background, sidebar_background: b ?? null }, true)
  }
  mode(mode: Appearance['mode']) { return this.save({...this.appearance,mode},true) }
  preview(t: Theme) { this.draft = t; this.apply() }
  reset() { this.draft = null; this.apply() }
  async add(t: Theme) {
    const custom = [...this.appearance.custom_themes, validateTheme(t)]
    if (custom.length > 12 || new TextEncoder().encode(JSON.stringify(custom)).length > 16384) throw Error('Use at most 12 custom themes and 16 KiB total.')
    const saved = await this.save({ ...this.appearance, custom_themes: custom, ...themeSlots(this.appearance, t.id, this.half) })
    if (!saved) throw Error(this.error)
  }
  /** Change one of your own themes in place, so the change reaches your other devices. */
  update(t: Theme) {
    const theme = validateTheme(t)
    return this.save({ ...this.appearance, custom_themes: this.appearance.custom_themes.map(c => c.id === theme.id ? theme : c) }, true)
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
