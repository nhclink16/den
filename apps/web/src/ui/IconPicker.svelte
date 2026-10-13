<script lang="ts">
  // Admin: choose a room's icon from all of Lucide, or go back to the default.
  // The names load when the picker first opens, like the emoji data.
  import { tick } from 'svelte'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import { store } from '../lib/store.svelte'
  import { defaultIcon, loadIcons, roomIcon, searchIcons, type IconEntry } from '../lib/room-list'
  import type { Channel } from '../lib/types'
  import Icon from './Icon.svelte'
  import RoomIcon from './RoomIcon.svelte'

  let { channel }: { channel: Channel } = $props()
  let details = $state<HTMLDetailsElement>()
  let open = $state(false)
  let q = $state('')
  let icons = $state<IconEntry[]>([])
  let error = $state('')
  let busy = $state('')
  const results = $derived(searchIcons(icons, q))
  const current = $derived(roomIcon(channel))

  $effect(() => {
    if (!open) return
    error = ''
    if (!icons.length) loadIcons().then((list) => (icons = list), () => (error = "The icons didn't load. Check your connection and try again."))
    // The field exists before the popover shows, and a hidden field ignores focus.
    void tick().then(() => requestAnimationFrame(() => details?.querySelector<HTMLInputElement>('input')?.focus()))
  })

  function close() { open = false; q = ''; details?.querySelector('summary')?.focus() }
  async function pick(icon: string | null) {
    busy = icon ?? 'default'; error = ''
    try { await store.setChannelIcon(channel.id, icon); close() }
    catch (err) { error = (err as Error).message || "That icon didn't save." }
    finally { busy = '' }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes this native disclosure.) -->
<details bind:this={details} bind:open onkeydown={(e) => { if (e.key === 'Escape') { e.preventDefault(); close() } }}>
  <summary title="Change icon" aria-label={`Change the icon for ${channel.name}`}><RoomIcon {channel} name={current} size={34} /></summary>
  <div class="sheet" popover="auto" use:disclosurePopover={'end'}>
    {#if open}
      <div class="picker">
        <label class="search">
          <Icon name="search" size={14} />
          <input bind:value={q} placeholder="Search icons" aria-label="Search icons" spellcheck="false" autocomplete="off" />
          {#if q}<button class="clear" onclick={() => (q = '')} aria-label="Clear search"><Icon name="x" size={12} /></button>{/if}
        </label>
        <div class="results">
          {#if error}<p class="note" role="alert">{error}</p>{/if}
          {#if !icons.length && !error}
            <p class="note">Loading icons…</p>
          {:else if icons.length && !results.length}
            <p class="note">No icons for “{q.trim()}”. Try another word.</p>
          {:else}
            <div class="grid" role="group" aria-label={q ? 'Matching icons' : 'Suggested icons'}>
              {#each results as icon (icon.name)}
                <button class="cell" class:on={channel.icon === icon.name} aria-pressed={channel.icon === icon.name} title={icon.name} aria-label={icon.name.replace(/-/g, ' ')} disabled={!!busy} onclick={() => pick(icon.name)}>
                  <RoomIcon name={icon.name} size={44} />
                </button>
              {/each}
            </div>
            {#if !q}<p class="hint">Type to search all {icons.length.toLocaleString()} icons.</p>{/if}
          {/if}
        </div>
        <div class="foot">
          <button class="btn quiet small" disabled={!channel.icon || !!busy} onclick={() => pick(null)}>
            <RoomIcon name={defaultIcon(channel)} size={24} /> Use the default
          </button>
          <a class="by" href="https://lucide.dev" target="_blank" rel="noopener noreferrer">Icons by Lucide</a>
        </div>
      </div>
    {/if}
  </div>
</details>

<style>
  details { display: flex; }
  summary { list-style: none; cursor: pointer; border-radius: var(--avatar-r, 50%); }
  summary::-webkit-details-marker { display: none; }
  summary:hover :global(.tile) { color: var(--ink); }
  details[open] > summary :global(.tile) { color: var(--lamp); box-shadow: 0 0 0 2px var(--lamp); }
  [popover] { position: fixed; inset: auto; margin: 0; padding: 0; border: 1px solid var(--line-strong); border-radius: var(--r-lg); background: var(--bg-2); box-shadow: 0 14px 44px var(--shadow); overflow: hidden; }
  .picker { width: min(380px, calc(100vw - 16px)); height: min(440px, calc(100vh - 120px)); display: flex; flex-direction: column; }
  .search { display: flex; align-items: center; gap: 8px; margin: 10px 10px 8px; padding: 0 10px; height: 36px; border-radius: var(--r-pill); background: var(--bg-3); border: 1px solid transparent; color: var(--ink-3); }
  .search:focus-within { border-color: var(--lamp); box-shadow: 0 0 0 3px var(--lamp-glow); }
  .search input { flex: 1; min-width: 0; background: none; border: 0; outline: 0; color: var(--ink); }
  .search input::placeholder { color: var(--ink-3); }
  .search input:focus-visible { box-shadow: none; }
  .clear { display: grid; padding: 4px; border-radius: var(--r); color: var(--ink-3); }
  .clear:hover { color: var(--ink); background: var(--bg-2); }
  .results { flex: 1; min-height: 0; overflow-y: auto; padding: 0 8px 8px; overscroll-behavior: contain; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(48px, 1fr)); gap: 2px; }
  .cell { display: grid; place-items: center; height: 52px; border-radius: var(--r); }
  .cell :global(.tile) { background: none; color: var(--ink-2); }
  .cell:hover:not(:disabled) { background: var(--hover); }
  .cell:hover:not(:disabled) :global(.tile) { color: var(--ink); }
  .cell.on { background: var(--sel); }
  .cell.on :global(.tile) { color: var(--lamp); }
  .cell:disabled { cursor: default; }
  .note, .hint { margin: 24px 12px; text-align: center; color: var(--ink-2); }
  .hint { margin: 10px 0 4px; font-size: var(--fs-xs); color: var(--ink-3); }
  .foot { display: flex; justify-content: space-between; align-items: center; gap: 8px; padding: 6px 8px 6px 4px; border-top: 1px solid var(--line); }
  .foot .btn { min-height: 34px; padding: 4px 10px; font-size: var(--fs-sm); }
  .foot .btn :global(.tile) { background: none; }
  .by { font-size: var(--fs-xs); color: var(--ink-3); padding: 4px 8px; }
  @media (max-width: 600px) {
    .picker { width: 100%; height: min(520px, 70vh); }
    .search input { font-size: 16px; }
  }
  @media (pointer: coarse) { .search input { font-size: 16px; } }
</style>
