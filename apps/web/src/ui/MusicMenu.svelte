<script lang="ts">
  // The call's Music menu: two ways to put music in a call. DM calls have no DJ,
  // so they only get the Jam.
  import type { Store } from '../lib/store.svelte'
  import MusicPanel from './MusicPanel.svelte'
  import JamPanel from './JamPanel.svelte'
  let { roomId, owner }: { roomId: string; owner: Store } = $props()
  const voice = $derived(owner.channel(roomId)?.kind === 'voice')
  const live = $derived(owner.jams.has(roomId))
  // Opens on the Jam when one is playing, since that is what people came to see.
  const initial = () => (owner.jams.has(roomId) ? 'jam' : 'youtube')
  let picked = $state<'youtube' | 'jam'>(initial())
  const choice = $derived(voice ? picked : 'jam')
</script>
<div class="music-menu" data-testid="music-menu">
  {#if voice}
    <div class="choices" role="group" aria-label="Music">
      <button aria-pressed={choice === 'youtube'} onclick={() => (picked = 'youtube')}><b>Play into the call</b><small>YouTube</small></button>
      <button aria-pressed={choice === 'jam'} onclick={() => (picked = 'jam')}><b>Spotify Jam</b><small class:live>{live ? 'Playing now' : 'Listen together'}</small></button>
    </div>
  {/if}
  {#if choice === 'youtube'}<MusicPanel {roomId} {owner} />{:else}<JamPanel {roomId} {owner} />{/if}
</div>
<style>
  .choices { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; padding: 12px 12px 0; }
  .choices button { display: grid; gap: 2px; padding: 8px 10px; text-align: left; border-radius: var(--r); border: 1px solid var(--line); color: var(--ink-2); transition: background-color var(--t-fast), border-color var(--t-fast), color var(--t-fast); }
  .choices b { font-size: 13px; color: var(--ink); }
  .choices small { font-size: 11px; }
  .choices small.live { color: var(--lamp); }
  .choices [aria-pressed='true'] { border-color: color-mix(in srgb, var(--lamp) 55%, var(--line)); background: color-mix(in srgb, var(--lamp) 12%, var(--bg-2)); color: var(--ink); }
  @media (hover: hover) { .choices button:hover { background: color-mix(in srgb, var(--ink) 6%, transparent); } }
</style>
