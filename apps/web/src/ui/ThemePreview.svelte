<script lang="ts">
  import type { ThemeColors, ThemeFonts } from '../lib/types'
  import Mark from './Mark.svelte'

  /** A miniature of the Den interface painted in one palette. Fills its container. */
  let { colors, fonts }: { colors: ThemeColors; fonts: ThemeFonts } = $props()
  const vars = $derived(
    `--p-bg:${colors.bg};--p-bg2:${colors.bg2};--p-bg3:${colors.bg3};` +
    `--p-line:${colors.line};--p-ink:${colors.ink};--p-ink2:${colors.ink2};` +
    `--p-ink3:${colors.ink3};--p-accent:${colors.accent}`,
  )
</script>

<div class="miniature" style={vars} aria-hidden="true">
<div class="preview" style={`font-family:"${fonts.body}",sans-serif`}>
  <div class="side">
    <div class="brand" style={`font-family:"${fonts.display}",serif`}><Mark size={9} accent={colors.accent} /><span>den</span></div>
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
    <div class="composer" style={`font-family:"${fonts.mono}",monospace`}><span>Message…</span><span class="send"></span></div>
  </div>
</div>
</div>

<style>
  .miniature { width: 100%; height: 100%; container-type: inline-size; overflow: hidden; }
  .preview {
    display: flex; width: 100%; height: 100%; overflow: hidden;
    background: var(--p-bg); color: var(--p-ink);
    font-size: 4.2cqw; line-height: 1.3;
  }
  .side {
    width: 27%; flex: none; padding: 7cqw 3cqw;
    background: var(--p-bg2); border-inline-end: 1px solid var(--p-line);
  }
  .brand { display: flex; align-items: center; gap: 2cqw; font-size: 5.6cqw; font-weight: 700; margin-bottom: 7cqw; }
  .brand :global(svg) { flex: none; width: 7cqw; height: 7cqw; }
  .row { display: block; height: 2cqw; border-radius: 1cqw; background: var(--p-line); margin-bottom: 3cqw; }
  .row.short { width: 62%; }
  .row.lit { background: var(--p-ink3); }
  .main { flex: 1; min-width: 0; display: flex; flex-direction: column; justify-content: space-between; padding: 7cqw 4cqw 5cqw; }
  .msg { display: flex; gap: 3cqw; align-items: flex-start; }
  .avatar { width: 9cqw; height: 9cqw; border-radius: 34%; background: var(--p-bg3); flex: none; }
  .lines { min-width: 0; }
  .lines b { display: block; font-size: 4.5cqw; margin-bottom: 2cqw; }
  .body { color: var(--p-ink2); }
  em { font-style: normal; color: var(--p-accent); background: color-mix(in srgb, var(--p-accent) 20%, transparent); padding: 0 1cqw; border-radius: 1cqw; }
  .composer {
    display: flex; align-items: center; justify-content: space-between;
    background: var(--p-bg3); border: 1px solid var(--p-line); color: var(--p-ink3);
    padding: 2.5cqw 3cqw; border-radius: 2cqw;
  }
  .send { width: 4cqw; height: 4cqw; border-radius: 50%; background: var(--p-accent); flex: none; }
</style>
