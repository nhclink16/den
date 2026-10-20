<script lang="ts">
  import type { Theme } from '../lib/types'
  import { themes } from '../lib/theme.svelte'
  import { fontStack } from '../lib/theme-runtime'
  import { resolveAccent } from '../lib/theme-color'
  import ThemePreview from './ThemePreview.svelte'
  import InlineConfirm from './InlineConfirm.svelte'

  let { theme, lightChosen, darkChosen, dimmed, accent, onpick, oncustomise, onremove }: {
    theme: Theme
    /** The chosen accent, so the card shows the theme as it would look. */
    accent?: string | null
    lightChosen: boolean
    darkChosen: boolean
    /** 'light' or 'dark' when the mode is fixed, so the other half reads as inactive. */
    dimmed?: 'light' | 'dark'
    onpick: (half: 'light' | 'dark') => void
    oncustomise?: () => void
    onremove?: () => void
  } = $props()

  const halves = ['light', 'dark'] as const
  const chosen = (half: 'light' | 'dark') => (half === 'light' ? lightChosen : darkChosen)
  // Clicking the name or the card's edges picks the theme for the appearance on
  // screen now. The two previews stay the precise, keyboard-reachable choice.
  function pickCard(e: MouseEvent) {
    if ((e.target as HTMLElement).closest('button, details, a, input, .tools')) return
    onpick(themes.half)
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions (the preview buttons are the keyboard path) -->
<div class="card" class:active={lightChosen || darkChosen} onclick={pickCard}>
  <div class="previews">
    {#each halves as half (half)}
      <button
        class="half"
        class:chosen={chosen(half)}
        class:dim={dimmed === (half === 'light' ? 'dark' : 'light')}
        aria-pressed={chosen(half)}
        title={`Use ${theme.name} for ${half}`}
        aria-label={`Use ${theme.name} for ${half} appearance`}
        onclick={() => onpick(half)}
      >
        <ThemePreview colors={{ ...theme[half], accent: resolveAccent(theme[half], accent, half) }} fonts={theme.fonts} />
        <span class="badge" aria-hidden="true">
          {#if half === 'light'}
            <svg viewBox="0 0 16 16" width="10" height="10" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
              <circle cx="8" cy="8" r="3.1" /><path d="M8 1.4v1.6M8 13v1.6M1.4 8h1.6M13 8h1.6M3.3 3.3l1.1 1.1M11.6 11.6l1.1 1.1M3.3 12.7l1.1-1.1M11.6 4.4l1.1-1.1" />
            </svg>
          {:else}
            <svg viewBox="0 0 16 16" width="10" height="10" fill="currentColor">
              <path d="M13.3 9.6A5.7 5.7 0 0 1 6.4 2.7a5.8 5.8 0 1 0 6.9 6.9Z" />
            </svg>
          {/if}
        </span>
      </button>
    {/each}
  </div>
  <div class="caption">
    <span class="name" style={`font-family:${fontStack(theme.fonts.display, 'display')}`}>{theme.name}</span>
    <span class="tools">
      {#if oncustomise}
        <button class="tool" title={`Edit ${theme.name}`} aria-label={`Edit ${theme.name}`} onclick={oncustomise}>
          <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"><path d="M11 2.2 13.8 5 5.6 13.2l-3.4.6.6-3.4z" /></svg>
        </button>
      {/if}
      {#if onremove}
        <InlineConfirm action="Delete" sentence={`Delete the ${theme.name} theme? This cannot be undone.`} confirm={async () => onremove()} />
      {/if}
    </span>
  </div>
</div>

<style>
  /* Sits in a settings list: no box of its own, just the two halves and a name. */
  .card { cursor: pointer; min-width: 0; }
  .previews { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .half {
    position: relative; aspect-ratio: 8 / 5; overflow: hidden; padding: 0;
    border-radius: calc(var(--r) + 1px); border: 1px solid var(--line);
    transition: transform .12s, box-shadow .12s, opacity .15s;
  }
  @media (hover: hover) { .half:hover { transform: translateY(-1px); } }
  .half:active { transform: scale(.98); }
  .half.dim { opacity: .5; }
  .half.chosen { box-shadow: 0 0 0 2px var(--accent); border-color: transparent; opacity: 1; }
  .badge {
    position: absolute; right: 4px; bottom: 4px; display: none;
    width: 17px; height: 17px; border-radius: 50%; place-items: center;
    background: var(--accent); color: var(--on-lamp);
  }
  .half.chosen .badge { display: grid; }
  .caption { display: flex; align-items: center; justify-content: space-between; gap: 6px; padding: 6px 2px 0; min-height: 30px; }
  .name { font-size: var(--fs-sm); font-weight: 500; color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .card.active .name { color: var(--ink); font-weight: 700; }
  .tools { display: flex; gap: 2px; opacity: 0; transition: opacity .12s; }
  .card:focus-within .tools { opacity: 1; }
  @media (hover: hover) { .card:hover .tools { opacity: 1; } }
  .tool { display: grid; place-items: center; width: 26px; height: 26px; border-radius: var(--r); color: var(--ink3); }
  @media (hover: hover) { .tool:hover { background: var(--bg3); color: var(--ink); } }
  .tool:active { background: var(--press); }
  .tools :global(.btn) { min-height: 26px; padding: 2px 8px; font-size: var(--fs-xs); }
  @media (max-width: 650px) { .tools { opacity: 1; } }
</style>
