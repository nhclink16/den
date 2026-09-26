<script lang="ts" module>
  import { SvelteSet } from 'svelte/reactivity'
  // Per Jam, for this session: a dismissed prompt stays dismissed across
  // leaving and rejoining the call, but a new Jam asks again.
  const dismissed = new SvelteSet<string>()
</script>
<script lang="ts">
  // The Jam in the call bar. Someone in the call who has not opened the Jam gets
  // one prompt; after Join or ✕ it settles into the now-playing strip.
  import { call } from '../lib/call.svelte'
  import { joinJam } from '../lib/jam'
  import Icon from './Icon.svelte'
  import JamCard from './JamCard.svelte'
  const owner = $derived(call.channel ? call.owner : null)
  const room = $derived(call.channel?.id ?? '')
  const jam = $derived(owner?.jams.get(room))
  const ask = $derived(!!jam && !!owner?.me && !jam.joined_user_ids.includes(owner.me.id) && !dismissed.has(jam.id))
  function join() {
    if (!jam || !owner) return
    dismissed.add(jam.id)
    void joinJam(owner, room).catch(() => {})
  }
</script>
{#if jam && owner}
  {#if ask}
    <div class="prompt" role="status" data-testid="jam-prompt">
      <span class="copy"><b>{owner.name(jam.host_id)}'s Jam is playing</b><small><Icon name="headset" size={12} /> Headphones recommended</small></span>
      <a class="btn lit" href={jam.url} target="_blank" rel="noopener noreferrer" onclick={join}>Join in Spotify<span class="sr-only"> (opens in a new window)</span></a>
      <button class="x" aria-label="Not now" title="Not now" onclick={() => dismissed.add(jam.id)}><Icon name="x" size={14} /></button>
    </div>
  {:else}
    <div class="strip" data-testid="call-jam"><JamCard channelId={room} {owner} compact /></div>
  {/if}
{/if}
<style>
  .prompt, .strip { border-top: 1px solid color-mix(in srgb, var(--lamp) 30%, var(--line)); background: var(--bg-2); }
  .prompt { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; padding: 10px 10px 10px 12px; background: color-mix(in srgb, var(--lamp) 10%, var(--bg-2)); }
  .copy { flex: 1; min-width: 12ch; display: grid; gap: 2px; }
  b { font-size: 13px; overflow-wrap: anywhere; }
  small { display: flex; align-items: center; gap: 4px; font-size: 11px; color: var(--ink-2); }
  .btn { min-height: 32px; padding: 4px 12px; font-size: 12.5px; text-decoration: none; }
  .x { width: 28px; height: 28px; display: grid; place-items: center; border-radius: var(--r); color: var(--ink-2); }
  @media (hover: hover) { .x:hover { background: color-mix(in srgb, var(--ink) 8%, transparent); color: var(--ink); } }
</style>
