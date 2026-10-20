<script module lang="ts">
  // Whether the device itself is offline, so a screen can tell "no network" from "Den isn't answering".
  const net = $state({ online: typeof navigator === 'undefined' || navigator.onLine })
  if (typeof window !== 'undefined') {
    addEventListener('online', () => (net.online = true))
    addEventListener('offline', () => (net.online = false))
  }
  /** What the bar says while Den's connection is down, as iPhone apps do: nothing while healthy. */
  export const trouble = (connected: boolean) => (connected ? '' : net.online ? 'Connecting…' : 'Waiting for network…')
</script>

<script lang="ts">
  import type { Snippet } from 'svelte'

  // The phone's top bar, the iPhone way: 44pt, back on the left, a centred title with an optional grey line
  // under it, plain icons on the right. It turns translucent once content scrolls under it. `large` adds a
  // large title below the bar that scrolls away, and the small title fades in as it goes.
  //
  // Put it first inside the screen's scroller (it sticks to the top, so content scrolls under it), or above a
  // screen whose scroller is further down (it watches scrolls below it). A screen that starts at the top of a
  // phone gets `--nav-inset` from Shell and the bar reaches up under the status bar.
  let {
    title, subtitle = '', status = '', large = false, leading, trailing, heading, subline,
  }: {
    title: string
    subtitle?: string
    /** Trouble, like "Connecting…": shown with a spinner in place of the subtitle (or the small title, with `large`). */
    status?: string
    large?: boolean
    leading?: Snippet
    trailing?: Snippet
    /** Replaces the small title, for one that does something (a DM's person). Bring your own h1. */
    heading?: Snippet
    /** Replaces the subtitle, for one that does something (a room's threads). */
    subline?: Snippet
  } = $props()

  let bar = $state<HTMLElement>()
  let big = $state<HTMLElement>()
  let lead = $state(0), trail = $state(0)
  // How far the content under the bar has scrolled.
  let top = $state(0)
  const scrolled = $derived(large ? top > (big?.offsetHeight ?? 40) - 4 : top > 0)

  $effect(() => {
    const host = bar?.parentElement
    if (!host) return
    top = host.scrollTop
    // Scroll events don't bubble, but they can be caught on the way down: the screen's own scroller, or the
    // one under the bar (a room's messages). Not a sheet's list, a text box, anything in the message box (a draft,
    // the @mention list), or a row that only scrolls sideways.
    const fn = (e: Event) => {
      const t = e.target as Element | Document
      if (!(t instanceof Element) || t.closest('[popover], textarea, .composer') || (t !== host && large)) return
      if (t.scrollHeight <= t.clientHeight) return
      top = t.scrollTop
    }
    host.addEventListener('scroll', fn, { capture: true, passive: true })
    return () => host.removeEventListener('scroll', fn, { capture: true })
  })
  // The title stays centred on the screen: both sides take the width of the wider one.
  const side = $derived(Math.max(lead, trail))
</script>

<header class="navbar" class:scrolled class:large bind:this={bar} style:--side="{side}px">
  <div class="row">
    <div class="side lead"><div class="fit" bind:clientWidth={lead}>{@render leading?.()}</div></div>
    <div class="mid" class:shown={!large || scrolled || !!status}>
      {#if large}<p class="title" aria-hidden="true">{status || title}</p>
      {:else if heading}{@render heading()}
      {:else}<h1 class="title" tabindex="-1">{title}</h1>{/if}
      {#if large}<!-- The large title below says it. -->
      {:else if status}<p class="sub busy" role="status"><span class="spin" aria-hidden="true"></span>{status}</p>
      {:else if subline}{@render subline()}
      {:else if subtitle}<p class="sub">{subtitle}</p>{/if}
      {#if large && status}<span class="spin top" aria-hidden="true"></span><span class="sr-only" role="status">{status}</span>{/if}
    </div>
    <div class="side trail"><div class="fit" bind:clientWidth={trail}>{@render trailing?.()}</div></div>
  </div>
</header>
{#if large}<h1 class="display big" bind:this={big} tabindex="-1">{title}</h1>{/if}

<style>
  .navbar {
    position: sticky; top: 0; z-index: 3; flex: none;
    padding: var(--nav-inset, 0px) 0 0;
    transition: background-color var(--t-fast), box-shadow var(--t-fast);
  }
  /* The iPhone's bar material: the page's colour, see-through and blurred, with a hairline under it. */
  .navbar.scrolled {
    background: color-mix(in srgb, var(--nav-bg, var(--bg)) var(--bar-alpha), transparent);
    -webkit-backdrop-filter: var(--bar-blur); backdrop-filter: var(--bar-blur);
    box-shadow: 0 .5px 0 var(--line);
  }
  .row {
    display: grid; grid-template-columns: var(--side) minmax(0, 1fr) var(--side); align-items: center;
    height: 44px; padding: 0 8px;
  }
  .side { display: flex; min-width: 0; }
  .trail { justify-content: flex-end; }
  .fit { display: flex; align-items: center; gap: 2px; }
  .mid { min-width: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; line-height: 1.2; }
  .large .mid { flex-direction: row; gap: 6px; opacity: 0; transition: opacity var(--t-fast); }
  .large .mid.shown { opacity: 1; }
  .title { max-width: 100%; margin: 0; font: 600 17px/1.25 var(--display); color: var(--ink); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .title:focus { outline: none; }
  .sub { max-width: 100%; margin: 0; font-size: 12px; color: var(--ink-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub.busy { display: flex; align-items: center; gap: 5px; }
  .spin { width: 11px; height: 11px; flex: none; border-radius: 50%; border: 1.5px solid currentColor; border-right-color: transparent; }
  .spin.top { order: -1; color: var(--ink-3); }
  @media (prefers-reduced-motion: no-preference) { .spin { animation: spin .8s linear infinite; } }
  @keyframes spin { to { transform: rotate(1turn); } }
  .big { margin: 0; padding: 2px 16px 10px; font-size: 34px; font-weight: 700; line-height: 1.15; letter-spacing: -.01em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .big:focus { outline: none; }
  /* Plain icons: no fill, and the accent, like an iPhone's bar buttons. */
  .fit :global(.icon-btn) { width: 44px; height: 44px; color: var(--lamp); }
  @media (hover: hover) { .fit :global(.icon-btn:hover:not(:disabled)) { background: none; color: var(--lamp); opacity: .75; } }
  .fit :global(.icon-btn:active:not(:disabled)) { background: none; opacity: .5; }
  .fit :global(.text-btn) { min-height: 44px; padding: 0 8px; color: var(--lamp); font-size: 17px; white-space: nowrap; }
  .fit :global(.text-btn:active) { opacity: .5; }
</style>
