<script lang="ts">
  import { tick } from 'svelte'
  import { store } from '../lib/store.svelte'
  import type { Channel, Message } from '../lib/types'
  import { dayLabel, sameDay, within } from '../lib/time'
  import MessageItem from './MessageItem.svelte'

  let { channel, onreply }: { channel: Channel; onreply: (m: Message) => void } = $props()
  let el: HTMLDivElement
  let stickToBottom = $state(true)
  const list = $derived(store.messages.get(channel.id) || [])
  // Where "new" starts: the read marker as it was when this view opened. Not reactive on purpose.
  // svelte-ignore state_referenced_locally
  const openedAt = store.lastRead[channel.id] || ''
  const firstUnread = $derived(list.find((m) => m.id > openedAt && m.author_id !== store.me?.id)?.id)

  // Keep the newest message in view unless the reader scrolled up.
  $effect(() => {
    void list.length
    if (stickToBottom) tick().then(() => el?.scrollTo({ top: el.scrollHeight }))
  })

  async function onScroll() {
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 80
    stickToBottom = nearBottom
    if (el.scrollTop < 120 && !store.exhausted.has(channel.id) && !store.loadingOlder.has(channel.id)) {
      const before = el.scrollHeight
      await store.loadOlder(channel.id)
      await tick()
      el.scrollTop += el.scrollHeight - before
    }
  }

  // A message continues the previous one when same author, same day, within 5 minutes, not a reply.
  function continues(prev: Message | undefined, m: Message) {
    return !!prev && prev.author_id === m.author_id && !m.reply_to && sameDay(prev.created_at, m.created_at) && within(prev.created_at, m.created_at, 5 * 60_000)
  }
</script>

<div class="list" bind:this={el} onscroll={onScroll}>
  {#if store.exhausted.has(channel.id)}
    <div class="start">
      <div class="display start-title">{channel.kind === 'dm' ? store.title(channel) : `#${channel.name}`}</div>
      <div class="muted">{list.length ? 'This is where it started.' : 'Nobody has said anything here yet. Say the first thing.'}</div>
    </div>
  {:else if store.loadingOlder.has(channel.id)}
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
    <MessageItem {m} compact={continues(prev, m)} {onreply} />
  {/each}
</div>

<style>
  .list { flex: 1; min-height: 0; overflow-y: auto; padding: 12px 0 8px; overscroll-behavior: contain; }
  .start { padding: 28px 20px 18px; }
  .start-title { font-size: 28px; }
  .center { text-align: center; padding: 8px; font-size: 13px; }
  .day { display: flex; align-items: center; gap: 12px; margin: 14px 20px 6px; color: var(--ink-3); font-size: 12px; font-family: var(--mono); }
  .day::before, .day::after { content: ''; flex: 1; height: 1px; background: var(--line); }
  .new { display: flex; align-items: center; gap: 10px; margin: 6px 20px; color: var(--lamp); font-size: 11px; font-family: var(--mono); text-transform: uppercase; letter-spacing: 0.08em; }
  .new::before { content: ''; flex: 1; height: 1px; background: var(--lamp); opacity: 0.6; }
</style>
