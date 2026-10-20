// The Settings sections and every row "Find a setting" can land on. Kept apart
// from the components so a test can check that what an admin sees stays hidden
// from everyone else.

export type SectionId =
  | 'profile' | 'appearance' | 'notifications' | 'sounds' | 'voice' | 'spotify' | 'layout' | 'desktop' | 'account'
  | 'members' | 'invites' | 'rooms' | 'plugins'
  | 'machines' | 'access' | 'keys' | 'agents' | 'developer'
export type Section = { id: SectionId; label: string; icon: string; group: 'you' | 'app' | 'account' | 'server' | 'advanced' | 'developer' }

/**
 * In list order. `server` sections are for admins; Desktop app only shows outside the desktop app.
 * The iPhone app has no Layout, Machines or Desktop app, and keeps Access, API keys and Agents behind Developer.
 */
export const SECTIONS: Section[] = [
  { id: 'profile', label: 'Profile', icon: 'user', group: 'you' },
  { id: 'appearance', label: 'Appearance', icon: 'palette', group: 'app' },
  { id: 'notifications', label: 'Notifications', icon: 'bell', group: 'app' },
  { id: 'sounds', label: 'Sounds', icon: 'sound', group: 'app' },
  { id: 'voice', label: 'Voice', icon: 'mic', group: 'app' },
  { id: 'spotify', label: 'Spotify', icon: 'music', group: 'app' },
  { id: 'layout', label: 'Layout', icon: 'panel', group: 'app' },
  { id: 'desktop', label: 'Desktop app', icon: 'monitor', group: 'app' },
  { id: 'account', label: 'Account', icon: 'lock', group: 'account' },
  { id: 'members', label: 'Members', icon: 'people', group: 'server' },
  { id: 'invites', label: 'Invites', icon: 'link', group: 'server' },
  { id: 'rooms', label: 'Rooms', icon: 'hash', group: 'server' },
  { id: 'plugins', label: 'Extras', icon: 'puzzle', group: 'server' },
  { id: 'machines', label: 'Machines', icon: 'terminal', group: 'advanced' },
  { id: 'access', label: 'Access', icon: 'shield', group: 'advanced' },
  { id: 'keys', label: 'API keys', icon: 'key', group: 'advanced' },
  { id: 'agents', label: 'Agents', icon: 'bot', group: 'advanced' },
  { id: 'developer', label: 'Developer', icon: 'terminal', group: 'developer' },
]

const COMPUTER_ONLY: SectionId[] = ['layout', 'machines', 'desktop']

/** The sections this person can open. `phoneApp`: inside Den Shell, the iPhone app. */
export function visibleSections(admin: boolean, desktopApp: boolean, phoneApp = false): Section[] {
  return SECTIONS.filter((s) => (admin || s.group !== 'server') && !(desktopApp && s.id === 'desktop')
    && (phoneApp ? !COMPUTER_ONLY.includes(s.id) : s.id !== 'developer'))
}

/**
 * One row a search can land on: `find` matches the row's `data-find`. `admin` rows show to admins only.
 * `computer` rows aren't in the iPhone app: its calls are native and voice only, so the browser's devices don't reach
 * them, and it can't see what you're playing.
 */
export type Entry = { section: SectionId; find: string; label: string; words?: string; admin?: boolean; computer?: boolean }

export const ENTRIES: Entry[] = [
  { section: 'profile', find: 'avatar', label: 'Profile picture', words: 'avatar photo picture gif upload' },
  { section: 'profile', find: 'banner', label: 'Banner', words: 'header cover picture' },
  { section: 'profile', find: 'name', label: 'Name', words: 'display name nickname' },
  { section: 'profile', find: 'bio', label: 'Bio', words: 'about me description' },
  { section: 'profile', find: 'colour', label: 'Profile colour', words: 'color bubble accent' },
  { section: 'profile', find: 'status', label: 'Status', words: 'emoji what you are up to' },
  { section: 'profile', find: 'clear', label: 'Clear status after', words: 'expire status' },
  { section: 'profile', find: 'activity', label: 'Share what I’m playing', words: 'activity game app playing', computer: true },
  { section: 'profile', find: 'music', label: 'Show my music', words: 'spotify listening' },

  { section: 'appearance', find: 'mode', label: 'Light, dark or system', words: 'dark mode light mode scheme night' },
  { section: 'appearance', find: 'themes', label: 'Themes', words: 'theme colours colors palette neutral den moss tide ember iris paper slate terminal' },
  { section: 'appearance', find: 'custom', label: 'Create or import a theme', words: 'custom theme editor export import own colours' },
  { section: 'appearance', find: 'accent', label: 'Accent colour', words: 'color accent blue indigo green graphite' },
  { section: 'appearance', find: 'shape', label: 'Corners', words: 'radius rounded sharp soft round' },
  { section: 'appearance', find: 'density', label: 'Density', words: 'compact comfortable spacing' },
  { section: 'appearance', find: 'contrast', label: 'Contrast', words: 'readability text borders' },
  { section: 'appearance', find: 'avatar-shape', label: 'Avatar shape', words: 'picture round square' },
  { section: 'appearance', find: 'presence', label: 'Online indicator', words: 'dot ring presence here' },
  { section: 'appearance', find: 'room-icons', label: 'Room icons', words: 'hash # sign icons nothing' },
  { section: 'appearance', find: 'bubble-shape', label: 'Bubble shape', words: 'message bubbles' },
  { section: 'appearance', find: 'bubble-colours', label: 'Bubble colours', words: 'message colors' },
  { section: 'appearance', find: 'wallpaper', label: 'Wallpaper', words: 'background picture image gif blur dim saturation' },
  { section: 'appearance', find: 'wallpaper', label: 'Sidebar wallpaper', words: 'background room list' },
  { section: 'appearance', find: 'fonts', label: 'Fonts', words: 'typography typeface display body monospace google font' },

  { section: 'notifications', find: 'permission', label: 'Desktop notifications', words: 'allow browser push turn on' },
  { section: 'notifications', find: 'mentions', label: 'When someone mentions me', words: 'mention notify' },
  { section: 'notifications', find: 'dms', label: 'Direct messages', words: 'dm notify' },
  { section: 'notifications', find: 'follow', label: 'Rooms you follow', words: 'subscribe follow room notify' },

  { section: 'sounds', find: 'pack', label: 'Sound pack', words: 'sounds notification sounds' },
  { section: 'sounds', find: 'volume', label: 'Master volume', words: 'loud quiet volume' },
  { section: 'sounds', find: 'events', label: 'Sound for each moment', words: 'event message join leave call tone replace silent mute none' },
  { section: 'sounds', find: 'keep', label: 'Save your sounds as a pack', words: 'import export share pack' },
  { section: 'sounds', find: 'server-pack', label: 'Server default sounds', words: 'everyone new members', admin: true },

  { section: 'voice', find: 'input-mode', label: 'Voice activity or push to talk', words: 'ptt push to talk key mic microphone' },
  { section: 'voice', find: 'microphone', label: 'Microphone', words: 'mic input device', computer: true },
  { section: 'voice', find: 'gain', label: 'Input gain', words: 'mic volume level meter', computer: true },
  { section: 'voice', find: 'processing', label: 'Echo cancellation and noise suppression', words: 'auto gain noise echo', computer: true },
  { section: 'voice', find: 'camera', label: 'Camera', words: 'video webcam device preview', computer: true },
  { section: 'voice', find: 'quality', label: 'Resolution and frame rate', words: 'fps 720p 1080p quality', computer: true },
  { section: 'voice', find: 'mirror', label: 'Mirror my preview', words: 'flip camera', computer: true },
  { section: 'voice', find: 'background', label: 'Camera background blur', words: 'blur background', computer: true },
  { section: 'voice', find: 'speaker', label: 'Speaker', words: 'output headphones audio', computer: true },
  { section: 'voice', find: 'camera-on', label: 'Join with camera on', words: 'video call start', computer: true },
  { section: 'voice', find: 'dictation', label: 'Dictation', words: 'voice typing speech punctuation model' },

  { section: 'spotify', find: 'connect', label: 'Connect Spotify', words: 'jam music account' },
  { section: 'spotify', find: 'share', label: 'Show what I’m listening to', words: 'spotify listening music share' },

  { section: 'layout', find: 'sidebar', label: 'Show the room list', words: 'sidebar hide' },
  { section: 'layout', find: 'members', label: 'Show people', words: 'member list hide' },
  { section: 'layout', find: 'shortcuts', label: 'Keyboard shortcuts', words: 'ctrl k keys' },

  { section: 'desktop', find: 'download', label: 'Download the desktop app', words: 'windows mac linux install' },
  { section: 'account', find: 'logout', label: 'Log out', words: 'sign out account' },

  { section: 'members', find: 'people', label: 'Rename or remove a member', words: 'member picture banner remove rename', admin: true },
  { section: 'invites', find: 'new', label: 'New invite link', words: 'invite link uses expires', admin: true },
  { section: 'rooms', find: 'server-name', label: 'Server name', words: 'name instance', admin: true },
  { section: 'rooms', find: 'add-room', label: 'Add a room', words: 'channel create new room', admin: true },
  { section: 'rooms', find: 'add-category', label: 'Add a category', words: 'group category', admin: true },
  { section: 'rooms', find: 'rooms', label: 'Room icons and deleting rooms', words: 'room icon picker delete channel', admin: true },
  { section: 'plugins', find: 'canvas', label: 'Canvas', words: 'tldraw drawing board plugin plugins extras', admin: true },

  { section: 'machines', find: 'add', label: 'Add a machine', words: 'terminal enroll computer host' },
  { section: 'machines', find: 'recording', label: 'Record terminal sessions', words: 'recording replay terminal' },
  { section: 'access', find: 'grants', label: 'Standing grants', words: 'access revoke terminal' },
  { section: 'access', find: 'log', label: 'Recent access', words: 'audit log history' },
  { section: 'keys', find: 'new', label: 'New API key', words: 'token cli script relay' },
  { section: 'agents', find: 'new', label: 'New agent', words: 'bot hermes token' },
]

export type Hit = { section: Section; entry?: Entry }

/**
 * What a search for `q` finds among the sections someone can open: sections by
 * name first, then rows whose name starts with a word of the query, then the rest.
 */
export function findSettings(q: string, sections: Section[], admin: boolean, phoneApp = false, limit = 12): Hit[] {
  const words = q.toLowerCase().split(/\s+/).filter(Boolean)
  if (!words.length) return []
  const open = new Map(sections.map((s) => [s.id, s]))
  const hits: (Hit & { rank: number })[] = []
  for (const s of sections) {
    const label = s.label.toLowerCase()
    if (words.every((w) => label.includes(w))) hits.push({ section: s, rank: label.startsWith(words[0]!) ? 0 : 1 })
  }
  for (const e of ENTRIES) {
    const s = open.get(e.section)
    if (!s || (e.admin && !admin) || (e.computer && phoneApp)) continue
    const label = e.label.toLowerCase(), text = `${label} ${e.words ?? ''} ${s.label.toLowerCase()}`
    if (!words.every((w) => text.includes(w))) continue
    const start = label.split(/[^a-z0-9#]+/).some((part) => part.startsWith(words[0]!))
    hits.push({ section: s, entry: e, rank: start ? 2 : 3 })
  }
  return hits.sort((a, b) => a.rank - b.rank).slice(0, limit).map(({ section, entry }) => ({ section, entry }))
}

/** Scrolls the row a search found into view inside `root` and marks it for a moment. */
export function reveal(root: ParentNode, find: string) {
  const row = root.querySelector<HTMLElement>(`[data-find="${CSS.escape(find)}"]`)
  if (!row) return false
  // A tall target (a whole group) starts at the top, so its first control is in view.
  let scroller = row.parentElement
  while (scroller && !/auto|scroll/.test(getComputedStyle(scroller).overflowY)) scroller = scroller.parentElement
  const tall = row.getBoundingClientRect().height > (scroller?.clientHeight ?? innerHeight) / 2
  row.scrollIntoView({ block: tall ? 'start' : 'center', behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' })
  row.classList.remove('found'); void row.offsetWidth; row.classList.add('found')
  // Keyboards land on the row's first control, or the row when it is one. A phone lands
  // on the row (or its own button), so no text field opens its keyboard.
  const controls = 'input:not([type=hidden]):not(:disabled), select:not(:disabled), textarea:not(:disabled), button:not(:disabled), a[href]'
  const control = row.matches(controls) ? row : matchMedia('(pointer: coarse)').matches ? null : row.querySelector<HTMLElement>(controls)
  if (control) control.focus({ preventScroll: true })
  else { if (!row.hasAttribute('tabindex')) row.tabIndex = -1; row.focus({ preventScroll: true }) }
  return true
}
