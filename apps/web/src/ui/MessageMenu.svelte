<script module lang="ts">
  export type Action = { label: string; icon: 'reply' | 'thread' | 'copy' | 'edit' | 'trash'; run: () => void; danger?: boolean }
</script>

<script lang="ts">
  import { onMount, tick, type Snippet } from 'svelte'
  import { modal } from '../lib/modal'
  import Icon from './Icon.svelte'
  import EmojiPicker from './EmojiPicker.svelte'

  // A long press on a message, the iPhone way: the chat blurs and dims, the
  // message lifts out of it, quick reactions sit above and its actions below,
  // with the time as the menu's header. A modal dialog, so VoiceOver and a
  // keyboard stay inside it and Escape closes it. Desktop keeps its hover toolbar.
  let { from, right, when, label, quick, picked, actions, preview, onreact, onclose }: {
    /** The message's content box: where the lifted copy starts, and how wide it is. */
    from: HTMLElement
    right: boolean
    when: string
    label: string
    quick: string[]
    /** The reactions you've already added, shown as chosen. */
    picked: string[]
    actions: Action[]
    preview: Snippet
    onreact: (emoji: string) => void
    onclose: () => void
  } = $props()

  let strip = $state<HTMLElement>(), lifted = $state<HTMLElement>(), menu = $state<HTMLElement>()
  let place = $state<{ strip: string; lifted: string; menu: string; cap?: number; drop: number }>()

  // Everything stays where the message was if it fits; otherwise it slides just
  // enough to fit between the notch and the home indicator, and a very long
  // message is cut short with a fade.
  function layout() {
    if (!strip || !lifted || !menu || !from.isConnected) return
    const box = from.getBoundingClientRect()
    const said = from.querySelector<HTMLElement>(':scope > :is(.bubble, .gif, .files)')?.getBoundingClientRect() ?? box
    const frame = getComputedStyle(strip.parentElement!)
    const top = parseFloat(frame.paddingTop), bottom = innerHeight - parseFloat(frame.paddingBottom)
    // At the message's own width first: narrower, its text wraps onto more lines.
    lifted.style.width = `${box.width}px`
    const gap = 8, s = strip.offsetHeight, m = menu.offsetHeight
    const room = bottom - top - s - m - gap * 2
    // Its own box: scrollHeight would count the reaction pills' wider touch areas.
    const full = (lifted.firstElementChild as HTMLElement).offsetHeight, h = Math.min(full, room)
    const y = Math.min(Math.max(box.top, top + s + gap), bottom - m - gap - h)
    // The strip and the menu line up with the bubble's own edge, inside the screen.
    const edge = (w: number) => {
      const x = right ? said.right - w : said.left
      return Math.min(Math.max(x, 10), innerWidth - w - 10)
    }
    place = {
      strip: `top:${y - gap - s}px;left:${edge(strip.offsetWidth)}px`,
      lifted: `top:${y}px;left:${box.left}px;width:${box.width}px`,
      menu: `top:${y + h + gap}px;left:${edge(menu.offsetWidth)}px`,
      cap: h < full ? h : undefined,
      drop: box.top - y,
    }
  }
  onMount(() => {
    void tick().then(layout)
    // The keyboard going down (focus left the reply box) moves the message.
    addEventListener('resize', layout)
    return () => removeEventListener('resize', layout)
  })

  function act(a: Action) { onclose(); a.run() }
  // Arrow keys walk the menu, as in any menu.
  function walk(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
    e.preventDefault()
    const items = [...menu!.querySelectorAll<HTMLElement>('[role=menuitem]')], n = items.length
    const at = items.indexOf(document.activeElement as HTMLElement), down = e.key === 'ArrowDown'
    items[at < 0 ? (down ? 0 : n - 1) : (at + (down ? 1 : n - 1)) % n]?.focus()
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (focus starts on the dialog, so VoiceOver reads its name; Escape closes it) -->
<dialog class="layer" use:modal tabindex="-1" aria-label={label} class:placed={!!place} style:--drop="{place?.drop ?? 0}px"
  oncancel={(e) => { e.preventDefault(); onclose() }} onkeydown={(e) => { if (e.key === 'Escape' && !e.defaultPrevented) { e.preventDefault(); onclose() } }}>
  <div class="frame">
    <div class="strip" class:right bind:this={strip} style={place?.strip} role="group" aria-label="React">
      {#each quick as e (e)}
        <button type="button" class:on={picked.includes(e)} aria-pressed={picked.includes(e)} onclick={() => { onreact(e); onclose() }}>{e}</button>
      {/each}
      <EmojiPicker label="More reactions" align={right ? 'end' : 'start'} onpick={(e) => { onreact(e); onclose() }}><span class="more" aria-hidden="true"><Icon name="plus" size={20} /></span></EmojiPicker>
    </div>
    <div class="lifted" class:capped={!!place?.cap} bind:this={lifted} style={place?.lifted} style:max-height={place?.cap ? `${place.cap}px` : undefined} inert aria-hidden="true">
      {@render preview()}
    </div>
    <!-- svelte-ignore a11y_interactive_supports_focus (the items take focus, not the menu) -->
    <div class="menu" class:right bind:this={menu} style={place?.menu} role="menu" aria-label={when} tabindex="-1" onkeydown={walk}>
      <div class="when" aria-hidden="true">{when}</div>
      {#each actions as a (a.label)}
        <button type="button" role="menuitem" class:danger={a.danger} onclick={() => act(a)}>{a.label}<Icon name={a.icon} size={19} /></button>
      {/each}
    </div>
  </div>
  <!-- Last, so VoiceOver reads the reactions and actions first; it covers the chat behind them. -->
  <button type="button" class="scrim" tabindex="-1" aria-label="Close" onclick={onclose}></button>
</dialog>

<style>
  .layer {
    position: fixed; inset: 0; width: 100%; height: 100%; max-width: none; max-height: none; margin: 0; padding: 0; border: 0;
    background: none; color: var(--ink); overflow: hidden; touch-action: none; -webkit-touch-callout: none;
  }
  .layer::backdrop { background: transparent; }
  .layer:focus { outline: none; }
  .scrim {
    position: fixed; inset: 0; z-index: 0; width: 100%; cursor: default;
    /* Frosted: a wash of the page colour over a light dim, so the lifted message stands
       out even where the blur isn't drawn. */
    background: color-mix(in srgb, var(--bg) 45%, rgb(0 0 0 / .22));
    -webkit-backdrop-filter: blur(18px) saturate(1.3); backdrop-filter: blur(18px) saturate(1.3);
  }
  .frame {
    position: fixed; inset: 0; z-index: 1; pointer-events: none;
    padding: max(12px, env(safe-area-inset-top)) 0 max(12px, env(safe-area-inset-bottom));
  }
  .strip, .lifted, .menu { position: fixed; pointer-events: auto; }
  /* Measured where they'd sit, then shown: no jump on the first frame. */
  .layer:not(.placed) :is(.strip, .lifted, .menu) { visibility: hidden; }

  .strip {
    display: flex; align-items: center; gap: 2px; padding: 4px 6px;
    border-radius: var(--r-pill, 999px); background: var(--bg-2); box-shadow: 0 8px 30px var(--shadow);
  }
  .strip button { width: 42px; height: 42px; display: grid; place-items: center; font-size: 26px; line-height: 1; border-radius: 50%; transition: transform var(--t) var(--ease-out), background-color var(--t-fast); }
  .strip button.on { background: var(--sel); }
  .strip button:active { transform: scale(.88); }
  .strip :global(summary) { list-style: none; }
  .more { width: 36px; height: 36px; margin: 3px; display: grid; place-items: center; border-radius: 50%; background: var(--bg-3); color: var(--ink-2); }

  .lifted { pointer-events: none; overflow: hidden; filter: drop-shadow(0 10px 24px var(--shadow)); }
  .lifted.capped { mask: linear-gradient(#000 calc(100% - 48px), transparent); }

  .menu {
    min-width: 220px; max-width: calc(100vw - 20px); padding: 0; overflow: hidden;
    border-radius: max(12px, var(--r-lg)); background: var(--bg-2); box-shadow: 0 12px 40px var(--shadow); outline: none;
  }
  .when { padding: 9px 16px 7px; font-size: var(--fs-xs); color: var(--ink-3); text-align: center; border-bottom: 1px solid var(--hair); }
  .menu button {
    display: flex; align-items: center; justify-content: space-between; gap: 24px; width: 100%; min-height: 46px; padding: 0 16px;
    font-size: var(--fs-body); color: var(--ink); text-align: start; border-bottom: 1px solid var(--hair);
  }
  .menu button:last-child { border-bottom: 0; }
  .menu button :global(svg) { color: var(--ink-2); flex: none; }
  .menu button.danger, .menu button.danger :global(svg) { color: var(--ember); }
  .menu button:active { background: var(--press); }
  @media (hover: hover) { .menu button:hover { background: var(--hover); } .strip button:hover { background: var(--bg-3); } }

  @media (prefers-reduced-motion: no-preference) {
    .placed .scrim { animation: fade .2s ease-out; }
    .placed .lifted { animation: lift .28s var(--ease-out); }
    .placed .strip, .placed .menu { animation: pop .26s var(--ease-out); transform-origin: left center; }
    .placed .right:is(.strip, .menu) { transform-origin: right center; }
  }
  @keyframes fade { from { opacity: 0; } }
  @keyframes lift { from { transform: translateY(var(--drop)); filter: none; } }
  @keyframes pop { from { opacity: 0; transform: scale(.85); } }
</style>
