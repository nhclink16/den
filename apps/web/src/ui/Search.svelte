<script module lang="ts">
  // Recent searches, on this device and for this account only: a convenience, so a missing or blocked
  // storage just means none.
  const key = (who: string) => `den.recentSearches.${who}`
  function readRecent(who: string): string[] {
    try { const v = JSON.parse(localStorage.getItem(key(who)) || '[]'); return Array.isArray(v) ? v.filter((x) => typeof x === 'string') : [] } catch { return [] }
  }
  function writeRecent(who: string, list: string[]) { try { localStorage.setItem(key(who), JSON.stringify(list)) } catch { /* not kept */ } }
</script>

<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import SidebarToggle from './SidebarToggle.svelte'
  import HomeButton from './HomeButton.svelte'
  import { instances, store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { goToMessage } from '../lib/notify.svelte'
  import type { Channel, Message } from '../lib/types'
  import { plainMentions } from '../lib/markdown'
  import { previewText } from '../lib/room-list'
  import { dayLabel, shortTime } from '../lib/time'
  import { native } from '../lib/native'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'

  let { q, channelId, narrow }: { q: string; channelId?: string; narrow: boolean } = $props()
  // svelte-ignore state_referenced_locally
  let text = $state(q)
  // svelte-ignore state_referenced_locally
  let scope = $state(channelId || '')
  let results = $state<Message[] | null>(null)
  let error = $state('')
  const who = `${store.origin}:${store.me?.id ?? ''}`
  let recent = $state(readRecent(who))
  let input = $state<HTMLInputElement>()
  const scopeName = $derived(scope ? store.channel(scope) : undefined)
  const bar = $derived(narrow && !native)
  const places = $derived(store.channels.filter((c) => c.kind !== 'voice'))
  const placeName = (c: Channel) => (c.kind === 'dm' ? store.title(c) : `#${c.name}`)

  // The URL is the search. It follows the field, trimmed, so it only overwrites the field when it changed
  // some other way (Back, a recent search): "hello " mid-word must stay "hello ".
  $effect(() => {
    if (untrack(() => text.trim()) !== q) text = q
    scope = channelId || ''
    run(q, channelId)
  })

  // Only the newest search may fill the list: an older, slower one must not land on top of it.
  let seq = 0
  async function run(query: string, chan?: string) {
    const mine = ++seq
    if (!query.trim()) { results = null; error = ''; return }
    try {
      const found = await store.search(query.trim(), chan || undefined)
      if (mine === seq) { results = found; error = '' }
    } catch (e) { if (mine === seq) { error = (e as Error).message; results = [] } }
  }
  // Results come as you type: the URL follows the field a moment after the last key, in place.
  let timer: ReturnType<typeof setTimeout> | undefined
  const target = () => `/find?q=${encodeURIComponent(text.trim())}${scope ? `&in=${scope}` : ''}`
  function typed() {
    clearTimeout(timer)
    timer = setTimeout(() => router.go(target(), true), 300)
  }
  $effect(() => () => clearTimeout(timer))
  function remember(query: string) {
    const t = query.trim()
    if (t) writeRecent(who, recent = [t, ...recent.filter((x) => x !== t)].slice(0, 8))
  }
  function submit(e: SubmitEvent) {
    e.preventDefault()
    clearTimeout(timer)
    remember(text)
    router.go(target(), true)
    input?.blur()
  }
  function pick(id: string) { scope = id; clearTimeout(timer); router.go(target(), true) }
  function again(query: string) { text = query; remember(query); router.go(target(), true) }
  function clear() { text = ''; clearTimeout(timer); router.go(target(), true); input?.focus() }
  // Cancel goes back where Search was opened from (the list, or a room), or to the list when it was opened
  // any other way.
  function cancel() {
    if (router.parent) router.back()
    else router.go('/', true)
  }
  // The same resolver notifications and root badges use: a result that is a
  // reply opens its conversation and reveals the message, not just its room.
  // The concrete owner, not the proxy: goToMessage's lifetime check requires a
  // real instance and the exported proxy is not one.
  function open(m: Message) { remember(q); void goToMessage(instances.active, m.channel_id, m.id) }

  // Opening Search is asking to type: the field is ready, as on an iPhone. Coming back to results isn't.
  onMount(() => { if (!q) input?.focus() })

  // The words found, in bold, inside a plain preview of the message.
  const terms = $derived(q.trim().split(/\s+/).filter(Boolean).map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')))
  function parts(m: Message) {
    const plain = plainMentions(previewText({ content: m.content, attachment_types: m.attachments.map((a) => a.content_type), object_kinds: (m.objects ?? []).map((o) => o.kind) }), store.users)
    if (!terms.length) return [{ t: plain, hit: false }]
    return plain.split(new RegExp(`(${terms.join('|')})`, 'gi')).map((t, i) => ({ t, hit: i % 2 === 1 }))
  }
</script>

<!-- Not `.search` or `.bar`: app.css quiets the ring of an input inside those shells,
     and the query box here is the field itself. -->
<section class="search-page" class:bar>
  <!-- `head`: in the desktop app this row is what drags the window (app.css). -->
  <header class="head top">
    {#if !narrow && !store.layout.sidebar}<SidebarToggle />{/if}
    {#if narrow && !bar}<HomeButton />{/if}
    <form class="find" role="search" onsubmit={submit}>
      <label class="q">
        <Icon name="search" size={17} />
        <input bind:this={input} bind:value={text} oninput={typed} placeholder="Search" aria-label="Search messages" enterkeyhint="search" autocomplete="off" />
        {#if text}<button type="button" class="clear" aria-label="Clear search" onclick={clear}><Icon name="x" size={12} /></button>{/if}
      </label>
    </form>
    {#if bar}<button class="cancel" onclick={cancel}>Cancel</button>{/if}
  </header>
  <!-- Where to look: everywhere, or one room or DM. -->
  <div class="chips" role="group" aria-label="Where">
    <button class="chip" aria-pressed={!scope} onclick={() => pick('')}>Everywhere</button>
    {#each places as c (c.id)}<button class="chip" aria-pressed={scope === c.id} onclick={() => pick(c.id)}>{placeName(c)}</button>{/each}
  </div>

  <div class="scroll">
    {#if error}<p class="error">{error}</p>{/if}
    {#if results === null}
      {#if recent.length}
        <h2 class="head-row"><span>Recent</span><button class="link" onclick={() => writeRecent(who, recent = [])}>Clear</button></h2>
        <div class="slist">
          {#each recent as r (r)}<button class="nav-row recent" onclick={() => again(r)}><Icon name="search" size={16} /><span class="grow">{r}</span></button>{/each}
        </div>
        <p class="list-foot">Words are matched exactly. Several words means all of them.</p>
      {:else}
        <div class="empty">
          <Icon name="search" size={36} />
          <div class="title">Search messages</div>
          <div class="muted">Words are matched exactly. Several words means all of them.</div>
        </div>
      {/if}
    {:else if !results.length && !error}
      <div class="empty"><div class="title">No results for “{q}”</div><div class="muted">{scopeName ? `Try Everywhere instead of ${placeName(scopeName)}.` : 'Search matches whole words. Try fewer words.'}</div></div>
    {:else if results.length}
      <h2 class="head-row">{results.length}{results.length === 50 ? '+' : ''} results</h2>
      <div class="slist">
        {#each results as m (m.id)}
          {@const c = store.channel(m.channel_id)}
          <button class="hit" data-hover-animates onclick={() => open(m)}>
            <Avatar userId={m.author_id} size={40} />
            <span class="txt">
              <span class="line1">
                <span class="who">{store.name(m.author_id)}</span>
                {#if c}<span class="where">{placeName(c)}</span>{/if}
                <time class="when">{dayLabel(m.created_at) === 'Today' ? shortTime(m.created_at) : dayLabel(m.created_at)}</time>
              </span>
              <span class="text">{#each parts(m) as p, i (i)}{#if p.hit}<b>{p.t}</b>{:else}{p.t}{/if}{/each}</span>
            </span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</section>

<style>
  .search-page { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .top { display: flex; align-items: center; gap: 10px; padding: 10px var(--gutter) 6px; min-height: 52px; }
  .bar .top { gap: 4px; padding: 4px 8px 4px 16px; min-height: 44px; }
  .find { flex: 1; min-width: 0; }
  .search-page:not(.bar) .find { max-width: 640px; }
  /* The iPhone's search field: a grey capsule with the magnifier inside and a clear button. */
  .q { display: flex; align-items: center; gap: 6px; min-height: 36px; padding: 0 8px 0 10px; border: 1px solid transparent; border-radius: 10px; background: var(--bg-3); color: var(--ink-3); cursor: text; }
  :global(:where(:root:not([data-input='touch']))) .q:focus-within { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-glow); }
  .q input { flex: 1; min-width: 0; padding: 7px 0; background: none; border: 0; outline: 0; color: var(--ink); font-size: 16px; }
  .q input:focus-visible { box-shadow: none; }
  .clear { display: grid; place-items: center; width: 18px; height: 18px; flex: none; border-radius: 50%; background: var(--ink-3); color: var(--bg); }
  .cancel { min-height: 44px; padding: 0 8px; color: var(--lamp); font-size: 17px; }
  .cancel:active { opacity: .5; }
  .chips { display: flex; gap: 8px; padding: 6px var(--gutter) 10px; overflow-x: auto; scrollbar-width: none; flex: none; border-bottom: 1px solid var(--line); }
  .bar .chips { padding-inline: 16px; }
  .chips::-webkit-scrollbar { display: none; }
  .chip { flex: none; min-height: 32px; padding: 4px 14px; border-radius: var(--r-pill); background: var(--bg-3); color: var(--ink-2); font-size: var(--fs-sm); font-weight: 600; white-space: nowrap; }
  .chip[aria-pressed=true] { background: var(--lamp); color: var(--on-lamp); }
  .chip:active { opacity: .7; }
  .scroll { flex: 1; overflow-y: auto; position: relative; padding: 0 var(--gutter) calc(24px + env(safe-area-inset-bottom, 0px)); }
  .bar .scroll { padding-inline: 16px; }
  .scroll > :global(*) { max-width: 760px; }
  .head-row { display: flex; align-items: center; justify-content: space-between; margin: 20px 4px 7px; font: 600 var(--fs-sm)/1.3 var(--body); color: var(--ink-2); }
  .link { color: var(--lamp); font-weight: 400; font-size: var(--fs-sm); min-height: 32px; }
  .recent { color: var(--ink); }
  .recent :global(svg) { color: var(--ink-3); flex: none; }
  .empty { padding: 56px 20px; text-align: center; display: grid; justify-items: center; gap: 6px; color: var(--ink-3); }
  .empty .title { margin-top: 6px; font-size: var(--fs-title); font-weight: 600; color: var(--ink); }
  .empty .muted { max-width: 32ch; text-wrap: pretty; }
  /* A result: who, where and when on one line, two lines of the message with the words found in bold. */
  .hit { width: 100%; display: flex; align-items: flex-start; gap: 12px; padding: 10px 16px; text-align: left; color: var(--ink); transition: background-color var(--t-fast); }
  @media (hover: hover) { .hit:hover { background-color: var(--hover); } }
  .hit:active { background-color: var(--press); }
  .slist > .hit + .hit { background-size: calc(100% - 68px) 1px; background-position: 68px 0; }
  .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .line1 { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
  .who { flex: none; max-width: 60%; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .where { flex: 1; min-width: 0; font-size: var(--fs-sm); color: var(--ink-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .when { flex: none; margin-inline-start: auto; font-size: var(--fs-sm); color: var(--ink-3); font-variant-numeric: tabular-nums; }
  .text { color: var(--ink-2); font-size: var(--fs-sm); line-height: 1.35; display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; line-clamp: 2; overflow: hidden; overflow-wrap: break-word; }
  .text b { color: var(--ink); font-weight: 700; }
  .error { color: var(--ember); }
</style>
