<script lang="ts">
  import { call } from '../lib/call.svelte'
  import { native } from '../lib/native'
  import CallControls from './CallControls.svelte'
  const people = $derived(new Set(call.participants.map((p) => p.userId)).size)
</script>

{#if call.channel}
  <div class="dock" class:ptt={call.prefs.mode === 'ptt'} data-testid="call-dock" aria-label="Current call">
    <span class="dot" aria-hidden="true"></span>
    <button class="room" title={call.title} onclick={() => (call.expanded = !call.expanded)}>{call.title}{#if native}<small class="mono"> · {call.instanceName}</small>{/if}</button>
    <span class="count" title={`${people} people on ${call.participants.length} devices`} aria-label={`${people} people on ${call.participants.length} devices`}>{people}</span>
    <CallControls />
  </div>
{/if}

<style>
  .dock { height: 44px; flex: none; display: flex; align-items: center; gap: 6px; padding: 0 8px; background: var(--bg-3); border-top: 1px solid var(--line); }
  .dot { width: 7px; height: 7px; flex: none; border-radius: 50%; background: var(--lamp); box-shadow: 0 0 8px var(--lamp); }
  .room { min-width: 0; flex: 1; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; font-weight: 700; }
  .count { font: 11px var(--mono); color: var(--ink-2); }
  .ptt { gap: 3px; padding-inline: 5px; }
</style>
