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

  function mediaReady() {
    void tick().then(() => {
      if (stickToBottom && !source.target && el?.isConnected) el.scrollTo({ top: el.scrollHeight })
    })
  }

  async function onScroll() {
    const scroller = el
    if (!scroller) return
    stickToBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80
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

<div class="list" bind:this={el} onscroll={onScroll}>
  {#if source.exhausted}
    <div class="start">
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

<style>
  .list { flex: 1; min-height: 0; overflow-y: auto; padding: 12px 0 8px; overscroll-behavior: contain; }
  .start { padding: 28px var(--gutter) 18px; }
  .start-title { font-size: 28px; }
  .center { text-align: center; padding: 8px; font-size: 13px; }
  .day { display: flex; align-items: center; gap: 12px; margin: 14px var(--gutter) 6px; color: var(--ink-3); font-size: 12px; font-family: var(--mono); }
  .day::before, .day::after { content: ''; flex: 1; height: 1px; background: var(--line); }
  .new { display: flex; align-items: center; gap: 10px; margin: 6px 20px; color: var(--lamp); font-size: 11px; font-family: var(--mono); text-transform: uppercase; letter-spacing: 0.08em; }
  .new::before { content: ''; flex: 1; height: 1px; background: var(--lamp); opacity: 0.6; }
</style>
