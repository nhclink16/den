import { parse, type Route } from './routes'
export type { Route }
// Tiny history router. The URL shapes themselves live in routes.ts so they can
// be pinned by tests without a browser.
//
// Entries carry their own parent in history.state rather than being counted.
// A counter cannot tell you WHICH entry is behind you — forward navigation and
// popstate both move through it — and "Back to room" has to mean the room this
// conversation was opened from, or the label is a lie.
type Den = { parent?: string }
const den = (): Den => (history.state?.den ?? {}) as Den

let route = $state<Route>(parse(location.pathname, location.search))

export const router = {
  get route() { return route },
  /// The room this entry was opened from, when it records one.
  get parent() { return den().parent },
  /**
   * @param parent the route this entry was opened from.
   *
   * Only a REPLACEMENT inherits the current entry's parent, because a
   * replacement is the same conceptual entry — switching conversations in place
   * must not rewrite where Back leads. An ordinary push starts with no parent
   * unless the caller names its real immediate one; inheriting there gave an
   * unrelated Inbox or search entry a conversation's parent, and a later Back
   * to room walked into it.
   */
  go(path: string, replace = false, parent?: string) {
    if (location.pathname + location.search === path && !replace) return
    const carry: Den = parent !== undefined ? { parent } : replace ? den() : {}
    history[replace ? 'replaceState' : 'pushState']({ den: carry }, '', path)
    route = parse(location.pathname, location.search)
  },
  /// Ordinary browser back. Callers check `parent` first to decide whether back
  /// is the right thing at all.
  back() { history.back() },
}

window.addEventListener('popstate', () => { route = parse(location.pathname, location.search) })
