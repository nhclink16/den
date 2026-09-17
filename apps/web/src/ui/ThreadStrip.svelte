<script lang="ts">
  import { store } from '../lib/store.svelte'
  import type { Channel, ThreadSummary } from '../lib/types'

  // The conversations in a room: the open ones, always visible rather than
  // behind an icon; the unread ones, which include resolved conversations the
  // open list can never show; and resolved history, which is browseable rather
  // than archived away.
  let { channel, selected, onopen }: {
    channel: Channel
    selected?: string
    onopen: (threadId: string, rootId: string) => void
  } = $props()

  type Mode = 'open' | 'unread' | 'resolved'
  const LABEL: Record<Mode, string> = { open: 'Open', unread: 'Unread threads', resolved: 'Resolved' }
  const PER = 20
  let mode = $state<Mode>('open')

  // Each list keeps the ids the SERVER returned, in the order it returned them,
  // and pages from that order. The strip displays open conversations in its own
  // stable activity order, and using a display cursor against a creation-ordered
  // API would skip or repeat whole pages.
  // `attempted` is pinned separately from `loaded`: a failure must not reset the
  // condition the activation effect watches, or the effect fires again the
  // moment the catch runs and Retry becomes a loop with a button on it.
  // `appending` remembers what a failed More was doing, so retrying it resumes
  // that cursor instead of silently discarding the pages already shown.
  type List = {
    ids: string[]; cursor?: string; more: boolean
    loading: boolean; error: string; loaded: boolean; attempted: boolean; appending: boolean
  }
  const blank = (): List => ({ ids: [], more: false, loading: false, error: '', loaded: false, attempted: false, appending: false })
  let lists = $state<Record<Mode, List>>({ open: blank(), unread: blank(), resolved: blank() })
  const here = $derived(lists[mode])

  function reset() { lists = { open: blank(), unread: blank(), resolved: blank() } }
  // A different room is a different set of conversations.
  // svelte-ignore state_referenced_locally
  let shown = $state(channel.id)
  $effect(() => {
    if (channel.id === shown) return
    shown = channel.id
    mode = 'open'
    reset()
  })

  const query = (m: Mode, before?: string) =>
    m === 'open' ? { resolved: false, before, limit: PER }
    : m === 'unread' ? { unreadOnly: true, before, limit: PER }
    : { resolved: true, before, limit: PER }

  async function load(m: Mode, more = false) {
    const from = lists[m]
    if (from.loading) return
    lists = { ...lists, [m]: { ...from, loading: true, error: '', attempted: true, appending: more } }
    try {
      const page = await store.loadThreads(channel.id, query(m, more ? from.cursor : undefined))
      const ids = page.map((v) => v.thread.id)
      lists = {
        ...lists,
        [m]: {
          ids: more ? [...from.ids, ...ids.filter((id) => !from.ids.includes(id))] : ids,
          cursor: ids.at(-1) ?? from.cursor,
          more: page.length === PER,
          loading: false, error: '', loaded: true, attempted: true, appending: false,
        },
      }
    } catch (e) {
      // Keeps what was already shown, offers Retry, and does NOT reset
      // `attempted`, so nothing re-triggers it on its own.
      lists = { ...lists, [m]: { ...from, loading: false, error: (e as Error).message, attempted: true, appending: more } }
    }
  }
  /// Retry resumes exactly what failed, including which page it was fetching.
  const retry = (m: Mode) => load(m, lists[m].appending)

  // Open is loaded by the room itself; the other two load when first shown and
  // refresh whenever they are shown again, because what belongs in them changes
  // while the strip is mounted.
  function show(m: Mode) {
    mode = m
    if (m !== 'open') void load(m)
  }
  $effect(() => { if (mode === 'open' && !lists.open.attempted) void load('open') })
  // A reconnect can change every one of these.
  let online = $state(store.connected)
  $effect(() => {
    if (store.connected === online) return
    online = store.connected
    if (online) void load(mode)
  })

  const summaries = $derived.by(() => {
    if (mode === 'open') return store.threadsIn(channel.id)
    const rows = here.ids.map((id) => store.thread(id)).filter((t): t is ThreadSummary => !!t)
    // Rows that no longer belong are dropped rather than left sitting there, in
    // both directions: a conversation that was read leaves Unread, and one that
    // was reopened leaves Resolved.
    if (mode === 'unread') {
      const known = new Set(rows.map((t) => t.id))
      // A conversation that became unread while this list was open belongs here
      // too, without waiting for a click: the room's own set is authoritative.
      const alsoUnread = store.threadsIn(channel.id, false)
        .filter((t) => !known.has(t.id) && store.threadUnread(t.id).count > 0)
      return [...rows.filter((t) => store.threadUnread(t.id).count > 0), ...alsoUnread]
    }
    return mode === 'resolved' ? rows.filter((t) => t.resolved_at) : rows
  })
  const empty = $derived({
    open: 'No open conversations yet. Reply to a message to start one.',
    unread: 'Nothing unread in a conversation here.',
    resolved: 'Nothing has been resolved here yet.',
  }[mode])
</script>

<div class="strip" data-thread-strip aria-label="Conversations in this room">
  <div class="tabs">
    {#each ['open', 'unread', 'resolved'] as const as m (m)}
      <button class:on={mode === m} aria-pressed={mode === m} onclick={() => show(m)}>{LABEL[m]}</button>
    {/each}
  </div>

  <div class="rail">
    {#if here.loading && !summaries.length}
      <span class="faint">Looking…</span>
    {:else if here.error}
      <span class="err" role="alert">{here.error}</span>
      <button class="chip" onclick={() => retry(mode)}>Retry</button>
    {:else if !summaries.length}
      <span class="faint">{empty}</span>
      <!-- Everything visible was filtered away, but the server has more pages;
           an emptied view is not the end of the list. -->
      {#if here.more}
        <button class="chip more" onclick={() => load(mode, true)} disabled={here.loading}>
          {here.loading ? 'Loading…' : 'More'}
        </button>
      {/if}
    {:else}
      {#each summaries as t (t.id)}
        {@const u = store.threadUnread(t.id)}
        <button
          class="chip" class:lit={!!u.count} class:active={selected === t.id} data-thread={t.id}
          aria-current={selected === t.id ? 'true' : undefined}
          onclick={() => onopen(t.id, t.root_message_id)}
        >
          <span class="name">{t.title}</span>
          <span class="faint mono">{t.reply_count}</span>
          {#if u.count}<span class="count" class:at={u.mention}>{u.mention ? '@' : u.count}</span>{/if}
          {#if t.resolved_at && mode !== 'resolved'}<span class="faint mono">resolved</span>{/if}
        </button>
      {/each}
      <!-- Still offered when the visible rows were filtered away but the server
           has more pages: an emptied view is not the end of the list. -->
      {#if here.more}
        <button class="chip more" onclick={() => load(mode, true)} disabled={here.loading}>
          {here.loading ? 'Loading…' : 'More'}
        </button>
      {/if}
    {/if}
  </div>
</div>

<style>
  .strip { border-bottom: 1px solid var(--line); padding: 6px var(--gutter) 8px; display: grid; gap: 6px; }
  .tabs { display: flex; gap: 4px; }
  .tabs button { padding: 3px 9px; border-radius: 999px; font-size: 12px; color: var(--ink-3); border: 1px solid transparent; }
  .tabs button.on { color: var(--ink); border-color: var(--line); background: var(--bg-2); }
  .tabs button:hover { color: var(--ink); }
  .rail { display: flex; gap: 6px; overflow-x: auto; padding-bottom: 2px; align-items: center; scrollbar-width: thin; }
  .rail .faint, .rail .err { font-size: 12px; white-space: nowrap; }
  .err { color: var(--danger, var(--lamp)); }
  .chip {
    display: inline-flex; align-items: center; gap: 7px; flex: none; max-width: 260px;
    padding: 4px 10px; border: 1px solid var(--line); border-radius: 999px;
    font-size: 12px; color: var(--ink-2); background: var(--bg-2);
  }
  .chip:hover { color: var(--ink); background: var(--hover); }
  .chip.lit { border-color: var(--lamp); color: var(--ink); }
  .chip.active { border-color: var(--accent); color: var(--ink); box-shadow: 0 0 0 2px var(--accent-glow); }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .count {
    min-width: 18px; padding: 0 5px; border-radius: 999px; background: var(--lamp);
    color: var(--on-lamp, #000); font-family: var(--mono); font-size: 11px; text-align: center;
  }
  .more { font-family: var(--mono); }
  @media (prefers-reduced-motion: no-preference) { .chip { transition: background 0.12s, border-color 0.12s; } }
</style>
