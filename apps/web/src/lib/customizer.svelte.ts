// The live customizer: Appearance opens as a panel over the real app, so every
// change shows where it matters instead of in a settings page. Changes save as
// they happen; "Undo changes" returns to how things looked when it opened.
import { themes } from './theme.svelte'
import { store } from './store.svelte'
import type { Appearance } from './types'

type Snapshot = { appearance: Appearance; avatarShape: typeof store.layout.avatarShape; presence: typeof store.layout.presence }
const copy = <T>(v: T): T => JSON.parse(JSON.stringify(v)) as T

class Customizer {
  open = $state(false)
  private start = $state<Snapshot | null>(null)
  /** Where "Settings › Appearance" returns you: the last room or page outside Settings. */
  lastPlace = '/'

  show() {
    if (!this.open) this.start = { appearance: copy(themes.appearance), avatarShape: store.layout.avatarShape, presence: store.layout.presence }
    this.open = true
  }
  close() { this.open = false; this.start = null; themes.reset() }

  get changed() {
    const s = this.start
    if (!s) return false
    return JSON.stringify(s.appearance) !== JSON.stringify(themes.appearance) || s.avatarShape !== store.layout.avatarShape || s.presence !== store.layout.presence
  }

  /** Back to how it looked on opening. Themes made or imported meanwhile are kept. */
  undo() {
    const s = this.start
    if (!s) return
    themes.reset()
    void themes.save({ ...copy(s.appearance), custom_themes: themes.appearance.custom_themes })
    store.saveLayout({ avatarShape: s.avatarShape, presence: s.presence })
  }
}
export const customizer = new Customizer()
