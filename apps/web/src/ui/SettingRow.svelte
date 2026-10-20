<script lang="ts">
  import type { Snippet } from 'svelte'
  /**
   * One setting in a settings list: name and a line of help on the left, the
   * control on the right. A control too wide for the row moves under the name,
   * still on the right. `wide` puts it under the name at full width (text fields).
   * `find` names the row for settings search.
   *
   * On a phone it follows iOS: a switch or a choice ("Label   Value ⌃⌄") stays on the
   * row, and a segmented control gets its own line under the name, full width.
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
  @media (max-width: 650px) {
    /* A switch, a menu or a value edited in place never drops under its name: the name wraps instead, and a long value shortens. */
    .row:has(> .control > :global(:is(input.toggle, select.field, input.value-field))) { flex-wrap: nowrap; }
    .row:has(> .control > :global(:is(input.toggle, select.field, input.value-field))) .text { flex: 1 1 auto; }
    .control:has(> :global(select.field)) { max-width: 60%; }
    .control > :global(select.field) { max-width: 100%; overflow: hidden; text-overflow: ellipsis; }
    /* A slider takes its own line under its name, full width. */
    .row:has(> .control > :global(input[type=range])) { flex-direction: column; align-items: stretch; gap: 4px; }
    .row:has(> .control > :global(input[type=range])) .text { flex: none; }
    .control:has(> :global(input[type=range])) { margin: 0; flex-wrap: nowrap; }
    .control > :global(input[type=range]) { flex: 1; width: auto; }
    /* A segmented control takes its own line, full width, with equal segments. */
    .row:has(> .control > :global(.segments)) { flex-direction: column; align-items: stretch; gap: 10px; }
    .row:has(> .control > :global(.segments)) .text { flex: none; }
    .control:has(> :global(.segments)) { margin: 0; }
    .control > :global(.segments) { display: flex; flex: 1; }
    .control > :global(.segments) > :global(*) { flex: 1 1 0; justify-content: center; min-width: 0; padding-inline: 6px; text-align: center; }
  }
</style>
