// The live customizer: on a wide screen Appearance opens as a panel over the real
// app, so every change shows where it matters instead of in a settings page. Changes save as
// they happen; "Undo changes" returns to how things looked when it opened.
import { themes } from './theme.svelte'
import { activeOrigin } from './native'
import { store } from './store.svelte'
import type { Appearance } from './types'

type Snapshot = { origin: string; appearance: Appearance; avatarShape: typeof store.layout.avatarShape; presence: typeof store.layout.presence }
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T

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
    if (!this.start) this.start = { origin: activeOrigin(), appearance: copy(themes.appearance), avatarShape: store.layout.avatarShape, presence: store.layout.presence }
  }
  end() { this.start = null; this.edited.clear(); themes.reset() }
  /** Undo puts these themes back, and leaves the rest as they are now (another device may have changed one). */
  editedTheme(id: string) { if (this.start) this.edited.add(id) }

  get changed() {
    const s = this.start
    if (!s || s.origin !== activeOrigin()) return false
    return JSON.stringify(s.appearance) !== JSON.stringify(themes.appearance) || s.avatarShape !== store.layout.avatarShape || s.presence !== store.layout.presence
  }

  /** Back to how it looked on opening. Themes made or imported meanwhile are kept;
   *  your own themes changed here in place (corners, density, fonts) go back too. */
  undo() {
    const s = this.start
    // Opened on another server: its snapshot is not this account's to restore.
    if (!s || s.origin !== activeOrigin()) return
    themes.reset()
    const custom = themes.appearance.custom_themes.map(t => (this.edited.has(t.id) && s.appearance.custom_themes.find(o => o.id === t.id)) || t)
    void themes.save({ ...copy(s.appearance), custom_themes: copy(custom) })
    store.saveLayout({ avatarShape: s.avatarShape, presence: s.presence })
  }
}
export const customizer = new Customizer()
