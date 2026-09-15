<script lang="ts">
  import type { Snippet } from 'svelte'
  /** One labelled setting: name and explanation on the left, control on the right. */
  let { label, hint, control, wide = false }: { label: string; hint?: string; control: Snippet; wide?: boolean } = $props()
</script>

<div class="row" class:wide>
  <div class="text">
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </div>
  <div class="control">{@render control()}</div>
</div>

<style>
  .row {
    display: flex; align-items: center; justify-content: space-between; gap: 24px;
    padding: 14px 16px; border-bottom: 1px solid var(--line);
  }
  .row:last-child { border-bottom: 0; }
  .text { min-width: 0; display: grid; gap: 3px; }
  .label { font-size: 14px; font-weight: 600; }
  .hint { font-size: 12.5px; color: var(--ink2); }
  .control { flex: none; display: flex; align-items: center; gap: 8px; }
  .row.wide { flex-direction: column; align-items: stretch; gap: 12px; }
  .row.wide .control { flex: 1; }
  @media (max-width: 720px) {
    .row { flex-direction: column; align-items: stretch; gap: 10px; }
    .control { justify-content: flex-start; }
  }
</style>
