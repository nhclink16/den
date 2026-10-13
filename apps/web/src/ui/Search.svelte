<script lang="ts">
  import SidebarToggle from './SidebarToggle.svelte'
  import { instances, store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { goToMessage } from '../lib/notify.svelte'
  import type { Message } from '../lib/types'
  import { render } from '../lib/markdown'
  import { klipyGif } from '../lib/klipy'
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
  // The same resolver notifications and root badges use: a result that is a
  // reply opens its conversation and reveals the message, not just its room.
  // The concrete owner, not the proxy: goToMessage's lifetime check requires a
  // real instance and the exported proxy is not one.
  function open(m: Message) { void goToMessage(instances.active, m.channel_id, m.id) }
</script>

<!-- Not `.search` or `.bar`: app.css quiets the ring of an input inside those shells,
     and the query box here is the field itself. -->
<section class="search-page">
  <header class="head">
    {#if !narrow && !store.layout.sidebar}<SidebarToggle />{/if}
    {#if narrow}<button class="icon-btn" onclick={onmenu} aria-label="Menu"><Icon name="menu" /></button>{/if}
    <span class="kind"><Icon name="search" size={18} /></span>
    <form class="find" onsubmit={submit}>
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
        <button class="hit" data-hover-animates onclick={() => open(m)}>
          <div class="where faint">{c ? (c.kind === 'dm' ? store.title(c) : `#${c.name}`) : ''} · {dayLabel(m.created_at)} {shortTime(m.created_at)}</div>
          <div class="line">
            <Avatar userId={m.author_id} size={24} />
            <span class="who">{store.name(m.author_id)}</span>
            <span class="text">{@html klipyGif(m.content) ? '<i>sent a GIF</i>' : render(m.content, store.users) || '<i>sent a file</i>'}</span>
          </div>
        </button>
      {/each}
    {/if}
  </div>
</section>

<style>
  .search-page { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .head { display: flex; align-items: center; gap: 10px; padding: 10px 16px; min-height: 52px; border-bottom: 1px solid var(--line); }
  .kind { color: var(--ink-3); display: grid; }
  .find { flex: 1; display: flex; gap: 8px; min-width: 0; }
  /* The same search field as the room header and Settings; app.css draws its focus ring. */
  .q, .scope { min-height: var(--control-height); background: var(--bg-3); border: 1px solid transparent; border-radius: var(--r-pill); }
  .q { flex: 1; min-width: 0; padding: 6px 14px; }
  .q:focus-visible, .scope:focus-visible { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-glow); }
  .scope { padding: 6px 12px; color: var(--ink-2); max-width: 160px; }
  .scroll { flex: 1; overflow-y: auto; padding: 12px 16px 24px; }
  .empty { padding: 50px 20px; text-align: center; display: grid; gap: 6px; }
  .big { font-size: 24px; }
  .count { padding: 4px 4px 10px; }
  .hit { width: 100%; text-align: left; padding: 10px 12px; border-radius: var(--r); color: var(--ink); border: 1px solid transparent; }
  .hit:hover { background: var(--bg-2); border-color: var(--line); }
  .where { font-size: var(--fs-xs); margin-bottom: 4px; }
  .line { display: flex; align-items: flex-start; gap: 8px; }
  .who { font-weight: 700; flex: none; }
  .text { min-width: 0; max-width: 68ch; overflow-wrap: anywhere; }
  @media (max-width: 600px) { .head { align-items: flex-start; } .find { flex-wrap: wrap; } .q { flex: 1 0 100%; width: 100%; } .scope { flex: 1; max-width: none; } }
  .error { color: var(--ember); }
</style>
