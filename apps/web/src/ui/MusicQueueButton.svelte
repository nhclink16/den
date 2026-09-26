<script lang="ts">
  import { call } from '../lib/call.svelte'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import Icon from './Icon.svelte'
  import MusicMenu from './MusicMenu.svelte'
  let open = $state(false)
  const live = $derived(!!call.channel && !!call.owner?.jams.has(call.channel.id))
</script>
{#if call.channel && call.channel.kind !== 'text' && call.owner}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes this native disclosure.) -->
  <details bind:open onkeydown={e => { if (e.key === 'Escape') { e.preventDefault(); open = false; e.currentTarget.querySelector('summary')?.focus() } }}>
    <summary class:live title={live ? 'Music · a Jam is playing' : 'Music'} aria-label="Music"><Icon name="music" /></summary>
    <div popover="auto" use:disclosurePopover>{#if open}<MusicMenu roomId={call.channel.id} owner={call.owner} />{/if}</div>
  </details>
{/if}
<style>
  summary { width: 28px; height: 28px; border-radius: 6px; display: grid; place-items: center; cursor: pointer; list-style: none; color: var(--ink); } summary::-webkit-details-marker { display: none; }
  summary.live { color: var(--lamp); }
  [popover] { position: fixed; inset: auto; margin: 0; padding: 0; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-2); box-shadow: 0 10px 36px var(--shadow); }
  @media (hover: hover) { summary:hover { background: color-mix(in srgb, var(--ink) 10%, transparent); } }
</style>
