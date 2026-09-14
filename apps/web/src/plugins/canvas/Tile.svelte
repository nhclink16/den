<script lang="ts">
  import type { ObjectProps } from '..'
  import { objects } from '../../lib/objects.svelte'
  import { call } from '../../lib/call.svelte'
  import View from './View.svelte'
  let { object }: ObjectProps = $props()
</script>
<div class="tile screen canvas-tile" data-testid="canvas-call-tile">
  <View {object} readonly />
  <button class="focus" aria-label={`Focus canvas ${object.name}`} onclick={() => { call.expanded = false; objects.expanded = true }}></button>
  <span class="label">{object.name}</span>
</div>
<style>
  .canvas-tile { isolation: isolate; position: relative; flex: none; width: 260px; aspect-ratio: 16/9; border: 1px solid var(--line); border-radius: var(--r-lg); overflow: hidden; }
  .focus { position: absolute; inset: 0 0 34px; z-index: 2; }
  .label { position: absolute; bottom: 6px; left: 8px; max-width: 65%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; padding: 2px 6px; background: var(--overlay); border-radius: 4px; pointer-events: none; }
</style>
