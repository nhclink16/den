<script lang="ts">
  import type { Snippet } from 'svelte'
  /**
   * One setting in a settings list: name and a line of help on the left, the
   * control on the right. A control too wide for the row moves under the name,
   * still on the right. `wide` puts it under the name at full width (text fields).
   * `find` names the row for settings search.
   */
  let { label, hint, control, wide = false, find }: { label: string; hint?: string; control?: Snippet; wide?: boolean; find?: string } = $props()
</script>

<div class="row" class:wide data-find={find}>
  <div class="text">
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </div>
  {#if control}<div class="control">{@render control()}</div>{/if}
</div>

<style>
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 16px; min-height: 48px; padding: 10px 16px; }
  .text { flex: 1 1 14em; min-width: 0; display: grid; gap: 2px; }
  .label { font-size: var(--fs-body); line-height: 1.3; overflow-wrap: anywhere; }
  .hint { font-size: var(--fs-xs); line-height: 1.4; color: var(--ink-2); text-wrap: pretty; }
  .control { flex: 0 1 auto; min-width: 0; margin-inline-start: auto; display: flex; flex-wrap: wrap; align-items: center; justify-content: flex-end; gap: 8px; }
  .row.wide { flex-direction: column; align-items: stretch; gap: 10px; }
  .row.wide .text { flex: none; }
  .row.wide .control { margin: 0; justify-content: flex-start; }
  @media (pointer: coarse) { .row { min-height: 52px; } }
</style>
