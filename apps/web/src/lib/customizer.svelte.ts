// The live customizer: on a wide screen Appearance opens as a panel over the real
// app, so every change shows where it matters instead of in a settings page. Changes save as
// they happen; "Undo changes" returns to how things looked when it opened.
import { themes } from './theme.svelte'
import type { Appearance } from './types'

type Snapshot = { owner: string; appearance: Appearance }
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T
/** Whose appearance is showing: a snapshot belongs to one account on one server. */
const owner = () => themes.owner()

class Customizer {
  open = $state(false)
  private start = $state<Snapshot | null>(null)
  /** Your own themes changed in place since opening (corners, density, fonts). */
  private edited = new Set<string>()
  /** Where "Settings › Appearance" returns you: the last room or page outside Settings. */
  lastPlace = '/'

  /** A row for the panel to scroll to once it opens: settings search found it. */
  find = $state<string | null>(null)

  show(find?: string) {
    this.begin()
    this.find = find ?? null
    this.open = true
  }
  close() { this.open = false; this.end() }
  /** Remembers how things look now, for Undo. The phone's Appearance page uses this without the panel. */
  begin() {
    if (!this.start) this.start = { owner: owner(), appearance: copy(themes.appearance) }
  }
  end() { this.start = null; this.edited.clear(); themes.reset() }
  /** Undo puts these themes back, and leaves the rest as they are now (another device may have changed one). */
  editedTheme(id: string) { if (this.start?.owner === owner()) this.edited.add(id) }

  get changed() {
    const s = this.start
    if (!s || s.owner !== owner()) return false
    return JSON.stringify(s.appearance) !== JSON.stringify(themes.appearance)
  }

  /** Back to how it looked on opening. Themes made or imported meanwhile are kept;
   *  your own themes changed here in place (corners, density, fonts) go back too. */
  undo() {
    const s = this.start
    // Opened on another server or account: its snapshot is not this one's to restore.
    if (!s || s.owner !== owner()) return
    themes.reset()
    const now = themes.appearance
    // Only what differs goes back, and the custom themes only when one is put back, so a
    // change another device makes meanwhile to something else stays.
    const patch = Object.fromEntries(Object.entries(copy(s.appearance)).filter(([k, v]) => k !== 'custom_themes' && JSON.stringify(v) !== JSON.stringify(now[k as keyof Appearance])))
    if (now.custom_themes.some(t => this.edited.has(t.id) && s.appearance.custom_themes.some(o => o.id === t.id)))
      patch.custom_themes = now.custom_themes.map(t => (this.edited.has(t.id) && s.appearance.custom_themes.find(o => o.id === t.id)) || t)
    void themes.save(patch)
  }
}
export const customizer = new Customizer()
