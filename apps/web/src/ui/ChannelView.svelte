<script lang="ts">
  import SidebarToggle from './SidebarToggle.svelte'
  import ObjectDock from './ObjectDock.svelte'
  import { objects } from '../lib/objects.svelte'
  import { call } from '../lib/call.svelte'
  import CallView from './CallView.svelte'
  import { store } from '../lib/store.svelte'
  import type { Channel, Message } from '../lib/types'
  import Icon from './Icon.svelte'
  import MessageList from './MessageList.svelte'
  import Composer from './Composer.svelte'
  import { router } from '../lib/router.svelte'
  let q = $state('')
  let listening = $state(false)
  function search(e: SubmitEvent) { e.preventDefault(); if (q.trim()) router.go(`/find?q=${encodeURIComponent(q.trim())}&in=${channel.id}`) }

  let { channel, onmenu, narrow }: { channel: Channel; onmenu: () => void; narrow: boolean } = $props()
  let replyTo = $state<Message | null>(null)
  let dragging = $state(0)
  let dropped = $state<File[]>([])

  $effect(() => { if (channel.kind !== 'voice' && !store.messages.has(channel.id)) store.loadLatest(channel.id) })
  // Mark read while we're looking at it and the tab is visible, and again when it becomes visible.
  $effect(() => {
    void store.messages.get(channel.id)
    if (document.visibilityState === 'visible') store.markRead(channel.id)
  })
  $effect(() => {
    const fn = () => { if (document.visibilityState === 'visible') store.markRead(channel.id) }
    document.addEventListener('visibilitychange', fn)
    return () => document.removeEventListener('visibilitychange', fn)
  })
  const typing = $derived(store.typingNames(channel.id))
  const isDm = $derived(channel.kind === 'dm')

  function onDrop(e: DragEvent) {
    e.preventDefault(); dragging = 0
    const files = [...(e.dataTransfer?.files || [])]
    if (files.length) dropped = files
  }
</script>

<section class="view" aria-label={store.title(channel)} ondragenter={(e) => { e.preventDefault(); dragging++ }} ondragleave={() => dragging--} ondragover={(e) => e.preventDefault()} ondrop={onDrop}>
  <header class="head">
    {#if !narrow && !store.layout.sidebar}<SidebarToggle />{/if}
    {#if narrow}<button class="btn quiet iconbtn" onclick={onmenu} aria-label="Menu"><Icon name="menu" /></button>{/if}
    <span class="kind">{#if isDm}<Icon name="lock" />{:else}<Icon name={channel.kind === 'voice' ? 'headset' : 'hash'} size={18} />{/if}</span>
    <h1 class="display">{store.title(channel)}</h1>
    <span class="spacer"></span>
    {#if isDm}<button class="btn quiet dm-call" onclick={() => call.join(channel)} aria-label="Call"><Icon name="phone" /> <span>Call</span></button>{/if}
    <form class="search" onsubmit={search}><Icon name="search" size={14} /><input bind:value={q} placeholder={narrow ? 'Search' : isDm ? `Search ${store.title(channel)}` : `Search #${channel.name}`} aria-label={isDm ? 'Search this conversation' : 'Search this room'} /></form>
    {#if !narrow}
      <button class="btn quiet iconbtn" title="Toggle people (Ctrl+Shift+M)" onclick={() => store.saveLayout({ members: !store.layout.members })}><Icon name="people" /></button>
    {/if}
  </header>

  {#if call.channel && !objects.expanded}<CallView />{/if}
  {#if objects.active}<div class="object-slot" class:hidden={!!call.channel && call.expanded && !objects.expanded}><ObjectDock /></div>{/if}
  {#if (!call.channel || !call.expanded) && !objects.expanded}
  {#if channel.kind === 'voice'}
    <div class="voice-empty"><button class="btn lit" onclick={() => call.join(channel)}><Icon name="headset" /> Join {channel.name}</button></div>
  {:else}
  {#if isDm && call.channel?.id !== channel.id && call.ids(channel.id).length}
    <button class="call-banner" onclick={() => call.join(channel)}><Icon name="phone" /> {call.ids(channel.id).map((id) => store.name(id)).join(', ')} {call.ids(channel.id).length === 1 ? 'is' : 'are'} in a call · Join</button>
  {/if}
  <MessageList {channel} onreply={(m) => (replyTo = m)} />

  <div class="typing" aria-live="polite">
    {#if listening}<span class="mono">listening</span>{:else if typing.length}{typing.join(', ')} {typing.length === 1 ? 'is' : 'are'} typing{/if}
  </div>

  <Composer {channel} bind:replyTo bind:dropped bind:listening />
  {/if}
  {/if}

  {#if dragging > 0}
    <div class="drop">
      <div class="drop-inner">
        <Icon name="clip" size={28} />
        <span class="display big">Drop it here</span>
        <span class="muted">Anything up to 1 GB. Clips play right in the chat.</span>
      </div>
    </div>
  {/if}
</section>

<style>
  .object-slot { display: contents; }
  .object-slot.hidden { display: none; }
  .view { flex: 1; min-height: 0; display: flex; flex-direction: column; position: relative; }
  .head {
    display: flex; align-items: center; gap: 10px; padding: 10px var(--gutter); min-height: 52px;
    border-bottom: 1px solid var(--line);
  }
  .kind { color: var(--ink-3); display: grid; }
  h1 { font-size: 19px; margin: 0; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .spacer { flex: 1; }
  .iconbtn { padding: 6px; }
  .search { display: flex; align-items: center; gap: 6px; padding: 4px 10px; border: 1px solid transparent; border-radius: 999px; color: var(--ink-3); background: var(--bg-2); }
  .search:focus-within { border-color: var(--accent); color: var(--ink-2); box-shadow: 0 0 0 3px var(--accent-glow); }
  .search input { background: none; border: 0; outline: 0; width: 120px; font-size: 13px; color: var(--ink); transition: width 0.15s; }
  .search input:focus { width: 200px; }
  .search input::placeholder { color: var(--ink-3); }
  .call-banner { display: flex; align-items: center; gap: 8px; padding: 10px var(--gutter); color: var(--lamp); background: var(--bg-3); border-bottom: 1px solid var(--line); text-align: left; }
  .voice-empty { flex: 1; display: grid; place-items: center; }
  @media (max-width: 600px) { .head { gap: 6px; padding-inline: 10px; } .search input { width: 64px; } .search input:focus { width: 90px; } .dm-call { padding: 6px; } .dm-call span { display: none; } }
  .typing { height: 18px; padding: 0 20px; font-size: 12px; color: var(--ink-3); }
  .drop {
    position: absolute; inset: 8px; z-index: 5; border-radius: var(--r-lg);
    background: var(--overlay); border: 2px dashed var(--lamp);
    display: grid; place-items: center; pointer-events: none;
  }
  .drop-inner { display: grid; justify-items: center; gap: 6px; color: var(--lamp); }
  .big { font-size: 26px; }
</style>
