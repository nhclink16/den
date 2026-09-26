<script lang="ts">
  import { call } from '../lib/call.svelte'
  import { native } from '../lib/native'
  import CallControls from './CallControls.svelte'
  import CallJam from './CallJam.svelte'
  const people = $derived(new Set(call.participants.filter(p => !p.music).map((p) => p.userId)).size)
</script>

{#if call.channel}
  <CallJam />
  <div class="dock" class:ptt={call.prefs.mode === 'ptt'} data-testid="call-dock" aria-label="Current call">
    <span class="dot" aria-hidden="true"></span>
    <button class="room" title={call.title} onclick={() => (call.expanded = !call.expanded)}>{call.title}{#if native}<small class="mono"> · {call.instanceName}</small>{/if}</button>
    <span class="count" title={`${people} people on ${call.participants.filter(p => !p.music).length} devices`} aria-label={`${people} people on ${call.participants.filter(p => !p.music).length} devices`}>{people}</span>
    <CallControls />
  </div>
{/if}

<style>
  .dock { min-height: 44px; height: auto; flex: none; display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 6px 8px; background: linear-gradient(180deg, color-mix(in srgb, var(--lamp) 9%, var(--bg-3)), var(--bg-3)); border-top: 1px solid color-mix(in srgb, var(--lamp) 30%, var(--line)); }
  .dock :global(.controls) { margin-inline-start: auto; max-width: 100%; flex-wrap: wrap; justify-content: flex-end; }
  .dot { width: 8px; height: 8px; margin-inline: 2px; flex: none; border-radius: 50%; background: var(--lamp); box-shadow: 0 0 8px var(--lamp); }
  @media (prefers-reduced-motion: no-preference) { .dot { animation: breathe 2.4s ease-in-out infinite; } }
  @keyframes breathe { 50% { box-shadow: 0 0 2px var(--lamp); } }
  .room { min-width: 5ch; flex: 1; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 700; }
  .count { font: 11px var(--mono); color: var(--ink-2); }
  .ptt { gap: 3px; padding-inline: 5px; }
</style>
