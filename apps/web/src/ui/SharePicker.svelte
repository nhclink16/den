<script lang="ts">
  import { sharePicker } from '../lib/share-picker.svelte'
  import { modal, outsideDialog } from '../lib/modal'
  import { desktop } from '../lib/desktop.svelte'

  const req = $derived(sharePicker.request)
  let tab = $state<'screen' | 'window'>('screen')
  let selectedId = $state<string | null>(null)
  let audio = $state(true)
  let dialog = $state<HTMLDialogElement | undefined>()

  $effect(() => {
    if (req) { tab = 'screen'; selectedId = null; audio = true }
  })

  const screens = $derived(req?.sources.filter(s => s.isScreen) ?? [])
  const windows = $derived(req?.sources.filter(s => !s.isScreen) ?? [])
  const current = $derived(tab === 'screen' ? screens : windows)

  $effect(() => {
    if (req && screens.length === 0 && windows.length > 0) tab = 'window'
  })

  $effect(() => {
    if (!req) return
    const id = req.requestId
    const t = setInterval(() => { sharePicker.refreshSources(id).catch(() => {}) }, 2000)
    return () => clearInterval(t)
  })

  function confirm() {
    if (!req || !selectedId) return
    sharePicker.choose(req.requestId, selectedId, showAudio ? audio : false).catch(() => {})
  }

  function cancel() {
    if (!req) return
    sharePicker.cancel(req.requestId).catch(() => {})
  }

  const showAudio = $derived(req?.platform === 'win32' && !!req?.audioRequested)

  function gridKeydown(e: KeyboardEvent) {
    const items = current
    if (!items.length) return
    const idx = selectedId ? items.findIndex(s => s.id === selectedId) : -1
    const cols = 3
    let next = idx
    if (e.key === 'ArrowRight') { e.preventDefault(); next = idx < 0 ? 0 : Math.min(idx + 1, items.length - 1) }
    else if (e.key === 'ArrowLeft') { e.preventDefault(); next = idx <= 0 ? 0 : idx - 1 }
    else if (e.key === 'ArrowDown') { e.preventDefault(); next = idx < 0 ? 0 : Math.min(idx + cols, items.length - 1) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); next = idx < 0 ? 0 : Math.max(idx - cols, 0) }
    else if (e.key === 'Enter') { e.preventDefault(); confirm(); return }
    else return
    if (next !== idx && items[next]) {
      selectedId = items[next].id
      ;(e.currentTarget as HTMLElement).querySelectorAll<HTMLElement>('[role="gridcell"]')[next]?.focus()
    }
  }
</script>

{#if req}
<dialog bind:this={dialog} use:modal class="picker" aria-label="Share your screen"
  oncancel={(e) => { e.preventDefault(); cancel() }}
  onclick={(e) => { if (outsideDialog(e)) cancel() }}>
  <div class="head">
    <span class="display">Share your screen</span>
    <div class="tabs" role="tablist">
      <button role="tab" aria-selected={tab === 'screen'} class:active={tab === 'screen'} onclick={() => { tab = 'screen'; selectedId = null }}>Screens</button>
      <button role="tab" aria-selected={tab === 'window'} class:active={tab === 'window'} onclick={() => { tab = 'window'; selectedId = null }}>Windows</button>
    </div>
  </div>

  <div class="grid" role="grid" tabindex="-1" aria-label={tab === 'screen' ? 'Screens' : 'Windows'} onkeydown={gridKeydown}>
    {#each current as source (source.id)}
      {@const selected = source.id === selectedId}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div role="gridcell" class="card" class:selected
        tabindex={selected || (!selectedId && current[0]?.id === source.id) ? 0 : -1}
        aria-selected={selected}
        onclick={() => selectedId = source.id}
        ondblclick={confirm}
        onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); selectedId = source.id; confirm() } }}>
        <div class="thumb">
          {#if source.thumbnail}
            <img src={source.thumbnail} alt={source.name} draggable="false" />
          {:else}
            <div class="no-thumb"></div>
          {/if}
          {#if source.appIcon && !source.isScreen}
            <img class="app-icon" src={source.appIcon} alt="" aria-hidden="true" draggable="false" />
          {/if}
        </div>
        <span class="label" title={source.name}>{source.name}</span>
      </div>
    {:else}
      <div class="empty muted">No {tab === 'screen' ? 'screens' : 'windows'} found</div>
    {/each}
  </div>

  <div class="foot">
    {#if showAudio}
      <label class="audio-toggle">
        <input type="checkbox" bind:checked={audio} />
        Share sound
      </label>
    {:else}
      <span></span>
    {/if}
    <div class="actions">
      <button class="btn quiet" onclick={cancel}>Cancel</button>
      <button class="btn lit" disabled={!selectedId} onclick={confirm}>Share</button>
    </div>
  </div>
</dialog>
{/if}

<style>
  .picker {
    width: min(760px, calc(100vw - 32px));
    max-height: min(600px, calc(100vh - 48px));
    padding: 0;
    border: 1px solid var(--line-strong);
    border-radius: var(--r-lg);
    background: var(--bg-2);
    color: var(--ink);
    box-shadow: 0 24px 80px -16px var(--shadow-lg), 0 0 0 1px var(--line);
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .picker::backdrop { background: var(--scrim); }
  .head {
    padding: 18px 20px 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-shrink: 0;
  }
  .head .display { font-size: 16px; }
  .tabs { display: flex; gap: 2px; background: var(--bg-3); border-radius: var(--r); padding: 2px; }
  .tabs button { padding: 4px 14px; border-radius: calc(var(--r) - 1px); font-size: 13px; color: var(--ink-2); }
  .tabs button.active { background: var(--bg-2); color: var(--ink); box-shadow: 0 1px 3px var(--shadow); }
  .tabs button:not(.active):hover { color: var(--ink); }
  .grid {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    align-content: start;
  }
  .empty { grid-column: 1 / -1; text-align: center; padding: 32px 0; }
  .card {
    border-radius: var(--r);
    border: 2px solid transparent;
    background: var(--bg-3);
    cursor: pointer;
    overflow: hidden;
    outline: none;
    transition: border-color var(--t-fast);
  }
  .card:hover { border-color: var(--line-strong); }
  .card.selected { border-color: var(--lamp); box-shadow: 0 0 0 2px var(--lamp-glow); }
  .card:focus-visible { border-color: var(--lamp); box-shadow: 0 0 0 2px var(--lamp-glow); }
  .thumb {
    position: relative;
    width: 100%;
    aspect-ratio: 16/9;
    background: var(--bg);
    overflow: hidden;
  }
  .thumb img:not(.app-icon) { width: 100%; height: 100%; object-fit: contain; display: block; }
  .no-thumb { width: 100%; height: 100%; background: var(--bg-3); }
  .app-icon {
    position: absolute;
    bottom: 6px;
    left: 6px;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    object-fit: contain;
  }
  .label {
    display: block;
    padding: 6px 8px;
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .foot {
    padding: 12px 20px;
    border-top: 1px solid var(--line);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-shrink: 0;
  }
  .audio-toggle { display: flex; align-items: center; gap: 8px; font-size: 13px; cursor: pointer; }
  .audio-toggle input { accent-color: var(--lamp); }
  .actions { display: flex; gap: 8px; }
</style>
