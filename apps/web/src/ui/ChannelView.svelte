<script lang="ts">
  import { store } from '../lib/store.svelte'
  import type { Channel, Message } from '../lib/types'
  import Icon from './Icon.svelte'
  import MessageList from './MessageList.svelte'
  import Composer from './Composer.svelte'

  let { channel, onmenu, narrow }: { channel: Channel; onmenu: () => void; narrow: boolean } = $props()
  let replyTo = $state<Message | null>(null)
  let dragging = $state(0)
  let dropped = $state<File[]>([])

  $effect(() => { if (!store.messages.has(channel.id)) store.loadLatest(channel.id) })
  $effect(() => {
    // Mark read while we're looking at it and the tab is visible.
    void store.messages.get(channel.id)
    if (document.visibilityState === 'visible') store.markRead(channel.id)
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
    {#if narrow}<button class="btn quiet iconbtn" onclick={onmenu} aria-label="Menu"><Icon name="menu" /></button>{/if}
    <span class="kind">{#if isDm}<Icon name="lock" />{:else}<Icon name="hash" size={18} />{/if}</span>
    <h1 class="display">{store.title(channel)}</h1>
    <span class="spacer"></span>
    {#if !narrow}
      <button class="btn quiet iconbtn" title="Toggle people (Ctrl+Shift+M)" onclick={() => store.savePrefs({ members: !store.prefs.members })}><Icon name="people" /></button>
    {/if}
  </header>

  <MessageList {channel} onreply={(m) => (replyTo = m)} />

  <div class="typing" aria-live="polite">
    {#if typing.length}{typing.join(', ')} {typing.length === 1 ? 'is' : 'are'} typing{/if}
  </div>

  <Composer {channel} bind:replyTo bind:dropped />

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
  .view { flex: 1; min-height: 0; display: flex; flex-direction: column; position: relative; }
  .head {
    display: flex; align-items: center; gap: 10px; padding: 10px 16px; min-height: 52px;
    border-bottom: 1px solid var(--line);
  }
  .kind { color: var(--ink-3); display: grid; }
  h1 { font-size: 19px; margin: 0; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .spacer { flex: 1; }
  .iconbtn { padding: 6px; }
  .typing { height: 18px; padding: 0 20px; font-size: 12px; color: var(--ink-3); }
  .drop {
    position: absolute; inset: 8px; z-index: 5; border-radius: var(--r-lg);
    background: rgba(27, 25, 22, 0.9); border: 2px dashed var(--lamp);
    display: grid; place-items: center; pointer-events: none;
  }
  .drop-inner { display: grid; justify-items: center; gap: 6px; color: var(--lamp); }
  .big { font-size: 26px; }
</style>
