<script lang="ts">
  // The Spotify half of the call's Music menu: how to start a Jam, or the live one.
  import type { Store } from '../lib/store.svelte'
  import JamCard from './JamCard.svelte'
  import JamComposer from './JamComposer.svelte'
  import InlineConfirm from './InlineConfirm.svelte'
  import { endJam } from '../lib/jam'
  let { roomId, owner }: { roomId: string; owner: Store } = $props()
  const jam = $derived(owner.jams.get(roomId))
  const voice = $derived(owner.channel(roomId)?.kind === 'voice')
  const canEnd = $derived(!!jam && (jam.host_id === owner.me?.id || owner.me?.role === 'admin'))
  const joined = $derived(jam?.joined_user_ids ?? [])
  let error = $state('')
  async function end() {
    error = ''
    try { await endJam(owner, roomId) } catch (e) { error = e instanceof Error ? e.message : 'Could not end the Jam.' }
  }
</script>
<section class="jam-panel" aria-label="Spotify Jam" data-testid="jam-panel">
  {#if jam}
    <div class="card"><JamCard channelId={roomId} {owner} compact /></div>
    <div class="meta">
      <p title="Spotify does not say who is listening; these are the people who pressed Join here.">Started by {owner.name(jam.host_id)} · {joined.length} joined from Den</p>
      {#if canEnd}<InlineConfirm action="End" sentence="End this Jam for everyone?" confirm={end} />{/if}
    </div>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <p class="note">Headphones recommended, so your mic doesn't send the music back into the call.</p>
    <details class="replace"><summary>Start a different Jam</summary><JamComposer {roomId} {owner} label="Replace the Jam" /></details>
  {:else}
    <h2>Listen together on Spotify</h2>
    <p class="lede">Everyone hears the same songs in their own Spotify app. Den shows what's playing and who joined.</p>
    <ol class="steps">
      <li><span><b>Start a Jam in Spotify.</b> On what's playing, tap the devices icon, then Start a Jam.</span></li>
      <li><span><b>Copy the invite link.</b> Tap Invite, then Copy link.</span></li>
      <li><span><b>Paste it here.</b> Everyone in the call gets a Join button.</span></li>
    </ol>
    <JamComposer {roomId} {owner} />
    <p class="note">{#if voice}Starting a Jam pauses the {owner.settings.instance_name} DJ queue until the Jam ends.{' '}{/if}Headphones recommended.</p>
  {/if}
</section>
<style>
  .jam-panel { width: min(380px, calc(100vw - 24px)); max-height: min(720px, calc(100dvh - 32px)); overflow: auto; overscroll-behavior: contain; padding: 18px; color: var(--ink); display: grid; gap: 12px; }
  h2 { font-size: 18px; margin: 0; }
  .lede, .note { margin: 0; font-size: 12px; color: var(--ink-2); }
  .steps { margin: 0; padding: 0; list-style: none; counter-reset: step; display: grid; gap: 10px; }
  .steps li { counter-increment: step; display: grid; grid-template-columns: 22px 1fr; gap: 10px; font-size: 13px; color: var(--ink-2); }
  .steps li::before { content: counter(step); width: 22px; height: 22px; display: grid; place-items: center; border-radius: 50%; background: var(--bg-3); color: var(--lamp); font: 11px var(--mono); }
  .steps b { color: var(--ink); font-weight: 700; }
  .card { margin: -18px -18px 0; border-radius: var(--r-lg) var(--r-lg) 0 0; overflow: hidden; }
  .card { border-bottom: 1px solid var(--line); }
  .meta { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 32px; }
  .meta p { margin: 0; font-size: 12px; color: var(--ink-2); }
  .error { margin: 0; font-size: 12px; color: var(--danger); }
  .replace summary { cursor: pointer; min-height: 32px; display: flex; align-items: center; font-size: 12px; color: var(--ink-2); }
  .replace[open] summary { margin-bottom: 8px; }
</style>
