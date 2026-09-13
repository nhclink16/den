<script lang="ts">
  import { objectKind } from '../plugins'
  import { objects } from '../lib/objects.svelte'
  import { call } from '../lib/call.svelte'
  import CallTile from './CallTile.svelte'
  import CallControls from './CallControls.svelte'
  import Icon from './Icon.svelte'
</script>

<div class="call-view" class:expanded={call.expanded} class:multiple-shares={call.participants.filter((p) => p.screen).length > 1} data-testid={call.expanded ? 'call-grid' : 'call-strip'}>
  {#if call.otherDevices > 0 && !call.micOn && call.outputMuted}<p class="device-note" role="status">Mic and sound are off on this device. Your other device stays connected.</p>{/if}
  <div class="tiles">
    {#if objects.active}
      {@const Tile = objectKind(objects.active.kind)?.tile}
      {#if Tile}<Tile object={objects.active} />{/if}
    {/if}
    {#each call.participants.filter((p) => p.screen) as participant (participant.id)}<CallTile {participant} screen />{/each}
    {#each call.participants as participant (participant.id)}<CallTile {participant} />{/each}
  </div>
  <button class="expand" aria-label={call.expanded ? 'Collapse call' : 'Expand call'} title={call.expanded ? 'Collapse call' : 'Expand call'} onclick={() => (call.expanded = !call.expanded)}><Icon name={call.expanded ? 'collapse' : 'expand'} /></button>
  {#if call.expanded}<div class="toolbar"><CallControls large /></div>{/if}
</div>

<style>
  .device-note { position: absolute; left: 12px; top: 4px; z-index: 1; margin: 0; padding: 4px 8px; max-width: calc(100% - 56px); border-radius: 6px; background: var(--bg); font-size: 12px; color: var(--ink-2); }
  .call-view { height: 160px; flex: none; position: relative; min-height: 0; background: var(--bg-2); border-bottom: 1px solid var(--line); }
  .tiles { height: 100%; display: flex; align-items: center; gap: 10px; padding: 12px; overflow-x: auto; scrollbar-width: none; }
  .call-view:not(.expanded) .tiles :global(.tile.screen) { height: 100%; aspect-ratio: auto; }
  .tiles::-webkit-scrollbar { display: none; }
  .expand { z-index: 3; position: absolute; top: 6px; right: 6px; width: 28px; height: 28px; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 6px; color: var(--ink-2); background: var(--bg-2); }
  .expand:hover { color: var(--ink); }
  .expanded { flex: 1; height: auto; }
  .expanded .tiles { display: grid; height: auto; max-height: 100%; grid-auto-rows: max-content; grid-template-columns: repeat(auto-fit, minmax(280px, 1fr)); align-content: start; align-items: start; gap: 12px; padding: 16px 16px 88px; overflow: auto; }
  .expanded :global(.tile) { width: 100%; }
  .expanded :global(.tile.screen) { grid-column: 1 / -1; }
  .expanded.multiple-shares :global(.tile.screen) { grid-column: auto; }
  .toolbar { z-index: 3; position: absolute; bottom: 12px; left: 50%; transform: translateX(-50%); padding: 8px; border-radius: 999px; border: 1px solid var(--line); background: var(--bg-2); box-shadow: 0 8px 24px rgba(0,0,0,.3); max-width: calc(100% - 16px); }
  @media (max-width: 340px) { .expanded .tiles { grid-template-columns: minmax(0,1fr); } }
</style>
