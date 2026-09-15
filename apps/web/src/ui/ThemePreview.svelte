<script lang="ts">
  import type { ThemeColors } from '../lib/types'
  import Mark from './Mark.svelte'

  /** A miniature of the Den interface painted in one palette. Fills its container. */
  let { colors, display = 'system-ui' }: { colors: ThemeColors; display?: string } = $props()
  const vars = $derived(
    `--p-bg:${colors.bg};--p-bg2:${colors.bg2};--p-bg3:${colors.bg3};` +
    `--p-line:${colors.line};--p-ink:${colors.ink};--p-ink2:${colors.ink2};` +
    `--p-ink3:${colors.ink3};--p-accent:${colors.accent}`,
  )
</script>

<div class="preview" style={vars} aria-hidden="true">
  <div class="side">
    <div class="brand" style={`font-family:${display},serif`}><Mark size={9} accent={colors.accent} /><span>den</span></div>
    <i class="row"></i>
    <i class="row lit"></i>
    <i class="row"></i>
    <i class="row short"></i>
  </div>
  <div class="main">
    <div class="msg">
      <span class="avatar"></span>
      <div class="lines">
        <b>Evening, everyone</b>
        <span class="body">The room is ready. <em>@you</em></span>
      </div>
    </div>
    <div class="composer"><span>Message…</span><span class="send"></span></div>
  </div>
</div>

<style>
  .preview {
    display: flex; width: 100%; height: 100%; overflow: hidden;
    background: var(--p-bg); color: var(--p-ink);
    font-family: system-ui, sans-serif; font-size: 5.2px; line-height: 1.3;
    container-type: inline-size;
  }
  .side {
    width: 27%; flex: none; padding: 7% 6%;
    background: var(--p-bg2); border-inline-end: 1px solid var(--p-line);
  }
  .brand { display: flex; align-items: center; gap: 2.5px; font-size: 7px; font-weight: 700; margin-bottom: 9%; }
  .brand :global(svg) { flex: none; }
  .row { display: block; height: 2.6px; border-radius: 2px; background: var(--p-line); margin-bottom: 14%; }
  .row.short { width: 62%; }
  .row.lit { background: var(--p-ink3); }
  .main { flex: 1; min-width: 0; display: flex; flex-direction: column; justify-content: space-between; padding: 9% 7% 7%; }
  .msg { display: flex; gap: 4px; align-items: flex-start; }
  .avatar { width: 11px; height: 11px; border-radius: 34%; background: var(--p-bg3); flex: none; }
  .lines { min-width: 0; }
  .lines b { display: block; font-size: 5.6px; margin-bottom: 2.5px; }
  .body { color: var(--p-ink2); }
  em { font-style: normal; color: var(--p-accent); background: color-mix(in srgb, var(--p-accent) 20%, transparent); padding: 0 1.5px; border-radius: 1.5px; }
  .composer {
    display: flex; align-items: center; justify-content: space-between;
    background: var(--p-bg3); border: 1px solid var(--p-line); color: var(--p-ink3);
    padding: 3.5px 4px; border-radius: 3px;
  }
  .send { width: 5px; height: 5px; border-radius: 50%; background: var(--p-accent); flex: none; }
</style>
