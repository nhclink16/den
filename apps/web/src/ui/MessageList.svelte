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
  }
</script>

<script lang="ts">
  import { tick } from 'svelte'
  import { store } from '../lib/store.svelte'
  import type { Channel, Message } from '../lib/types'
  import { dayLabel, sameDay, within } from '../lib/time'
  import MessageItem from './MessageItem.svelte'
  import Icon from './Icon.svelte'

  let { channel, source, onreply }: { channel: Channel; source: Source; onreply: (m: Message) => void } = $props()
  let el: HTMLDivElement
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
  // svelte-ignore state_referenced_locally
  let shownKey = $state(`${source.identity}:${source.identityExtra ?? ''}:${source.target ?? ''}`)
  $effect(() => {
    const key = `${source.identity}:${source.identityExtra ?? ''}:${source.target ?? ''}`
    if (key === shownKey) return
    shownKey = key
    openedAt = source.lastRead
    stickToBottom = !source.target
  })

  // Keep the newest message in view unless the reader scrolled up, or is looking
  // at an older target they asked for.
  $effect(() => {
    void list.length
    if (stickToBottom && !source.target) tick().then(() => el?.scrollTo({ top: el.scrollHeight }))
  })

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
    if (!stickToBottom) return
    const tail = list.at(-1)?.id ?? source.watermark
    if (tail) source.onread?.(tail)
  })

  // Reading history while the room keeps talking: count what arrived below you.
  let far = $state(false)
  let seenLast = $state<string | undefined>()
  $effect(() => { if (!far) seenLast = list.at(-1)?.id })
  const arrived = $derived(far && seenLast ? list.filter((m) => m.id > seenLast! && m.author_id !== store.me?.id).length : 0)
  function latest() {
    const smooth = !matchMedia('(prefers-reduced-motion: reduce)').matches
    el?.scrollTo({ top: el.scrollHeight, behavior: smooth ? 'smooth' : 'auto' })
  }

  function mediaReady() {
    void tick().then(() => {
      if (stickToBottom && !source.target && el?.isConnected) el.scrollTo({ top: el.scrollHeight })
    })
  }

  async function onScroll() {
    const scroller = el
    if (!scroller) return
    const fromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
    stickToBottom = fromBottom < 80
    far = fromBottom > 400
    if (el.scrollTop < 120 && !source.exhausted && !source.loadingOlder) {
      const before = el.scrollHeight
      await source.older()
      await tick()
      // The history request can outlive this view.
      if (el === scroller && scroller.isConnected) scroller.scrollTop += scroller.scrollHeight - before
    }
  }

  // A message continues the previous one when same author, same day, within 5 minutes, not a reply.
  function continues(prev: Message | undefined, m: Message) {
    return !!prev && prev.author_id === m.author_id && !m.reply_to && sameDay(prev.created_at, m.created_at) && within(prev.created_at, m.created_at, 5 * 60_000)
  }
</script>

<div class="wrap">
<div class="list" bind:this={el} onscroll={onScroll}>
  <!-- A thread pins its root above the list, so only a room gets a start block. -->
  {#if source.exhausted && prefix === 'm'}
    <div class="start">
      <div class="start-mark" aria-hidden="true"><Icon name={channel.kind === 'dm' ? 'lock' : 'hash'} size={24} /></div>
      <div class="display start-title">{channel.kind === 'dm' ? store.title(channel) : `#${channel.name}`}</div>
      <div class="muted">{list.length ? 'This is where it started.' : 'Nobody has said anything here yet. Say the first thing.'}</div>
    </div>
  {:else if source.loadingOlder}
    <div class="faint center">Loading older messages</div>
  {/if}

  {#each list as m, i (m.id)}
    {@const prev = list[i - 1]}
    {#if !prev || !sameDay(prev.created_at, m.created_at)}
      <div class="day"><span>{dayLabel(m.created_at)}</span></div>
    {/if}
    {#if m.id === firstUnread}
      <div class="new"><span>new</span></div>
    {/if}
    <MessageItem {m} {prefix} compact={continues(prev, m)} {onreply} onmediaready={mediaReady} />
  {/each}
</div>
{#if far}
  <button class="latest" class:lit={arrived > 0} onclick={latest}>
    {arrived ? `${arrived} new ${arrived === 1 ? 'message' : 'messages'}` : 'Jump to latest'}<Icon name="down" size={14} />
  </button>
{/if}
</div>

<style>
  .wrap { flex: 1; min-height: 0; display: flex; flex-direction: column; position: relative; }
  .latest {
    position: absolute; bottom: 10px; left: 50%; transform: translateX(-50%); z-index: 3;
    display: inline-flex; align-items: center; gap: 6px; padding: 6px 12px 6px 14px;
    border-radius: var(--r-pill, 999px); border: 1px solid var(--line-strong); background: var(--bg-2); color: var(--ink);
    font-size: 13px; font-weight: 600; box-shadow: var(--lift);
    transition: background-color var(--t-fast), transform var(--t) var(--ease-out);
  }
  .latest:hover { background: var(--bg-3); transform: translateX(-50%) translateY(-1px); }
  .latest.lit { background: var(--lamp); border-color: var(--lamp); color: var(--bg); box-shadow: var(--lift), var(--glow); }
  @media (prefers-reduced-motion: no-preference) { .latest { animation: latest-in .2s var(--ease-out); } }
  @keyframes latest-in { from { opacity: 0; transform: translateX(-50%) translateY(6px); } }
  /* A short conversation sits down by the composer, where the next line will appear,
     not at the top of an empty page. */
  .list { flex: 1; min-height: 0; overflow-y: auto; padding: 12px 0 8px; overscroll-behavior: contain; display: flex; flex-direction: column; }
  .list > :global(*) { flex: none; }
  .list > :global(:first-child) { margin-top: auto; }
  .start { padding: 28px var(--gutter) 18px; }
  .start-title { font-size: 28px; }
  .center { text-align: center; padding: 8px; font-size: 13px; }
  /* The day sits in a chip on the rule. */
  .day { display: flex; align-items: center; gap: 12px; margin: 18px var(--gutter) 6px; }
  .day::before, .day::after { content: ''; flex: 1; height: 1px; background: var(--line); }
  .day span {
    padding: 3px 10px; border-radius: var(--r-pill, 999px); border: 1px solid var(--line); background: var(--bg-2);
    color: var(--ink-2); font: 600 11px/1.45 var(--mono); letter-spacing: .06em; text-transform: uppercase;
  }
  /* Where unread starts is live, so it is lit. */
  .new { display: flex; align-items: center; gap: 10px; margin: 10px var(--gutter) 4px; }
  .new::before { content: ''; flex: 1; height: 2px; border-radius: 2px; background: linear-gradient(90deg, transparent, var(--lamp)); box-shadow: var(--glow); }
  .new span {
    padding: 1px 8px; border-radius: var(--r-pill, 999px); background: var(--lamp); color: var(--bg);
    font: 700 11px/1.5 var(--mono); text-transform: uppercase; letter-spacing: .08em; box-shadow: var(--glow);
  }
  .start-title { color: var(--ink); }
  .start-mark { display: grid; place-items: center; width: 52px; height: 52px; margin-bottom: 14px; border-radius: var(--r-avatar, 35%); color: var(--lamp); background: var(--lamp-glow); box-shadow: inset 0 0 0 1px var(--lamp-dim); }
</style>
