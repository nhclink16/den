<script lang="ts">
  import { call } from '../lib/call.svelte'
  import SidebarToggle from './SidebarToggle.svelte'
  import CallDock from './CallDock.svelte'
  import { store, instances } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'
  import { native } from '../lib/native'
  import { desktop } from '../lib/desktop.svelte'
  import ServerSwitcher from './ServerSwitcher.svelte'
  import Mark from './Mark.svelte'

  let { narrow = false }: { narrow?: boolean } = $props()
  const inboxCount = $derived(instances.totalUnread)
  const uncategorized = $derived(store.textChannels.filter((c) => c.kind !== 'voice' && !c.category_id))
  const active = (id: string) => router.route.name === 'channel' && router.route.id === id
  const go = (path: string) => (e: MouseEvent) => { e.preventDefault(); router.go(path) }
</script>

<nav class="side" class:mac={desktop.platform === 'macos'}>
  <div class="brand">
    {#if native}<ServerSwitcher />{:else}<a href="/" class="display wordmark" onclick={go('/')}><Mark size={22} /><span title={store.settings.instance_name}>{store.settings.instance_name}</span></a>{/if}
    <span class="conn" class:off={!store.connected} role="img" aria-label={store.connected ? 'Connected' : 'Offline, reconnecting'} title={store.connected ? 'Connected' : 'Reconnecting'}></span>
    {#if !narrow}<SidebarToggle hide />{/if}
  </div>

  <a href="/inbox" class="row inbox" class:active={router.route.name === 'inbox'} class:lit={inboxCount > 0} onclick={go('/inbox')}>
    <Icon name="inbox" /> <span>Inbox</span>
    {#if inboxCount}<span class="count">{inboxCount}</span>{/if}
  </a>

  {#if !store.connected}
    <p class="offline" role="status">You're offline. Den is trying to reconnect.</p>
  {/if}
  <div class="scroll">
    {#snippet channelRow(c: import('../lib/types').Channel)}
      {@const u = store.unread(c.id)}
      {#if c.kind === 'voice'}
        <button class="row voice" class:active={call.origin === store.origin && call.channel?.id === c.id} title={`Join ${c.name}`} aria-label={`Join ${c.name}`} onclick={() => call.join(c)}>
          <Icon name="headset" />
          <span class="voice-name"><span class="name">{c.name}</span>
            {#if call.ids(c.id).length}
              <span class="avatars">{#each call.ids(c.id).slice(0,5) as id (id)}<span><Avatar userId={id} size={20} /></span>{/each}{#if call.ids(c.id).length > 5}<small>+{call.ids(c.id).length - 5}</small>{/if}</span>
            {/if}
          </span>
          {#if call.joining === c.id}<span class="faint mono">…</span>{/if}
        </button>
      {:else}
      <a href="/c/{c.id}" class="row" class:active={active(c.id)} class:lit={u.count > 0} onclick={go(`/c/${c.id}`)}>
        {#if c.kind === 'dm'}
          {@const other = (c.member_ids || []).find((id) => id !== store.me?.id) || store.me?.id || ''}
          <Avatar userId={other} size={20} />
        {:else}
          <Icon name="hash" />
        {/if}
        <span class="name">{store.title(c)}</span>
        {#if store.uploads.active(c.id)}<span class="uploading" title="Uploading files" aria-label="Uploading files">↑</span>{/if}
        {#if u.mention}<span class="count at">@</span>{:else if u.count}<span class="count">{u.count}</span>{/if}
      </a>
      {/if}
    {/snippet}

    {#each store.textChannels.filter(c => c.kind === 'voice').sort((a, b) => a.position - b.position) as c (c.id)}{@render channelRow(c)}{/each}

    {#each uncategorized as c (c.id)}{@render channelRow(c)}{/each}

    {#each store.categories as cat (cat.id)}
      {@const chans = store.textChannels.filter((c) => c.kind !== 'voice' && c.category_id === cat.id)}
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

  {#if desktop.updateReady}<button class="btn quiet mono" onclick={() => desktop.restart()}>Update ready · Restart</button>{/if}
  <CallDock />
  <div class="me">
    {#if store.me}
      <Avatar userId={store.me.id} size={28} />
      <span class="name">{store.me.display_name || store.me.username}</span>
      <a href="/settings" class="gear" class:active={router.route.name === 'settings'} title="Settings" onclick={go('/settings')}><Icon name="gear" /></a>
    {/if}
  </div>
</nav>

<style>
  .mac .brand { padding-top: 38px; }
  .side { height: 100%; display: flex; flex-direction: column; }
  .brand { display: flex; align-items: center; gap: 8px; padding: 14px 16px 8px; }
  .wordmark { font-size: 22px; color: var(--ink); display: inline-flex; align-items: center; gap: 7px; }
  .wordmark:hover { text-decoration: none; color: var(--lamp); }
  .conn { width: 7px; height: 7px; border-radius: 50%; background: var(--moss); margin-top: 2px; }
  .conn.off { background: var(--danger); }
  /* A blink that reaches 0.3 opacity reads as broken rendering; and the global
     reduced-motion reset only shortens the duration, so guard the rule itself. */
  @media (prefers-reduced-motion: no-preference) {
    .conn.off { animation: blink 1.4s infinite alternate; }
  }
  @keyframes blink { to { opacity: 0.62; } }
  .offline {
    margin: 0 10px 6px; padding: 7px 10px; border-radius: var(--r);
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--ink); font-size: 12.5px; line-height: 1.35;
  }
  .scroll { flex: 1; overflow-y: auto; padding: 4px 8px 8px; }
  .cat { padding: 14px 10px 4px; user-select: none; }
  .row {
    display: flex; align-items: center; gap: 8px; padding: calc(6px * var(--density)) 10px; margin: 1px 0;
    border-radius: var(--r); color: var(--ink-2); font-size: 14.5px;
  }
  .row:hover { background: var(--bg-3); color: var(--ink); text-decoration: none; }
  .row.active { background: var(--bg-3); color: var(--ink); }
  .row.lit { color: var(--ink); font-weight: 700; }
  .row .name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .row :global(svg) { color: var(--ink-3); flex: none; }
  .row.lit :global(svg), .row.active :global(svg) { color: var(--ink-2); }
  .wordmark { min-width: 0; }
  .wordmark span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .wordmark :global(svg) { flex-shrink: 0; }
  .voice { width: 100%; text-align: left; }
  .voice-name { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 5px; }
  .voice.active :global(svg) { color: var(--lamp); }
  .avatars { display: flex; padding-left: 6px; padding-bottom: 2px; align-items: center; }
  .avatars > span { margin-left: -6px; display: flex; border-radius: 35%; box-shadow: 0 0 0 2px var(--bg-2); }
  .avatars small { margin-left: 6px; font: 11px var(--mono); color: var(--ink-2); }
  .inbox { margin: 4px 8px 0; }
  .uploading { font: 16px var(--mono); color: var(--lamp); }
  .count {
    font-family: var(--mono); font-size: 11px; font-weight: 500; line-height: 1;
    padding: 3px 6px; border-radius: 999px; background: var(--lamp); color: var(--bg);
  }
  .count.at { font-weight: 700; }
  .me { display: flex; align-items: center; gap: 10px; padding: 10px 14px; border-top: 1px solid var(--line); }
  .me .name { flex: 1; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .gear { color: var(--ink-3); display: grid; padding: 4px; border-radius: var(--r); }
  .gear:hover, .gear.active { color: var(--ink); background: var(--bg-3); }
</style>
