<script lang="ts">
  import { untrack } from 'svelte'
  import { store } from '../lib/store.svelte'
  import { plainMentions } from '../lib/markdown'
  import { snippet } from '../lib/klipy'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import type { Channel, ThreadSummary } from '../lib/types'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'

  // The conversations in a room, behind one pill under its title that says how
  // many are open: the open ones; the unread ones, which include resolved
  // conversations the open list can never show; and resolved history, which is
  // browseable rather than archived away.
  // `subtitle`: on a phone the pill is the grey line under the room's name in its bar, and opens the same list.
  let { channel, selected, onopen, subtitle = false }: {
    channel: Channel
    selected?: string
    onopen: (threadId: string, rootId: string) => void
    subtitle?: boolean
  } = $props()

  type Mode = 'open' | 'unread' | 'resolved'
  const LABEL: Record<Mode, string> = { open: 'Open', unread: 'Unread', resolved: 'Resolved' }
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

  // Conversations seen going from open to resolved during this visit, newest
  // first. The Resolved list shows them ahead of the server's pages without a
  // reload. Only a seen transition counts: a resolved conversation that merely
  // arrives (a rename, an older page, a cache from an earlier visit) does not.
  let knownOpen = new Set<string>()
  let watching = ''
  let justResolved = $state.raw<string[]>([])
  $effect(() => {
    const room = channel.id
    const now = store.threadsIn(room, false)
    untrack(() => {
      if (watching !== room) { watching = room; knownOpen = new Set(); justResolved = [] }
      const fresh = now.filter((t) => t.resolved_at && knownOpen.has(t.id)).map((t) => t.id)
      for (const t of now) {
        if (t.resolved_at) knownOpen.delete(t.id)
        else knownOpen.add(t.id)
      }
      if (fresh.length) justResolved = [...fresh, ...justResolved.filter((id) => !fresh.includes(id))]
    })
  })
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
  // The pill's unread count must see resolved conversations too, which the room
  // itself never loads: every unread one, in the Store, when the room opens and
  // again after a reconnect. The lists keep their own paging.
  async function loadUnread(room: string) {
    let before: string | undefined
    for (let page = 0; page < 10; page++) {
      const views = await store.loadThreads(room, { unreadOnly: true, before, limit: 200 }).catch(() => [])
      if (views.length < 200 || channel.id !== room) return
      before = views.at(-1)!.thread.id
    }
  }
  $effect(() => { const room = channel.id; void store.connected; untrack(() => void loadUnread(room)) })
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
    if (mode === 'resolved') {
      // Likewise a conversation resolved here since the list loaded.
      const known = new Set(rows.map((t) => t.id))
      const fresh = justResolved.map((id) => store.thread(id)).filter((t): t is ThreadSummary => !!t?.resolved_at && !known.has(t.id))
      return [...fresh, ...rows.filter((t) => t.resolved_at)]
    }
    return rows
  })
  let open = $state(false)
  let details = $state<HTMLDetailsElement>()
  const openCount = $derived(store.threadsIn(channel.id).length)
  // Unread anywhere in the room's conversations lights the pill, as the chips did.
  const unreadAll = $derived(store.threadsIn(channel.id, false).map((t) => store.threadUnread(t.id)).filter((u) => u.count))
  const authorOf = (t: ThreadSummary) =>
    (store.root(t.root_message_id) ?? store.messages.get(channel.id)?.find((m) => m.id === t.root_message_id))?.author_id ?? t.created_by
  function pick(t: ThreadSummary) {
    open = false
    onopen(t.id, t.root_message_id)
  }
  const empty = $derived({
    open: 'No open conversations yet. Reply to a message to start one.',
    unread: 'Nothing unread in a conversation here.',
    resolved: 'Nothing has been resolved here yet.',
  }[mode])
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes this native disclosure.) -->
<details class="threads" class:subtitle bind:this={details} bind:open onkeydown={(e) => { if (e.key === 'Escape') { e.preventDefault(); open = false; details?.querySelector('summary')?.focus() } }}>
  <summary class:pill={!subtitle} class:lit={unreadAll.length > 0} data-thread-pill aria-label={`Conversations in this room: ${openCount}${lists.open.more ? ' or more' : ''} open${unreadAll.length ? ', some unread' : ''}`}>
    {#if !subtitle}<Icon name="thread" size={14} />{/if}
    <span class="label">{#if openCount}<b>{openCount}{lists.open.more ? '+' : ''}</b> open {openCount === 1 && !lists.open.more ? 'thread' : 'threads'}{:else}Threads{/if}</span>
    {#if unreadAll.length}<span class="count" class:at={unreadAll.some((u) => u.mention)}>{unreadAll.some((u) => u.mention) ? '@' : unreadAll.reduce((n, u) => n + u.count, 0)}</span>{/if}
    <Icon name={subtitle ? 'chevron' : 'down'} size={subtitle ? 11 : 12} />
  </summary>
  <div class="sheet" popover="auto" use:disclosurePopover={'start'}>
    {#if open}
      <div class="box" data-thread-strip aria-label="Conversations in this room">
        <div class="tabs segments small">
          {#each ['open', 'unread', 'resolved'] as const as m (m)}
            <button aria-pressed={mode === m} onclick={() => show(m)}>{LABEL[m]}</button>
          {/each}
        </div>
        <div class="list">
          {#if here.loading && !summaries.length}
            <p class="note faint">Looking…</p>
          {:else if here.error}
            <p class="note"><span class="err" role="alert">{here.error}</span> <button class="pill" onclick={() => retry(mode)}>Retry</button></p>
          {:else if !summaries.length}
            <p class="note faint">{empty}</p>
          {:else}
            {#each summaries as t (t.id)}
              {@const u = store.threadUnread(t.id)}
              {@const last = t.last_reply}
              <button
                class="row chip" class:lit={!!u.count} class:on={selected === t.id} data-thread={t.id}
                aria-current={selected === t.id ? 'true' : undefined}
                onclick={() => pick(t)}
              >
                <!-- Whoever wrote the message it's about, when that message is loaded. -->
                <Avatar userId={authorOf(t)} size={32} presence={false} />
                <span class="text">
                  <span class="name">{plainMentions(t.title, store.users)}</span>
                  <span class="sub"><b>{t.reply_count} {t.reply_count === 1 ? 'reply' : 'replies'}</b>{#if last}{' · '}{store.name(last.author_id)}: {plainMentions(snippet(last.excerpt), store.users).trim() || 'shared something'}{/if}{#if t.resolved_at && mode !== 'resolved'}{' · resolved'}{/if}</span>
                </span>
                {#if u.count}<span class="count" class:at={u.mention}>{u.mention ? '@' : u.count}</span>{/if}
              </button>
            {/each}
          {/if}
          <!-- Still offered when the visible rows were filtered away but the server
               has more pages: an emptied view is not the end of the list. -->
          {#if here.more && !here.error}
            <button class="pill more" onclick={() => load(mode, true)} disabled={here.loading}>
              {here.loading ? 'Loading…' : 'More'}
            </button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</details>

<style>
  /* The popover places itself against this box, so it must have one. */
  .threads { display: flex; min-width: 0; }
  summary { list-style: none; cursor: pointer; }
  summary::-webkit-details-marker { display: none; }
  /* One pill under the room's title. Kept small: it is a way in, not a headline. */
  summary.pill { min-height: 26px; padding: 2px 9px 2px 8px; gap: 5px; font-size: var(--fs-xs); color: var(--ink-2); max-width: 100%; }
  summary.pill b { color: var(--lamp); font-weight: 700; }
  @media (hover: hover) { summary.pill:hover { color: var(--ink); } }
  .threads[open] > summary { color: var(--ink); }
  summary.pill.lit { border-color: color-mix(in srgb, var(--lamp) 45%, transparent); }
  .label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  @media (pointer: coarse) { summary.pill { min-height: 30px; } }
  /* The bar's grey subtitle. The tap area reaches past the text without moving it. */
  .subtitle { justify-content: center; max-width: 100%; }
  .subtitle summary { display: flex; align-items: center; gap: 3px; max-width: 100%; padding: 4px 6px; margin: -4px 0; font-size: 12px; line-height: 1.2; color: var(--ink-3); }
  .subtitle summary b { font-weight: 600; color: inherit; }
  .subtitle summary.lit b { color: var(--lamp); }
  .subtitle summary:active { opacity: .5; }
  .subtitle .count { min-width: 16px; padding: 0 4px; font-size: 11px; line-height: 16px; }
  [popover] { position: fixed; inset: auto; margin: 0; padding: 0; border: 1px solid var(--line-strong); border-radius: var(--r-lg); background: var(--bg-2); box-shadow: 0 14px 44px var(--shadow); overflow: hidden; }
  .box { width: min(400px, calc(100vw - 16px)); max-height: min(480px, calc(100vh - 120px)); display: flex; flex-direction: column; }
  .tabs { margin: 10px 10px 6px; align-self: flex-start; }
  .list { overflow-y: auto; overscroll-behavior: contain; padding: 0 6px 8px; display: grid; gap: 2px; }
  .row { display: flex; align-items: center; gap: 10px; padding: 8px; border-radius: var(--r); text-align: left; color: var(--ink); min-width: 0; }
  @media (hover: hover) { .row:hover { background: var(--hover); } }
  .row:active { background: var(--press); }
  .row.on { background: var(--sel); }
  .text { flex: 1; min-width: 0; display: grid; gap: 1px; }
  .name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { font-size: var(--fs-sm); color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub b { color: var(--lamp); font-weight: 700; }
  .row.lit .name { color: var(--ink); }
  .note { margin: 14px 10px; font-size: var(--fs-sm); }
  .err { color: var(--danger, var(--lamp)); }
  .more { justify-self: center; margin-top: 4px; }
  .count {
    flex: none; min-width: 18px; padding: 0 5px; border-radius: var(--r-pill, 999px); background: var(--lamp);
    color: var(--on-lamp, #000); font-size: var(--fs-xs); font-weight: 700; text-align: center;
  }
  @media (max-width: 600px) { .box { width: 100%; max-height: 70vh; } .row { min-height: 56px; } }
</style>
