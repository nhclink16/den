<script lang="ts">
  import SidebarToggle from './SidebarToggle.svelte'
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import type { Message } from '../lib/types'
  import { render } from '../lib/markdown'
  import { dayLabel, shortTime } from '../lib/time'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'

  let { q, channelId, onmenu, narrow }: { q: string; channelId?: string; onmenu: () => void; narrow: boolean } = $props()
  // svelte-ignore state_referenced_locally
  let text = $state(q)
  // svelte-ignore state_referenced_locally
  let scope = $state(channelId || '')
  let results = $state<Message[] | null>(null)
  let error = $state('')
  const scopeName = $derived(scope ? store.channel(scope) : undefined)

  $effect(() => { text = q; scope = channelId || ''; run(q, channelId) })

  async function run(query: string, chan?: string) {
    if (!query.trim()) { results = null; return }
    error = ''
    try { results = await store.search(query.trim(), chan || undefined) } catch (e) { error = (e as Error).message; results = [] }
  }
  function submit(e: SubmitEvent) {
    e.preventDefault()
    router.go(`/find?q=${encodeURIComponent(text.trim())}${scope ? `&in=${scope}` : ''}`)
  }
  function open(m: Message) { router.go(`/c/${m.channel_id}`) }
</script>

<section class="search">
  <header class="head">
    {#if !narrow && !store.layout.sidebar}<SidebarToggle />{/if}
    {#if narrow}<button class="btn quiet iconbtn" onclick={onmenu} aria-label="Menu"><Icon name="menu" /></button>{/if}
    <span class="kind"><Icon name="search" size={18} /></span>
    <form class="bar" onsubmit={submit}>
      <input class="q" bind:value={text} placeholder="Search messages" aria-label="Search messages" />
      <select class="scope" bind:value={scope} aria-label="Where">
        <option value="">Everywhere</option>
        {#each store.channels as c (c.id)}<option value={c.id}>{c.kind === 'dm' ? store.title(c) : `#${c.name}`}</option>{/each}
      </select>
      <button class="btn lit" type="submit">Search</button>
    </form>
  </header>

  <div class="scroll">
    {#if error}<p class="error">{error}</p>{/if}
    {#if results === null}
      <div class="empty muted">Words are matched exactly. Several words means all of them.</div>
    {:else if !results.length}
      <div class="empty"><div class="display big">Nothing for “{q}”</div><div class="muted">{scopeName ? `Try searching everywhere instead of ${store.title(scopeName)}.` : 'Try fewer words.'}</div></div>
    {:else}
      <div class="eyebrow count">{results.length}{results.length === 50 ? '+' : ''} results</div>
      {#each results as m (m.id)}
        {@const c = store.channel(m.channel_id)}
        <button class="hit" onclick={() => open(m)}>
          <div class="where faint mono">{c ? (c.kind === 'dm' ? store.title(c) : `#${c.name}`) : ''} · {dayLabel(m.created_at)} {shortTime(m.created_at)}</div>
          <div class="line">
            <Avatar userId={m.author_id} size={24} />
            <span class="who">{store.name(m.author_id)}</span>
            <span class="text">{@html render(m.content, store.users) || '<i>sent a file</i>'}</span>
          </div>
        </button>
      {/each}
    {/if}
  </div>
</section>

<style>
  .search { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .head { display: flex; align-items: center; gap: 10px; padding: 10px 16px; min-height: 52px; border-bottom: 1px solid var(--line); }
  .kind { color: var(--ink-3); display: grid; }
  .iconbtn { padding: 6px; }
  .bar { flex: 1; display: flex; gap: 8px; min-width: 0; }
  .q { flex: 1; min-width: 0; background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r); padding: 6px 10px; outline: 0; }
  .q:focus { border-color: var(--lamp); }
  .scope { background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r); padding: 6px 8px; color: var(--ink-2); max-width: 160px; }
  .scroll { flex: 1; overflow-y: auto; padding: 12px 16px 24px; }
  .empty { padding: 50px 20px; text-align: center; display: grid; gap: 6px; }
  .big { font-size: 24px; }
  .count { padding: 4px 4px 10px; }
  .hit { width: 100%; text-align: left; padding: 10px 12px; border-radius: var(--r); color: var(--ink); border: 1px solid transparent; }
  .hit:hover { background: var(--bg-2); border-color: var(--line); }
  .where { font-size: 11px; margin-bottom: 4px; }
  .line { display: flex; align-items: flex-start; gap: 8px; }
  .who { font-weight: 700; flex: none; }
  .text { min-width: 0; overflow-wrap: anywhere; }
  .error { color: var(--ember); }
</style>
