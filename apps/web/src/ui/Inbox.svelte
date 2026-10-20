<script lang="ts">
  import SidebarToggle from './SidebarToggle.svelte'
  import HomeButton from './HomeButton.svelte'
  import { instances, store, type Store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { plainMentions } from '../lib/markdown'
  import { previewText } from '../lib/room-list'
  import { native } from '../lib/native'
  import type { Message } from '../lib/types'
  import NavBar from './NavBar.svelte'
  import { shortTime, dayLabel } from '../lib/time'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'
  import Mark from './Mark.svelte'

  let { narrow }: { narrow: boolean } = $props()

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

  const bar = $derived(narrow && !native)
  const when = (iso: string) => (dayLabel(iso) === 'Today' ? shortTime(iso) : dayLabel(iso))
  const preview = (s: Store, m: Message) => plainMentions(previewText({
    content: m.content, attachment_types: m.attachments.map((a) => a.content_type), object_kinds: (m.objects ?? []).map((o) => o.kind),
  }), s.users)
</script>

<!-- `edge`: on a phone the bar reaches up under the status bar (Shell). -->
<section class="inbox" class:edge={bar}>
  {#if !bar}
  <header class="head">
    {#if !narrow && !store.layout.sidebar}<SidebarToggle />{/if}
    {#if narrow}<HomeButton />{/if}
    <span class="kind"><Icon name="inbox" size={18} /></span>
    <h1 class="display">Inbox</h1>
    <span class="spacer"></span>
    {#if groups.length}<button class="btn quiet" onclick={clearAll}><Icon name="check" /> Mark all read</button>{/if}
  </header>
  {/if}

  <div class="scroll">
    {#if bar}
      <NavBar title="Inbox">
        {#snippet leading()}<HomeButton />{/snippet}
        {#snippet trailing()}{#if groups.length}<button class="text-btn" onclick={clearAll}>Read All</button>{/if}{/snippet}
      </NavBar>
    {/if}
    <div class="body">
    {#if !groups.length}
      <div class="empty">
        <span class="empty-mark" aria-hidden="true"><Mark size={56} /></span>
        <div class="display big">You're caught up.</div>
        <div class="muted">Mentions, direct messages, and rooms you follow land here.</div>
      </div>
    {/if}
    <!-- The iPhone's inset grouped list: each room is a section, each message a two-line row. -->
    {#each groups as g (g.s.origin + g.c.id)}
      {#if instances.all.length > 1}<div class="eyebrow" style="margin:16px 0 6px">{g.s.settings.instance_name}</div>{/if}
      <h2 class="list-head">
        <button class="section" onclick={() => open(g.s, g.c.id)}>
          <span class="name">{g.s.title(g.c)}</span>
          <span class="chev" aria-hidden="true"><Icon name="chevron" size={12} /></span>
        </button>
        <span class="new">{#if g.mention}<span class="at">Mentions you</span> · {/if}{g.count} new</span>
      </h2>
      <div class="slist">
        {#each g.msgs.slice(-4) as m (m.id)}
          <button class="line" data-hover-animates onclick={() => open(g.s, g.c.id)}>
            <Avatar instance={g.s} userId={m.author_id} size={40} />
            <span class="txt">
              <span class="top"><span class="who">{g.s.name(m.author_id)}</span><time class="when">{when(m.created_at)}</time></span>
              <span class="text">{preview(g.s, m)}</span>
            </span>
          </button>
        {/each}
        {#if g.msgs.length > 4}<button class="more" onclick={() => open(g.s, g.c.id)}>And {g.msgs.length - 4} earlier</button>{/if}
        {#each unreadThreads(g.s, g.c.id) as t (t.id)}
          {@const u = g.s.threadUnread(t.id)}
          <button class="line thread" onclick={() => openThread(g.s, g.c.id, t.id)}>
            <span class="glyph" aria-hidden="true"><Icon name="thread" size={18} /></span>
            <span class="txt">
              <span class="top"><span class="who">{plainMentions(t.title, g.s.users)}</span>{#if t.resolved_at}<span class="when">Resolved</span>{/if}</span>
              <span class="text">{#if u.mention}<span class="at">Mentions you</span> · {/if}{u.count} unread {u.count === 1 ? 'reply' : 'replies'}</span>
            </span>
          </button>
        {/each}
        {#if rows[key(g.s, g.c.id)]?.error}
          <div class="line">
            <span class="text err" role="alert">{rows[key(g.s, g.c.id)]!.error}</span>
            <button class="btn quiet" onclick={() => loadRows(g.s, g.c.id, rows[key(g.s, g.c.id)]!.appending)}>Retry</button>
          </div>
        {:else if rows[key(g.s, g.c.id)]?.more}
          <button class="more" onclick={() => loadRows(g.s, g.c.id, true)}>More conversations</button>
        {/if}
      </div>
    {/each}
    </div>
  </div>
</section>

<style>
  .inbox { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .head { display: flex; align-items: center; gap: 10px; padding: 10px var(--gutter); min-height: 52px; border-bottom: 1px solid var(--line); }
  .kind { color: var(--ink-3); display: grid; }
  h1 { font-size: var(--fs-title); margin: 0; font-weight: 600; }
  .spacer { flex: 1; }
  .scroll { flex: 1; overflow-y: auto; position: relative; }
  .body { max-width: 880px; margin: 0 auto; padding: 0 var(--gutter) calc(24px + env(safe-area-inset-bottom, 0px)); }
  @media (max-width: 600px) { .body { padding-inline: 16px; } }
  .empty { padding: 80px 20px; text-align: center; display: grid; justify-items: center; gap: 6px; }
  .empty-mark { color: var(--ink-3); margin-bottom: 10px; opacity: .8; }
  .big { font-size: 26px; }
  /* Section header: the room, which opens it, and how much is new. */
  .list-head { margin-top: 22px; text-transform: none; letter-spacing: 0; font-size: var(--fs-sm); }
  .section { display: inline-flex; align-items: center; gap: 4px; min-width: 0; min-height: 28px; margin: -4px 0; color: var(--ink-2); font-weight: 600; }
  .section .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .section .chev { display: grid; transform: rotate(-90deg); color: var(--ink-3); }
  .section:active { opacity: .5; }
  .new { flex: none; font-weight: 400; color: var(--ink-3); }
  .at { color: var(--lamp); font-weight: 600; }
  .line { width: 100%; display: flex; align-items: flex-start; gap: 12px; padding: 10px 16px; text-align: left; color: var(--ink); transition: background-color var(--t-fast); }
  @media (hover: hover) { .line:hover { background-color: var(--hover); } }
  .line:active { background-color: var(--press); }
  .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .top { display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .who { flex: 1; min-width: 0; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .when { flex: none; font-size: var(--fs-sm); color: var(--ink-3); font-variant-numeric: tabular-nums; }
  .text { color: var(--ink-2); font-size: var(--fs-sm); line-height: 1.35; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; overflow: hidden; overflow-wrap: break-word; }
  .glyph { display: grid; place-items: center; width: 40px; height: 40px; flex: none; border-radius: 50%; background: var(--bg-3); color: var(--lamp); }
  .more { display: block; width: 100%; min-height: 44px; padding: 8px 16px 8px 68px; text-align: left; color: var(--lamp); font-size: var(--fs-sm); }
  .more:active { background-color: var(--press); }
  /* Hairlines start at the text, as on the home list. */
  .slist > .line + *, .slist > .more + * { background-size: calc(100% - 68px) 1px; background-position: 68px 0; }
  .err { color: var(--danger, var(--lamp)); flex: 1; }
</style>
