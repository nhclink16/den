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
type Patch = Partial<Appearance>
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T
class Themes {
  /** What shows: the server's newest copy with this device's saves still on their way on top. */
  appearance = $state<Appearance>(cachedAppearance())
  draft = $state<Theme | null>(null)
  error = $state('')
  saving = $state(false)
  /** Whose appearance this is: one account on one server. The store fills in the account. */
  owner = () => activeOrigin()
  private systemLight = $state(matchMedia('(prefers-color-scheme: light)').matches)
  private fontRun = 0
  private stored = copy(this.appearance)
  // Each save sends only the fields it changes, and the server keeps the rest, so a
  // save never writes an old copy of another device's change back.
  private queue: Promise<unknown> = Promise.resolve()
  private jobs: { owner: string; patch: Patch }[] = []
  /** Copies taken from the server. Without a revision on each copy, a copy that comes in while
   *  another is on its way leaves its order unknown, so the later one is set aside (a save asks again). */
  private taken = 0
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
  /** Taken before a sync asks for the server's copy, and passed back with it. */
  mark() { return this.taken }
  /** The server's copy for the account showing now: from the socket or a server switch, or from a
   *  sync with its `mark()`. A sync's copy is older than anything taken since it asked: false, set aside. */
  receive(a: Appearance, asked?: number) {
    if (asked !== undefined && asked !== this.taken) return false
    this.take(a); this.show()
    return true
  }
  /** The newest copy from the server, for the store to keep when it stops being the one showing. */
  latest() { return copy(this.stored) }
  private take(a: Appearance) { this.stored = copy(a); this.taken++ }
  private show() {
    const owner = this.owner()
    this.appearance = Object.assign(copy(this.stored), ...this.jobs.filter(j => j.owner === owner).map(j => copy(j.patch)))
    try { localStorage.setItem(appearanceCacheKey(), JSON.stringify(this.appearance)) } catch { /* cache is optional */ }
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
  /** Saves the fields in `patch`, one save at a time. A save waiting its turn is dropped
   *  if the account or server changed meanwhile, so it never reaches the wrong one. */
  save(patch: Patch, keepDraft = false) {
    const job = { owner: this.owner(), patch: copy(patch) }, api = apiFor(activeOrigin())
    this.error = ''; if (!keepDraft) this.draft = null
    this.jobs.push(job); this.saving = true; this.show()
    const result = this.queue.catch(() => {}).then(async () => {
      if (this.owner() !== job.owner) return false
      const asked = this.taken
      try {
        const saved = await api.put<Appearance>('/users/me/appearance', job.patch)
        if (this.owner() === job.owner && this.taken === asked) this.take(saved)
        // Another copy came in meanwhile, older or newer: one asked for now is newer than both.
        else if (this.owner() === job.owner) {
          const again = this.taken
          try { this.receive(await api.get<Appearance>('/users/me/appearance'), again) } catch { /* the next sync brings it */ }
        }
        return true
      } catch (e) {
        if (this.owner() === job.owner) this.error = (e as Error).message
        return false
      }
    }).finally(() => {
      this.jobs.splice(this.jobs.indexOf(job), 1); this.saving = this.jobs.length > 0
      this.show()
    })
    this.queue = result
    return result
  }
  /** Assign a theme to one appearance. Light and dark are independent. */
  select(t: Theme, half: 'light'|'dark' = this.half) {
    return this.save({ [half === 'light' ? 'light_theme' : 'dark_theme']: t.id })
  }
  /** A preset id, a `#rrggbb`, or null for each theme's own accent. */
  accent(choice: string | null) { return this.save({ accent: choice }, true) }
  /** What sits before a room's name in lists: its icon, a #, or nothing. */
  roomIcons(mode: NonNullable<Appearance['room_icons']>) { return this.save({ room_icons: mode }, true) }
  /** How messages are drawn, and whose bubbles carry a person's colour. */
  bubbles(patch: Pick<Appearance, 'bubble_shape'> | Pick<Appearance, 'bubble_colors'>) { return this.save(patch, true) }
  /** How pictures are drawn, and how being online shows on them. */
  avatars(patch: Pick<Appearance, 'avatar_shape'> | Pick<Appearance, 'presence_style'>) {
    for (const field of Object.keys(patch) as (keyof typeof patch)[]) forgetOldAvatarChoice(localStorage, field)
    return this.save(patch, true)
  }
  contrast(percent: number) { return this.save({ contrast: percent }, true) }
  background(b: Appearance['background']) { return this.save({ background: b ?? null }, true) }
  /** The sidebar's own wallpaper. A main one limited to the sidebar would then show
   *  nowhere, so it moves back to everywhere else. */
  sidebarBackground(b: Appearance['sidebar_background']) {
    const main = this.appearance.background
    const background = b && main?.scope === 'sidebar' ? { background: { ...main, scope: 'app' as const } } : {}
    return this.save({ ...background, sidebar_background: b ?? null }, true)
  }
  mode(mode: Appearance['mode']) { return this.save({ mode }, true) }
  preview(t: Theme) { this.draft = t; this.apply() }
  reset() { this.draft = null; this.apply() }
  async add(t: Theme) {
    const custom = [...this.appearance.custom_themes, validateTheme(t)]
    if (custom.length > 12 || new TextEncoder().encode(JSON.stringify(custom)).length > 16384) throw Error('Use at most 12 custom themes and 16 KiB total.')
    const saved = await this.save({ custom_themes: custom, ...themeSlots(this.appearance, t.id, this.half) })
    if (!saved) throw Error(this.error)
  }
  /** Change one of your own themes in place, so the change reaches your other devices. */
  update(t: Theme) {
    const theme = validateTheme(t)
    return this.save({ custom_themes: this.appearance.custom_themes.map(c => c.id === theme.id ? theme : c) }, true)
  }
  async remove(id: string) {
    const a = this.appearance
    await this.save({
      custom_themes: a.custom_themes.filter(t => t.id !== id),
      ...(a.light_theme === id && { light_theme: defaultAppearance.light_theme }),
      ...(a.dark_theme === id && { dark_theme: defaultAppearance.dark_theme }),
    })
  }
}
export const themes = new Themes()
