<script lang="ts">
  import type { ThemeColors, ThemeFonts } from '../lib/types'
  import { fontStack } from '../lib/theme-runtime'

  /** A miniature of Den today, painted in one palette: the room list with its round
   *  icons, a conversation in bubbles, and the message box. Fills its container. */
  let { colors, fonts }: { colors: ThemeColors; fonts: ThemeFonts } = $props()
  const vars = $derived(
    `--p-bg:${colors.bg};--p-bg2:${colors.bg2};--p-bg3:${colors.bg3};` +
    `--p-line:${colors.line};--p-ink:${colors.ink};--p-ink2:${colors.ink2};` +
    `--p-ink3:${colors.ink3};--p-accent:${colors.accent}`,
  )
</script>

<div class="miniature" style={vars} aria-hidden="true">
<div class="preview" style={`font-family:${fontStack(fonts.body, 'body')}`}>
  <div class="side">
    <b class="title" style={`font-family:${fontStack(fonts.display, 'display')}`}>Den</b>
    {#each [0, 1, 2] as i (i)}
      <span class="room" class:sel={i === 0}>
        <i class="tile"></i>
        <span class="lines"><i class="name"></i><i class="line" class:short={i === 2}></i></span>
        {#if i === 1}<i class="badge"></i>{/if}
      </span>
    {/each}
  </div>
  <div class="main">
    <span class="bubble them">Evening!</span>
    <i class="bubble them bar"></i>
    <span class="bubble me">on my way</span>
    <span class="box"><i></i><i class="send"></i></span>
  </div>
</div>
</div>

<style>
  .miniature { width: 100%; height: 100%; container-type: inline-size; overflow: hidden; }
  .preview {
    display: flex; width: 100%; height: 100%; overflow: hidden;
    background: var(--p-bg); color: var(--p-ink);
    font-size: 5cqw; line-height: 1.2;
  }
  i { display: block; }
  .side { width: 40%; flex: none; display: flex; flex-direction: column; gap: 2.4cqw; padding: 5cqw 2.5cqw; background: var(--p-bg2); border-inline-end: 1px solid var(--p-line); }
  .title { font-size: 6.5cqw; font-weight: 700; padding: 0 1.5cqw 1cqw; white-space: nowrap; }
  .room { display: flex; align-items: center; gap: 2cqw; padding: 1.4cqw 1.5cqw; border-radius: 2.5cqw; }
  .room.sel { background: color-mix(in srgb, var(--p-accent) 16%, transparent); }
  .tile { width: 7cqw; height: 7cqw; flex: none; border-radius: 50%; background: var(--p-bg3); }
  .sel .tile { background: color-mix(in srgb, var(--p-accent) 30%, var(--p-bg3)); }
  .lines { flex: 1; min-width: 0; display: grid; gap: 1.2cqw; }
  .name { height: 1.8cqw; width: 70%; border-radius: 1cqw; background: var(--p-ink2); }
  .line { height: 1.4cqw; width: 95%; border-radius: 1cqw; background: var(--p-ink3); opacity: .55; }
  .line.short { width: 60%; }
  .badge { width: 3.2cqw; height: 3.2cqw; flex: none; border-radius: 50%; background: var(--p-accent); }
  .main { flex: 1; min-width: 0; display: flex; flex-direction: column; justify-content: flex-end; gap: 2cqw; padding: 5cqw 4cqw 4cqw; }
  .bubble { max-width: 88%; padding: 1.6cqw 3.2cqw; border-radius: 4cqw; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .them { align-self: flex-start; background: var(--p-bg3); border-bottom-left-radius: 1.2cqw; }
  .them.bar { width: 60%; height: 6cqw; padding: 0; border-top-left-radius: 1.2cqw; }
  .me { align-self: flex-end; background: color-mix(in srgb, var(--p-accent) 24%, var(--p-bg)); border-bottom-right-radius: 1.2cqw; }
  .box { display: flex; align-items: center; justify-content: space-between; margin-top: 2cqw; padding: 1.6cqw 1.6cqw 1.6cqw 4cqw; border-radius: 99px; background: var(--p-bg3); }
  .box i:first-child { height: 1.4cqw; width: 40%; border-radius: 1cqw; background: var(--p-ink3); opacity: .6; }
  .send { width: 4.6cqw; height: 4.6cqw; border-radius: 50%; background: var(--p-accent); }
</style>
