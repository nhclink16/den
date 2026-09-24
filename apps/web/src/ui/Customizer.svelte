<script lang="ts">
  // Appearance as a panel over the live app, like a theme preview: pick, and the
  // rooms behind it change. A bottom sheet on phones so the chat stays in view.
  import { customizer } from '../lib/customizer.svelte'
  import { themes } from '../lib/theme.svelte'
  import Appearance from './Appearance.svelte'
  import Icon from './Icon.svelte'

  let panel = $state<HTMLElement>()
  $effect(() => { if (customizer.open) panel?.focus() })
  // Escape closes it from anywhere, unless a dialog or popover above it takes the key.
  function key(e: KeyboardEvent) {
    if (!customizer.open || e.key !== 'Escape' || e.defaultPrevented) return
    if (document.querySelector('dialog[open], :popover-open')) return
    e.preventDefault(); customizer.close()
  }
</script>

<svelte:window onkeydown={key} />

{#if customizer.open}
  <div class="customizer" bind:this={panel} tabindex="-1" role="dialog" aria-modal="false" aria-labelledby="customizer-title">
    <header>
      <div class="title">
        <h2 id="customizer-title" class="display">Appearance</h2>
        <p class="faint small">{themes.saving ? 'Saving…' : 'Changes show live and save as you go.'}</p>
      </div>
      <button class="btn quiet icon" aria-label="Done" onclick={() => customizer.close()}><Icon name="x" /></button>
    </header>
    <div class="body"><Appearance panel /></div>
    <footer>
      <button class="btn quiet" disabled={!customizer.changed} onclick={() => customizer.undo()}>Undo changes</button>
      <button class="btn lit" onclick={() => customizer.close()}>Done</button>
    </footer>
  </div>
{/if}

<style>
  .customizer {
    position: fixed; inset: 0 0 0 auto; z-index: 40; width: min(400px, 100vw);
    display: grid; grid-template-rows: auto minmax(0, 1fr) auto;
    background: var(--bg-2); border-left: 1px solid var(--line-strong);
    box-shadow: -18px 0 50px -20px var(--shadow-lg); outline: none;
    container: panel / inline-size;
  }
  @media (prefers-reduced-motion: no-preference) { .customizer { animation: slide-in .26s var(--ease-out); } }
  @keyframes slide-in { from { transform: translateX(24px); opacity: 0; } }
  header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; padding: 16px 16px 12px; border-bottom: 1px solid var(--line); }
  h2 { margin: 0; font-size: 20px; }
  .small { margin: 2px 0 0; font-size: 12.5px; }
  .icon { padding: 6px; }
  .body { overflow-y: auto; padding: 0 16px; overscroll-behavior: contain; }
  footer { display: flex; justify-content: flex-end; gap: 8px; padding: 12px 16px; border-top: 1px solid var(--line); }

  /* Phones: a sheet over the bottom of the room, so the conversation stays visible. */
  @media (max-width: 650px) {
    .customizer {
      inset: auto 0 0 0; width: 100%; height: min(62vh, 560px);
      border-left: 0; border-top: 1px solid var(--line-strong); border-radius: var(--r-lg) var(--r-lg) 0 0;
      box-shadow: 0 -18px 50px -20px var(--shadow-lg);
    }
    @media (prefers-reduced-motion: no-preference) { .customizer { animation-name: sheet-in; } }
  }
  @keyframes sheet-in { from { transform: translateY(24px); opacity: 0; } }
</style>
