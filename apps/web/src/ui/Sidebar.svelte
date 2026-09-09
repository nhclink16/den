<script lang="ts">
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'

  const inboxCount = $derived(store.totalUnread)
  const uncategorized = $derived(store.textChannels.filter((c) => !c.category_id))
  const active = (id: string) => router.route.name === 'channel' && router.route.id === id
  const go = (path: string) => (e: MouseEvent) => { e.preventDefault(); router.go(path) }
</script>

<nav class="side">
  <div class="brand">
    <a href="/" class="display wordmark" onclick={go('/')}>Den</a>
    <span class="conn" class:off={!store.connected} title={store.connected ? 'Connected' : 'Reconnecting'}></span>
  </div>

  <a href="/inbox" class="row inbox" class:active={router.route.name === 'inbox'} class:lit={inboxCount > 0} onclick={go('/inbox')}>
    <Icon name="inbox" /> <span>Inbox</span>
    {#if inboxCount}<span class="count">{inboxCount}</span>{/if}
  </a>

  <div class="scroll">
    {#snippet channelRow(c: import('../lib/types').Channel)}
      {@const u = store.unread(c.id)}
      <a href="/c/{c.id}" class="row" class:active={active(c.id)} class:lit={u.count > 0} onclick={go(`/c/${c.id}`)}>
        {#if c.kind === 'dm'}
          {@const other = (c.member_ids || []).find((id) => id !== store.me?.id) || store.me?.id || ''}
          <Avatar userId={other} size={20} />
        {:else}
          <Icon name={c.kind === 'voice' ? 'people' : 'hash'} />
        {/if}
        <span class="name">{store.title(c)}</span>
        {#if u.mention}<span class="count at">@</span>{:else if u.count}<span class="count">{u.count}</span>{/if}
      </a>
    {/snippet}

    {#each uncategorized as c (c.id)}{@render channelRow(c)}{/each}

    {#each store.categories as cat (cat.id)}
      {@const chans = store.textChannels.filter((c) => c.category_id === cat.id)}
      {#if chans.length}
        <div class="cat eyebrow">{cat.name}</div>
        {#each chans as c (c.id)}{@render channelRow(c)}{/each}
      {/if}
    {/each}

    {#if store.dms.length}
      <div class="cat eyebrow">Direct</div>
      {#each store.dms as c (c.id)}{@render channelRow(c)}{/each}
    {/if}
  </div>

  <div class="me">
    {#if store.me}
      <Avatar userId={store.me.id} size={28} />
      <span class="name">{store.me.display_name || store.me.username}</span>
      <a href="/settings" class="gear" class:active={router.route.name === 'settings'} title="Settings" onclick={go('/settings')}><Icon name="gear" /></a>
    {/if}
  </div>
</nav>

<style>
  .side { height: 100%; display: flex; flex-direction: column; }
  .brand { display: flex; align-items: center; gap: 8px; padding: 14px 16px 8px; }
  .wordmark { font-size: 22px; color: var(--ink); }
  .wordmark:hover { text-decoration: none; color: var(--lamp); }
  .conn { width: 7px; height: 7px; border-radius: 50%; background: var(--moss); margin-top: 2px; }
  .conn.off { background: var(--ember); animation: blink 1s infinite alternate; }
  @keyframes blink { to { opacity: 0.3; } }
  .scroll { flex: 1; overflow-y: auto; padding: 4px 8px 8px; }
  .cat { padding: 14px 10px 4px; user-select: none; }
  .row {
    display: flex; align-items: center; gap: 8px; padding: 6px 10px; margin: 1px 0;
    border-radius: var(--r); color: var(--ink-2); font-size: 14.5px;
  }
  .row:hover { background: var(--bg-3); color: var(--ink); text-decoration: none; }
  .row.active { background: var(--bg-3); color: var(--ink); }
  .row.lit { color: var(--ink); font-weight: 700; }
  .row .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row :global(svg) { color: var(--ink-3); flex: none; }
  .row.lit :global(svg), .row.active :global(svg) { color: var(--ink-2); }
  .inbox { margin: 4px 8px 0; }
  .count {
    font-family: var(--mono); font-size: 11px; font-weight: 500; line-height: 1;
    padding: 3px 6px; border-radius: 999px; background: var(--lamp); color: #1b1916;
  }
  .count.at { font-weight: 700; }
  .me { display: flex; align-items: center; gap: 10px; padding: 10px 14px; border-top: 1px solid var(--line); }
  .me .name { flex: 1; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .gear { color: var(--ink-3); display: grid; padding: 4px; border-radius: var(--r); }
  .gear:hover, .gear.active { color: var(--ink); background: var(--bg-3); }
</style>
