<script lang="ts">
  import SidebarToggle from './SidebarToggle.svelte'
  import { instances, store, type Store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { render } from '../lib/markdown'
  import { shortTime, dayLabel } from '../lib/time'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'

  let { onmenu, narrow }: { onmenu: () => void; narrow: boolean } = $props()

  // Rooms with unread, newest first, mentions on top. Message previews come from whatever is loaded.
  const groups = $derived.by(() =>
    instances.all.flatMap(s => s.channels
      .map((c) => {
        const u = s.unread(c.id)
        const msgs = (s.messages.get(c.id) || []).filter((m) => m.id > u.lastRead && m.author_id !== s.me?.id)
        return { s, c, msgs, count: u.count, mention: u.mention }
      })
      .filter((g) => g.count > 0)
      ).sort((a, b) => Number(b.mention) - Number(a.mention) || (b.msgs.at(-1)?.id || '').localeCompare(a.msgs.at(-1)?.id || '')),
  )
  // Load previews for unread rooms we haven't opened yet.
  $effect(() => { for (const g of groups) if (!g.s.messages.has(g.c.id)) g.s.loadLatest(g.c.id) })

  // Unread conversations inside each unread room. unread_only WITHOUT a
  // resolution filter: a resolved conversation with unread replies is exactly
  // what someone would otherwise never find. Their counts already sit inside the
  // room's server aggregate, so they are shown, never added to it.
  const PER = 10
  // Per room: the ids the SERVER returned and the cursor from ITS order. The
  // display order is activity, the API cursor is creation, and paging one by the
  // other skips or repeats whole pages. Reading the last displayed row must not
  // be able to destroy the next page's cursor either, so the cursor is kept here
  // rather than re-derived from what is still unread.
  // `appending` remembers what a failed More was fetching so Retry resumes that
  // cursor instead of discarding the pages already shown.
  type Rows = { ids: string[]; cursor?: string; more: boolean; error: string; loading: boolean; appending: boolean }
  let rows = $state<Record<string, Rows>>({})
  const key = (s: Store, channelId: string) => s.origin + channelId

  async function loadRows(s: Store, channelId: string, append = false) {
    const k = key(s, channelId)
    const from = rows[k] ?? { ids: [], more: false, error: '', loading: false, appending: false }
    if (from.loading) return
    rows = { ...rows, [k]: { ...from, loading: true, error: '', appending: append } }
    try {
      const page = await s.loadThreads(channelId, {
        unreadOnly: true, limit: PER, before: append ? from.cursor : undefined,
      })
      const ids = page.map((v) => v.thread.id)
      rows = {
        ...rows,
        [k]: {
          ids: append ? [...from.ids, ...ids.filter((id) => !from.ids.includes(id))] : ids,
          cursor: ids.at(-1) ?? from.cursor,
          more: page.length === PER,
          error: '', loading: false, appending: false,
        },
      }
    } catch (e) {
      // Previous rows are kept, the failure is visible, and nothing retries by
      // itself: a failing request inside a dependent effect would loop forever.
      rows = { ...rows, [k]: { ...from, loading: false, error: (e as Error).message, appending: append } }
    }
  }
  $effect(() => { for (const g of groups) if (!rows[key(g.s, g.c.id)]) void loadRows(g.s, g.c.id) })

  const unreadThreads = (s: Store, channelId: string) => {
    const listed = (rows[key(s, channelId)]?.ids ?? [])
      .map((id) => s.thread(id))
      .filter((t): t is import('../lib/types').ThreadSummary => !!t && s.threadUnread(t.id).count > 0)
    // A conversation that became unread while this view was open belongs here
    // without needing a click or a reopen.
    const known = new Set(listed.map((t) => t.id))
    const fresh = s.threadsIn(channelId, false)
      .filter((t) => !known.has(t.id) && s.threadUnread(t.id).count > 0)
    return [...listed, ...fresh]
  }

  function open(s: Store, id: string) { instances.select(s); router.go(`/c/${id}`) }
  function openThread(s: Store, channelId: string, threadId: string) {
    instances.select(s); router.go(`/c/${channelId}/t/${threadId}`)
  }
  // The explicit flat acknowledgement: it clears the room AND its conversations,
  // which is what "Mark all read" has always meant here.
  function clearAll() { for (const g of groups) g.s.markAllRead(g.c.id) }
</script>

<section class="inbox">
  <header class="head">
    {#if !narrow && !store.layout.sidebar}<SidebarToggle />{/if}
    {#if narrow}<button class="btn quiet iconbtn" onclick={onmenu} aria-label="Menu"><Icon name="menu" /></button>{/if}
    <span class="kind"><Icon name="inbox" size={18} /></span>
    <h1 class="display">Inbox</h1>
    <span class="spacer"></span>
    {#if groups.length}<button class="btn quiet" onclick={clearAll}><Icon name="check" /> Mark all read</button>{/if}
  </header>

  <div class="scroll">
    {#if !groups.length}
      <div class="empty">
        <div class="display big">You're caught up.</div>
        <div class="muted">Mentions, direct messages, and rooms you follow land here.</div>
      </div>
    {/if}
    {#each groups as g (g.s.origin + g.c.id)}
      {#if instances.all.length > 1}<div class="eyebrow" style="margin:16px 0 6px">{g.s.settings.instance_name}</div>{/if}
      <div class="group" class:mention={g.mention}>
        <button class="title" onclick={() => open(g.s, g.c.id)}>
          {#if g.c.kind === 'dm'}<Icon name="lock" size={14} />{:else}<Icon name="hash" size={14} />{/if}
          <span class="display name">{g.s.title(g.c)}</span>
          <span class="faint mono">{g.count} new</span>
          {#if g.mention}<span class="at">mentions you</span>{/if}
        </button>
        {#each g.msgs.slice(-4) as m (m.id)}
          <button class="line" onclick={() => open(g.s, g.c.id)}>
            <Avatar instance={g.s} userId={m.author_id} size={22} />
            <span class="who">{g.s.name(m.author_id)}</span>
            <span class="text">{@html render(m.content, g.s.users) || '<i>sent a file</i>'}</span>
            <span class="when faint mono">{dayLabel(m.created_at) === 'Today' ? shortTime(m.created_at) : dayLabel(m.created_at)}</span>
          </button>
        {/each}
        {#if g.msgs.length > 4}<button class="more faint" onclick={() => open(g.s, g.c.id)}>and {g.msgs.length - 4} earlier</button>{/if}
        {#each unreadThreads(g.s, g.c.id) as t (t.id)}
          {@const u = g.s.threadUnread(t.id)}
          <button class="line thread" onclick={() => openThread(g.s, g.c.id, t.id)}>
            <Icon name="reply" size={13} />
            <span class="who">{t.title}</span>
            <span class="text faint">{u.count} unread {u.count === 1 ? 'reply' : 'replies'}</span>
            {#if u.mention}<span class="at">mentions you</span>{/if}
            {#if t.resolved_at}<span class="when faint mono">resolved</span>{/if}
          </button>
        {/each}
        {#if rows[key(g.s, g.c.id)]?.error}
          <div class="line">
            <span class="text err" role="alert">{rows[key(g.s, g.c.id)]!.error}</span>
            <button class="btn quiet" onclick={() => loadRows(g.s, g.c.id, rows[key(g.s, g.c.id)]!.appending)}>Retry</button>
          </div>
        {:else if rows[key(g.s, g.c.id)]?.more}
          <button class="more faint" onclick={() => loadRows(g.s, g.c.id, true)}>More conversations</button>
        {/if}
      </div>
    {/each}
  </div>
</section>

<style>
  .inbox { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .head { display: flex; align-items: center; gap: 10px; padding: 10px 16px; min-height: 52px; border-bottom: 1px solid var(--line); }
  .kind { color: var(--ink-3); display: grid; }
  h1 { font-size: 19px; margin: 0; font-weight: 600; }
  .spacer { flex: 1; }
  .iconbtn { padding: 6px; }
  .scroll { flex: 1; overflow-y: auto; padding: 16px; }
  .empty { padding: 60px 20px; text-align: center; display: grid; gap: 6px; }
  .big { font-size: 26px; }
  .group { border: 1px solid var(--line); border-radius: var(--r-lg); margin-bottom: 12px; overflow: hidden; background: var(--bg-2); }
  .group.mention { border-color: var(--lamp-dim); }
  .title { width: 100%; display: flex; align-items: center; gap: 8px; padding: 10px 14px; text-align: left; color: var(--ink-2); border-bottom: 1px solid var(--line); }
  .title:hover { background: var(--bg-3); }
  .name { font-size: 17px; color: var(--ink); flex: 1; }
  .at { font-family: var(--mono); font-size: 11px; color: var(--lamp); }
  .line { width: 100%; display: flex; align-items: center; gap: 10px; padding: 7px 14px; text-align: left; color: var(--ink-2); }
  .line:hover { background: var(--bg-3); }
  .who { font-weight: 700; color: var(--ink); flex: none; }
  .text { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .text :global(br) { display: none; }
  .when { flex: none; font-size: 11px; }
  .more { width: 100%; padding: 6px 14px 10px; text-align: left; font-size: 12.5px; }
  .line.thread { border-top: 1px solid var(--line); }
  .err { color: var(--danger, var(--lamp)); }
  .line.thread .who { font-weight: 600; }
</style>
