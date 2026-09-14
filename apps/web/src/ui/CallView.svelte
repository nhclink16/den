<script lang="ts">
  import { objectKind } from '../plugins'
  import { objects } from '../lib/objects.svelte'
  import { call } from '../lib/call.svelte'
  import CallGrid from './CallGrid.svelte'
  import CallTile from './CallTile.svelte'
  import CallControls from './CallControls.svelte'
  import Icon from './Icon.svelte'
</script>

<div class="call-view" class:expanded={call.expanded} data-testid={call.expanded ? 'call-grid' : 'call-strip'}>
  {#if call.otherDevices > 0 && !call.micOn && call.outputMuted}<p class="device-note" role="status">Mic and sound are off on this device. Your other device stays connected.</p>{/if}
  {#if call.expanded}<CallGrid />{:else}
  <div class="tiles">
    {#if objects.active}
      {@const Tile = objectKind(objects.active.kind)?.tile}
      {#if Tile}<Tile object={objects.active} />{/if}
    {/if}
    {#each call.participants as participant (participant.id)}{#each participant.screens as share (share.name)}<CallTile {participant} {share} screen />{/each}{/each}
    {#each call.participants as participant (participant.id)}<CallTile {participant} />{/each}
  </div>
  {/if}
  <button class="expand" aria-label={call.expanded ? 'Collapse call' : 'Expand call'} title={call.expanded ? 'Collapse call' : 'Expand call'} onclick={() => (call.expanded = !call.expanded)}><Icon name={call.expanded ? 'collapse' : 'expand'} /></button>
  {#if call.expanded}<div class="toolbar"><CallControls large /></div>{/if}
</div>

<style>
  .expanded .device-note { top: 40px; }
  .device-note { position: absolute; left: 12px; top: 4px; z-index: 1; margin: 0; padding: 4px 8px; max-width: calc(100% - 56px); border-radius: 6px; background: var(--bg); font-size: 12px; color: var(--ink-2); }
  .call-view { height: 160px; flex: none; position: relative; min-height: 0; background: var(--bg-2); border-bottom: 1px solid var(--line); }
  .tiles { height: 100%; display: flex; align-items: center; gap: 10px; padding: 12px; overflow-x: auto; scrollbar-width: none; }
  .call-view:not(.expanded) .tiles :global(.tile.screen) { height: 100%; aspect-ratio: auto; }
  .tiles::-webkit-scrollbar { display: none; }
  .expand { z-index: 3; position: absolute; top: 6px; right: 6px; width: 28px; height: 28px; display: grid; place-items: center; border: 1px solid var(--line); border-radius: 6px; color: var(--ink-2); background: var(--bg-2); }
  .expand:hover { color: var(--ink); }
  .expanded { flex: 1; height: auto; }
  .toolbar { z-index: 3; position: absolute; bottom: 12px; left: 50%; transform: translateX(-50%); padding: 8px; border-radius: 999px; border: 1px solid var(--line); background: var(--bg-2); box-shadow: 0 8px 24px var(--shadow); max-width: calc(100% - 16px); }
</style>
