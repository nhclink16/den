<script lang="ts">
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { render } from '../lib/markdown'
  import { shortTime, dayLabel } from '../lib/time'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'

  let { onmenu, narrow }: { onmenu: () => void; narrow: boolean } = $props()

  const groups = $derived.by(() =>
    store.channels
      .map((c) => {
        const last = store.lastRead[c.id] || ''
        const msgs = (store.messages.get(c.id) || []).filter((m) => m.id > last && m.author_id !== store.me?.id)
        return { c, msgs, mention: store.unread(c.id).mention }
      })
      .filter((g) => g.msgs.length)
      .sort((a, b) => Number(b.mention) - Number(a.mention) || b.msgs.at(-1)!.id.localeCompare(a.msgs.at(-1)!.id)),
  )

  function open(id: string) { router.go(`/c/${id}`) }
  function clearAll() { for (const g of groups) store.markRead(g.c.id) }
</script>

<section class="inbox">
  <header class="head">
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
    {#each groups as g (g.c.id)}
      <div class="group" class:mention={g.mention}>
        <button class="title" onclick={() => open(g.c.id)}>
          {#if g.c.kind === 'dm'}<Icon name="lock" size={14} />{:else}<Icon name="hash" size={14} />{/if}
          <span class="display name">{store.title(g.c)}</span>
          <span class="faint mono">{g.msgs.length} new</span>
          {#if g.mention}<span class="at">mentions you</span>{/if}
        </button>
        {#each g.msgs.slice(-4) as m (m.id)}
          <button class="line" onclick={() => open(g.c.id)}>
            <Avatar userId={m.author_id} size={22} />
            <span class="who">{store.name(m.author_id)}</span>
            <span class="text">{@html render(m.content, store.users) || '<i>sent a file</i>'}</span>
            <span class="when faint mono">{dayLabel(m.created_at) === 'Today' ? shortTime(m.created_at) : dayLabel(m.created_at)}</span>
          </button>
        {/each}
        {#if g.msgs.length > 4}<button class="more faint" onclick={() => open(g.c.id)}>and {g.msgs.length - 4} earlier</button>{/if}
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
</style>
