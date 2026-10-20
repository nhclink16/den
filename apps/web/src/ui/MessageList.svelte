<script module lang="ts">
  import type { Message as SourceMessage } from '../lib/types'
  // Everything conversation-specific arrives as a source, so the room feed and a
  // thread panel are the same component reading different things. `prefix` keeps
  // their DOM ids apart: a panel shows the same root message the room does, and
  // a deep link must not scroll to the wrong copy of it.
  export type Source = {
    list: SourceMessage[]
    lastRead: string
    exhausted: boolean
    loadingOlder: boolean
    older: () => Promise<void>
    /// The newest message actually on screen, reported so the owner can decide
    /// whether this conversation is visible enough to acknowledge.
    onread?: (displayed: string) => void
    /// A message to reveal rather than following the tail.
    target?: string
    prefix?: string
    /// What an empty conversation acknowledges through: its root.
    watermark?: string
    /// What this source IS, so switching between two conversations resets the
    /// unread divider and the scroll mode. Every thread panel uses the same DOM
    /// prefix, so the prefix cannot tell them apart.
    identity: string
    /// Distinguishes a window from the latest list of the same conversation.
    identityExtra?: string
    /// Stowed out of sight behind a panel: laid out at a width nobody sees, so
    /// nothing here counts as having reached the newest message.
    hidden?: boolean
  }
</script>

<script lang="ts">
  import { tick, type Snippet } from 'svelte'
  import { store, instances } from '../lib/store.svelte'
  import type { Channel, Message } from '../lib/types'
  import { dayLabel, shortTime } from '../lib/time'
  import { places } from '../lib/bubbles'
  import MessageItem from './MessageItem.svelte'
  import Icon from './Icon.svelte'
  import { roomName } from '../lib/room-list'
  import { themes } from '../lib/theme.svelte'
  import RoomIcon from './RoomIcon.svelte'
  import Avatar from './Avatar.svelte'
  import { profileCard } from '../lib/people.svelte'
  import { atBottom, landing } from '../lib/landing'

  // On a phone the list runs under the top bar and the message box, which pad it by their heights through
  // `--list-top` and `--list-bottom`. `below` says whether anything is out of sight under the message box,
  // so it can frost. `lead` scrolls above the first message (a thread's root, on a phone), and gets the
  // list's media-ready handler, so a picture loading late in it keeps the view in place too.
  let { channel, source, onreply, lead, below = $bindable(false) }: {
    channel: Channel; source: Source; onreply: (m: Message) => void; lead?: Snippet<[() => void]>; below?: boolean
  } = $props()
  let el: HTMLDivElement
  // How much of the list the bars cover, top and bottom: the scroll padding, which is just the bars.
  function covered() {
    const css = getComputedStyle(el)
    return { top: parseFloat(css.scrollPaddingTop) || 0, bottom: parseFloat(css.scrollPaddingBottom) || 0 }
  }
  const list = $derived(source.list)
  const prefix = $derived(source.prefix ?? 'm')
  // Where "new" starts: the read marker as it was when this conversation opened.
  // Not reactive on purpose.
  // svelte-ignore state_referenced_locally
  let openedAt = $state(source.lastRead)
  const firstUnread = $derived(list.find((m) => m.id > openedAt && m.author_id !== store.me?.id)?.id)
  // Following the tail, unless this view was opened on an older target. Set from
  // the start rather than corrected later: the read effect runs before any
  // correction could, and would acknowledge a tail nobody is looking at.
  // svelte-ignore state_referenced_locally
  let stickToBottom = $state(!source.target)
  // Nothing follows the tail or acknowledges it until the view has decided where
  // to open: at the bottom, or at the "new" line when that would be off screen.
  // A target decides for itself.
  // svelte-ignore state_referenced_locally
  let landed = $state(!!source.target)
  // svelte-ignore state_referenced_locally
  let shownKey = $state(`${source.identity}:${source.identityExtra ?? ''}:${source.target ?? ''}`)
  // svelte-ignore state_referenced_locally
  let shown = { identity: source.identity, target: source.target }
  $effect(() => {
    const key = `${source.identity}:${source.identityExtra ?? ''}:${source.target ?? ''}`
    if (key === shownKey) return
    // Leaving a target for the same conversation is "Jump to newest": the
    // newest message, not the unread line.
    const toNewest = shown.identity === source.identity && !!shown.target && !source.target
    shownKey = key
    shown = { identity: source.identity, target: source.target }
    openedAt = source.lastRead
    stickToBottom = !source.target
    landed = !!source.target || toNewest
    touched = false
    gaveUp = false
  })

  // Where the view opens: the "new" line near the top when it would be off
  // screen, otherwise the bottom. A conversation never read has no place to
  // keep, so it opens at the bottom as before. A hidden view never counts as
  // read from there; `reached` decides once it is shown.
  function land() {
    const line = openedAt ? el.querySelector<HTMLElement>(':scope > .new') : null
    // Measured from where the list shows, below the bar.
    const at = line ? line.getBoundingClientRect().top - el.getBoundingClientRect().top + el.scrollTop - covered().top : undefined
    const { top, bottom } = landing(at, el.scrollHeight, el.clientHeight)
    el.scrollTop = top
    stickToBottom = bottom && !source.hidden
    far = el.scrollHeight - top - el.clientHeight > 400
  }
  // Until the reader scrolls, media that settles late above the bottom moves the
  // view back to the "new" line rather than wherever the growth pushed it.
  let touched = false
  // Set when fetching back to the first unread message failed: open anyway.
  let gaveUp = $state(false)

  // Open once the first page is here (or known to be empty), and once the first
  // unread message is in it: older pages are fetched for that, up to 250
  // messages so a room left for weeks does not pull its whole history first.
  // Catching up starts where the reader left off; the tail is acknowledged only
  // once they reach it.
  $effect(() => {
    if (landed || (!list.length && !source.exhausted)) return
    if (openedAt && list.length && list.length < 250 && list[0].id > openedAt && !source.exhausted && !gaveUp) {
      const key = shownKey
      if (!source.loadingOlder) source.older().catch(() => { if (shownKey === key) gaveUp = true })
      return
    }
    void tick().then(() => {
      if (landed || !el) return
      land()
      landed = true
    })
  })

  // Keep the newest message in view unless the reader scrolled up, or is looking
  // at an older target they asked for.
  $effect(() => {
    void list.length
    if (landed && stickToBottom && !source.target) tick().then(() => el?.scrollTo({ top: el.scrollHeight }))
  })
  // A message sent from here brings you down to it wherever you were: posting is
  // taken as having caught up. Its socket echo can arrive before the send
  // returns, so this watches both rather than the order they come in.
  let followed = store.lastSent
  $effect(() => {
    const sent = store.lastSent
    if (!landed || source.target || sent === followed || !list.some((m) => m.id === sent)) return
    followed = sent
    stickToBottom = true
  })

  // A view that grows until the tail is on screen, or is shown again with the
  // tail on screen, has reached it though nothing scrolled to say so: a bigger
  // window, or a panel closing beside it.
  // The Jump to latest pill follows the same layout changes.
  // What's measured is the part between the bars, so the message box growing over the list counts the same as
  // the list getting shorter. A hidden view keeps the height it was last seen at, to come back to the same place.
  let lastHeight = 0
  function reached() {
    if (!el || source.hidden) return
    const bars = covered()
    const seen = el.clientHeight - bars.top - bars.bottom
    const lost = lastHeight ? lastHeight - seen : 0
    lastHeight = seen
    if (!landed) return
    // A view that got shorter (the keyboard came up, the composer grew) stays on the newest message, as iMessage does.
    if (stickToBottom && !source.target) el.scrollTop = el.scrollHeight
    // Anywhere else its bottom edge stays put, so what you were reading just above the composer stays in view.
    else if (lost) el.scrollTop += lost
    below = el.scrollHeight - el.scrollTop - el.clientHeight > 1
    far = el.scrollHeight - el.scrollTop - el.clientHeight > 400
    if (!stickToBottom && !source.target && atBottom(el.scrollHeight, el.scrollTop, el.clientHeight)) stickToBottom = true
  }
  $effect(() => {
    const ro = new ResizeObserver(reached)
    ro.observe(el)
    return () => ro.disconnect()
  })
  $effect(() => { if (!source.hidden) void tick().then(reached) })

  // Reveal a requested target. This depends on the LIST as well as the target:
  // the window it lives in usually arrives after this first runs, and an effect
  // that only watched the target would query a tick later, find nothing, and
  // never look again.
  let revealed = $state('')
  $effect(() => {
    const id = source.target
    void list.length
    if (!id) { revealed = ''; return }
    if (revealed === id) return
    if (!list.some((m) => m.id === id)) return
    revealed = id
    void tick().then(() => {
      el?.querySelector(`#${prefix}-${CSS.escape(id)}`)?.scrollIntoView({ block: 'center' })
      stickToBottom = false
    })
  })

  // Acknowledging is the owner's decision; this only reports what is displayed.
  // An empty conversation still has something displayed — its root — and the
  // brief wants that watermark, so a reply arriving between the load and the
  // acknowledgement stays unread instead of being swallowed.
  $effect(() => {
    if (!landed || !stickToBottom || source.hidden) return
    const tail = list.at(-1)?.id ?? source.watermark
    if (tail) source.onread?.(tail)
  })

  // Reading history while the room keeps talking: count what arrived below you.
  let far = $state(false)
  let seenLast = $state<string | undefined>()
  $effect(() => { if (!far) seenLast = list.at(-1)?.id })
  const arrived = $derived(far && seenLast ? list.filter((m) => m.id > seenLast! && m.author_id !== store.me?.id).length : 0)
  function mention(e: MouseEvent) {
    const tag = (e.target as HTMLElement).closest<HTMLElement>('.mention[data-user]')
    if (tag?.dataset.user) profileCard.open(tag.dataset.user, tag, instances.active)
  }
  function latest() {
    touched = true
    const smooth = !matchMedia('(prefers-reduced-motion: reduce)').matches
    el?.scrollTo({ top: el.scrollHeight, behavior: smooth ? 'smooth' : 'auto' })
  }

  function mediaReady() {
    void tick().then(() => {
      if (!landed || source.target || !el?.isConnected) return
      if (stickToBottom) el.scrollTo({ top: el.scrollHeight })
      else if (!touched) land()
    })
  }

  // While the reader scrolls history, the day they're in floats at the top
  // (touch screens: CSS hides it elsewhere), then fades once they stop.
  let day = $state(''), dayShown = $state(false)
  let dayTimer: ReturnType<typeof setTimeout> | undefined
  // Only for the reader's own scrolling (and its glide), not the keyboard nudging the list.
  let swiped = 0
  function showDay() {
    const top = el.getBoundingClientRect().top + covered().top
    let label = ''
    for (const t of el.querySelectorAll<HTMLElement>(':scope > .time')) {
      if (t.getBoundingClientRect().top > top) break
      label = t.dataset.day!
    }
    day = label || day
    dayShown = !!label
    clearTimeout(dayTimer)
    dayTimer = setTimeout(() => (dayShown = false), 1200)
  }

  async function onScroll() {
    const scroller = el
    if (!scroller) return
    if (Date.now() - swiped < 1500) showDay()
    const fromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
    // A hidden view scrolls only programmatically, at a width nobody sees (or because the bars it lost no longer
    // pad it), so it keeps whether it was following the newest message for when it's shown again.
    if (!source.hidden) stickToBottom = atBottom(el.scrollHeight, el.scrollTop, el.clientHeight)
    far = fromBottom > 400
    below = fromBottom > 1
    if (el.scrollTop < 120 && !source.exhausted && !source.loadingOlder) {
      const before = el.scrollHeight, key = shownKey
      await source.older()
      await tick()
      // The history request can outlive this view, and this conversation: a
      // thread panel reuses its scroller for the next thread.
      if (el === scroller && scroller.isConnected && shownKey === key) scroller.scrollTop += scroller.scrollHeight - before
    }
  }

  // Runs of one person's messages, and where the time shows between them.
  const placed = $derived(places(list, firstUnread))
  // A DM with one person opens on their picture rather than an icon.
  const dm = $derived(channel.kind === 'dm')
  const other = $derived(dm && (channel.member_ids?.length ?? 0) <= 2 ? channel.member_ids?.find((id) => id !== store.me?.id) ?? store.me?.id : undefined)
  // A DM with yourself: notes, not a conversation.
  const self = $derived(!!other && other === store.me?.id)
</script>

<div class="wrap">
<!-- Mentions are rendered HTML, so one delegated handler opens their profile cards. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (mentions are not focusable text; the person is reachable from their avatar and the people list) -->
<div class="list" class:thread={prefix === 't'} bind:this={el} onscroll={onScroll} onclick={mention} onwheel={() => { touched = true; swiped = Date.now() }} ontouchstart={() => (touched = true)} ontouchmove={() => (swiped = Date.now())} onpointerdown={() => (touched = true)} onkeydown={() => (touched = true)}>
  {@render lead?.(mediaReady)}
  <!-- A thread pins its root above the list, so only a room gets a start block. -->
  {#if source.exhausted && prefix === 'm'}
    <!-- Where a conversation begins. An empty one shows it in the middle of the screen. -->
    <div class="start" class:empty={!list.length}>
      {#if other}<span class="start-face" aria-hidden="true"><Avatar userId={other} size={72} presence={false} /></span>
      {:else}<div class="start-mark" aria-hidden="true"><RoomIcon {channel} size={28} bare /></div>{/if}
      <div class="display start-title">{dm ? store.title(channel) : `Welcome to ${roomName(channel, themes.appearance.room_icons)}`}</div>
      <div class="muted">{list.length ? 'This is where it started.' : !dm ? 'Nobody has said anything yet. Say the first thing.' : self ? 'Notes to yourself. Only you can see this.' : `Say hello. Only ${other ? 'the two of you' : 'the people here'} can see this.`}</div>
    </div>
  {:else if source.loadingOlder}
    <div class="faint center">Loading older messages</div>
  {/if}

  {#each list as m, i (m.id)}
    {@const at = placed[i]}
    {#if at?.time}
      <div class="time" data-day={dayLabel(m.created_at)}><b>{dayLabel(m.created_at)}</b> {shortTime(m.created_at)}</div>
    {/if}
    {#if m.id === firstUnread}
      <div class="new"><span>New</span></div>
    {/if}
    <MessageItem {m} {prefix} head={at?.head ?? true} tail={at?.tail ?? true} {onreply} onmediaready={mediaReady} />
  {/each}
</div>
{#if day}<div class="day" class:shown={dayShown} aria-hidden="true">{day}</div>{/if}
{#if far}
  <!-- A pill with words on a wide screen; a round button with a count on a phone. -->
  <button class="latest" class:lit={arrived > 0} onclick={latest} aria-label={arrived ? `${arrived} new ${arrived === 1 ? 'message' : 'messages'}` : 'Jump to latest'}>
    <span class="words">{arrived ? `${arrived} new ${arrived === 1 ? 'message' : 'messages'}` : 'Jump to latest'}</span><Icon name="down" size={14} />
    {#if arrived}<span class="count">{arrived > 99 ? '99+' : arrived}</span>{/if}
  </button>
{/if}
</div>

<style>
  .wrap { flex: 1; min-height: 0; display: flex; flex-direction: column; position: relative; }
  .latest {
    position: absolute; bottom: calc(10px + var(--list-bottom, 0px)); left: 50%; transform: translateX(-50%); z-index: 3;
    display: inline-flex; align-items: center; gap: 6px; padding: 6px 12px 6px 14px;
    border-radius: var(--r-pill, 999px); border: 1px solid var(--line-strong); background: var(--bg-2); color: var(--ink);
    font-size: var(--fs-sm); font-weight: 600; box-shadow: var(--lift);
    transition: background-color var(--t-fast), transform var(--t) var(--ease-out);
  }
  @media (hover: hover) { .latest:hover { background: var(--bg-3); transform: translateX(-50%) translateY(-1px); } }
  .latest:active { transform: translateX(-50%) scale(.96); }
  .latest.lit { background: var(--lamp); border-color: var(--lamp); color: var(--bg); box-shadow: var(--lift), var(--glow); }
  @media (prefers-reduced-motion: no-preference) { .latest { animation: latest-in .2s var(--ease-out); } }
  @keyframes latest-in { from { opacity: 0; transform: translateX(-50%) translateY(6px); } }
  .latest .count { display: none; }
  /* A phone: a round chevron in the corner, clear of the words, with how many are new. */
  @media (max-width: 600px) {
    .latest, .latest.lit { left: auto; right: 12px; bottom: calc(12px + var(--list-bottom, 0px)); transform: none; width: 40px; height: 40px; padding: 0; justify-content: center;
      border: 0; border-radius: 50%; background: var(--bg-2); color: var(--ink-2); box-shadow: 0 0 0 .5px var(--line-strong), var(--lift); }
    .latest :global(svg) { width: 20px; height: 20px; }
    .latest .words { display: none; }
    .latest .count {
      display: block; position: absolute; top: -6px; right: -4px; min-width: 20px; height: 20px; padding: 0 5px; border-radius: 10px;
      background: var(--lamp); color: var(--on-lamp); font: 700 12px/20px var(--body); text-align: center; font-variant-numeric: tabular-nums;
    }
    .latest:active { transform: scale(.94); }
    @keyframes latest-in { from { opacity: 0; transform: translateY(6px); } }
  }
  /* The day you're reading, floating at the top while you scroll, on touch screens. */
  .day {
    position: absolute; top: calc(8px + var(--list-top, 0px)); left: 50%; z-index: 3; transform: translateX(-50%); padding: 3px 10px; border-radius: var(--r-pill, 999px);
    background: var(--glass); -webkit-backdrop-filter: blur(12px); backdrop-filter: blur(12px); box-shadow: 0 0 0 .5px var(--line);
    font-size: var(--fs-xs); font-weight: 600; color: var(--ink-2); pointer-events: none; opacity: 0; transition: opacity .3s;
  }
  .day.shown { opacity: 1; transition-duration: .1s; }
  @media (hover: hover) { .day { display: none; } }
  /* A short conversation sits down by the composer, where the next line will appear,
     not at the top of an empty page. */
  .list {
    flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain; display: flex; flex-direction: column;
    padding: calc(12px + var(--list-top, 0px)) 0 calc(8px + var(--list-bottom, 0px));
    /* A message brought into view lands between the bars, not under them. */
    scroll-padding-block: var(--list-top, 0px) var(--list-bottom, 0px);
  }
  .list > :global(*) { flex: none; }
  .list > :global(:first-child) { margin-top: auto; }
  /* A thread reads on from its first message, pinned above, so its replies start
     right under it rather than down by the reply box. */
  .list.thread > :global(:first-child) { margin-top: 0; }
  .start { display: flex; flex-direction: column; align-items: center; gap: 4px; padding: 28px var(--gutter) 18px; text-align: center; }
  .start.empty { margin-block: auto; padding-bottom: 28px; }
  .start-title { font-size: 26px; text-wrap: balance; }
  .start .muted { max-width: 34ch; text-wrap: pretty; }
  .start-face { display: grid; margin-bottom: 10px; }
  .center { text-align: center; padding: 8px; font-size: var(--fs-sm); }
  /* The time sits between bursts, not on every line. */
  .time { margin: calc(20px * var(--density)) var(--gutter) 2px; text-align: center; font-size: var(--fs-xs); color: var(--ink-3); }
  .time b { font-weight: 600; color: var(--ink-2); }
  /* Where unread starts is live, so it is lit: a thin accent rule. */
  .new { display: flex; align-items: center; gap: 10px; margin: 12px var(--gutter) 0; color: var(--lamp); font: 700 var(--fs-xs)/1.5 var(--body); }
  .new::before, .new::after { content: ''; flex: 1; height: 1px; background: color-mix(in srgb, var(--lamp) 50%, transparent); }
  .start-title { color: var(--ink); }
  .start-mark { display: grid; place-items: center; width: 72px; height: 72px; margin-bottom: 10px; border-radius: var(--r-avatar, 35%); color: var(--lamp); background: var(--lamp-glow); box-shadow: inset 0 0 0 1px var(--lamp-dim); }
</style>
