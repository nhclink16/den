<script lang="ts">
  // Two people talking, drawn with the shape and colours chosen above, so the
  // choice can be judged before going back to a room.
  import { store } from '../lib/store.svelte'
  import { themes } from '../lib/theme.svelte'
  import { resolveAccent } from '../lib/theme-color'
  import { accentBubble, bubbleFill, nameInk, personColor, withContrast } from '../lib/bubbles'

  /** `bare`: no frame of its own, for a preview that draws one. */
  let { bare = false }: { bare?: boolean } = $props()

  const other = '#d17465'
  const look = $derived.by(() => {
    const half = themes.half, a = themes.appearance, c = withContrast(themes.active[half], a.contrast)
    const accent = resolveAccent(c, a.accent, half)
    const me = personColor(store.me?.id ?? '', store.me?.accent)
    return {
      feed: a.bubble_shape === 'none',
      them: bubbleFill(a.bubble_colors, false, false, other, accent, c),
      mine: bubbleFill(a.bubble_colors, true, false, me, accent, c),
      lit: accentBubble(a.bubble_colors),
      themName: nameInk(a.bubble_colors, false, other, c),
      myName: nameInk(a.bubble_colors, true, me, c),
    }
  })
</script>

<div class="preview" class:bare class:feed={look.feed} aria-hidden="true">
  <div class="line them">
    <span class="who" style:color={look.themName}>Maya</span>
    <span class="b first" style:--fill={look.them}>are we still on for 9?</span>
    <span class="b" style:--fill={look.them}>I can bring the speaker</span>
  </div>
  <div class="line mine">
    {#if look.feed}<span class="who" style:color={look.myName}>{store.me?.display_name || 'You'}</span>{/if}
    <span class="b first" class:lit={look.lit && !look.feed} style:--fill={look.mine}>yes! leaving at 10</span>
  </div>
</div>

<style>
  .preview { display: grid; gap: 10px; margin-top: 12px; padding: 14px; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg); font-size: var(--fs-sm); }
  .preview.bare { margin: 0; padding: 0; border: 0; background: none; }
  .line { display: flex; flex-direction: column; align-items: flex-start; gap: 2px; }
  .line.mine { align-items: flex-end; }
  .who { margin: 0 0 1px 10px; font-size: var(--fs-xs); color: var(--ink-2); font-weight: 600; }
  .b { max-width: 80%; padding: 6px 11px; border-radius: var(--r-bubble); background: var(--fill, var(--bg-3)); color: var(--ink); }
  .them .b { border-bottom-left-radius: var(--r-tight); }
  .them .b:not(.first) { border-top-left-radius: var(--r-tight); }
  .mine .b { border-bottom-right-radius: var(--r-tight); }
  .b.lit { background: var(--lamp); color: var(--on-lamp); }
  .feed .line, .feed .line.mine { align-items: flex-start; }
  .feed .who { margin-left: 0; font-size: var(--fs-sm); font-weight: 700; color: var(--ink); }
  .feed .b { padding: 0; background: none; border-radius: 0; max-width: none; }
</style>
