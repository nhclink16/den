<script module lang="ts">
  // The key is the server's, fetched once per server when someone first looks.
  const keys = new Map<string, Promise<string | null>>()
</script>

<script lang="ts">
  import type { Snippet } from 'svelte'
  import { store } from '../lib/store.svelte'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import { reducedMotion } from '../lib/motion.svelte'
  import { search, shared, thumb, type Gif, type Session } from '../lib/klipy'
  import type { Klipy } from '../lib/types'
  import Icon from './Icon.svelte'

  // A button that opens GIF search. `onpick` does the work (send, save as a
  // picture); the picker closes once it succeeds and shows its error if not.
  let { label, onpick, children, disabled = false, open = $bindable(false) }: { label: string; onpick: (g: Gif) => Promise<void>; children: Snippet; disabled?: boolean; open?: boolean } = $props()

  let details = $state<HTMLDetailsElement>()
  let q = $state('')
  let gifs = $state<Gif[]>([])
  let page = $state(0)
  let more = $state(false)
  let loading = $state(false)
  let error = $state('')
  let missing = $state(false)
  let picking = $state<number | null>(null)
  let hovered = $state<number | null>(null)
  let scroller = $state<HTMLElement>()
  let sentinel = $state<HTMLElement>()
  let session: Session | null = null
  let controller: AbortController | null = null

  async function connect(): Promise<Session | null> {
    const origin = store.origin, userId = store.me?.id
    if (!userId) return null
    let key = keys.get(origin)
    if (!key) {
      key = store.api.get<Klipy>('/klipy').then((k) => k.app_key ?? null, () => null)
      keys.set(origin, key)
    }
    const k = await key
    return k ? { key: k, origin, userId } : null
  }

  async function load(reset: boolean) {
    controller?.abort()
    const c = (controller = new AbortController())
    if (reset) { page = 0; more = false; error = ''; scroller?.scrollTo({ top: 0 }) }
    loading = true
    try {
      session ??= await connect()
      if (!session) { missing = true; return }
      const result = await search(session, q.trim(), page + 1, c.signal)
      if (c.signal.aborted) return
      page += 1
      gifs = reset ? result.gifs : [...gifs, ...result.gifs]
      more = result.more
    } catch (err) {
      if (!c.signal.aborted) { error = (err as Error).message; if (reset) gifs = [] }
    } finally {
      if (controller === c) loading = false
    }
  }

  // Trending when the box is empty, then search as you type, a beat after the
  // last key so a word costs one request rather than one per letter.
  $effect(() => {
    if (!open) return
    const text = q
    const timer = setTimeout(() => void load(true), text ? 350 : 0)
    return () => clearTimeout(timer)
  })
  $effect(() => {
    if (!open || !sentinel || !scroller) return
    const io = new IntersectionObserver((e) => { if (e[0]?.isIntersecting && more && !loading) void load(false) }, { root: scroller, rootMargin: '240px' })
    io.observe(sentinel)
    return () => io.disconnect()
  })
  $effect(() => { if (!open) { controller?.abort(); picking = null; error = '' } })

  // Two columns, each GIF going to the shorter one, so KLIPY's order still
  // reads left to right and top to bottom.
  const columns = $derived.by(() => {
    const cols: Gif[][] = [[], []], heights = [0, 0]
    for (const g of gifs) {
      const t = thumb(g)!
      const i = heights[0]! <= heights[1]! ? 0 : 1
      cols[i]!.push(g)
      heights[i]! += t.height / t.width
    }
    return cols
  })

  async function pick(g: Gif) {
    if (picking !== null) return
    picking = g.id; error = ''
    try {
      await onpick(g)
      if (session) void shared(session, g, q.trim())
      open = false
      q = ''
    } catch (err) {
      error = (err as Error).message || "That GIF didn't go through."
    } finally { picking = null }
  }
  const still = (g: Gif) => g.file.sm?.jpg ?? g.file.xs?.jpg
  // A phone shows what's trending first; the keyboard comes up when someone taps the box.
  function focusOnDesktop(node: HTMLInputElement) {
    if (!matchMedia('(pointer: coarse)').matches) requestAnimationFrame(() => node.focus())
  }
</script>

{#snippet empty(title: string, line: string)}
  <div class="empty">
    <span class="mark" aria-hidden="true"><span class="badge">GIF</span></span>
    <p class="title">{title}</p>
    <p class="line">{line}</p>
  </div>
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes this native disclosure.) -->
<details bind:this={details} bind:open onkeydown={(e) => { if (e.key === 'Escape' && open) { e.preventDefault(); open = false; details?.querySelector('summary')?.focus() } }}>
  <summary class:disabled title={label} aria-label={label}>{@render children()}</summary>
  <div class="sheet" popover="auto" use:disclosurePopover={'start'}>
    {#if open}
      <div class="picker">
        {#if !missing}
          <label class="search">
            <Icon name="search" size={17} />
            <input bind:value={q} placeholder="Search GIFs" aria-label="Search GIFs" spellcheck="false" use:focusOnDesktop />
            {#if q}<button class="clear" onclick={() => (q = '')} aria-label="Clear search"><Icon name="x" size={12} /></button>{/if}
          </label>
        {/if}
        <div class="results" bind:this={scroller}>
          {#if missing}
            {@render empty('GIFs aren’t set up', 'Whoever runs this Den can add a KLIPY key to turn them on.')}
          {:else if !gifs.length && error}
            {@render empty('GIFs didn’t load', error)}
          {:else if !gifs.length && !loading}
            {@render empty('No GIFs found', `Nothing for “${q.trim()}”. Try another word.`)}
          {:else}
            <div class="grid" class:waiting={loading && page === 0}>
              {#each columns as column, c (c)}
                <div class="col">
                  {#each column as g (g.id)}
                    {@const t = thumb(g)!}
                    {@const s = still(g)}
                    <button
                      class="tile" class:busy={picking === g.id}
                      style:aspect-ratio="{t.width} / {t.height}"
                      style:background-image={g.blur_preview ? `url("${g.blur_preview}")` : undefined}
                      disabled={picking !== null}
                      title={g.title}
                      onclick={() => pick(g)}
                      onpointerenter={() => (hovered = g.id)} onpointerleave={() => (hovered = null)}
                      onfocus={() => (hovered = g.id)} onblur={() => (hovered = null)}
                    >
                      <img src={reducedMotion.current && s && hovered !== g.id ? s.url : t.url} alt={g.title} loading="lazy" draggable="false" />
                      {#if picking === g.id}<span class="spin" aria-hidden="true"></span>{/if}
                    </button>
                  {/each}
                </div>
              {/each}
            </div>
            {#if gifs.length && error}<p class="note" role="alert">{error}</p>{/if}
          {/if}
          <div class="sentinel" bind:this={sentinel}></div>
        </div>
        <div class="foot" class:hidden={missing}>
          <span class="faint">{q.trim() ? 'Results' : 'Trending'}</span>
          <span class="powered">Powered by <b>KLIPY</b></span>
        </div>
      </div>
    {/if}
  </div>
</details>

<style>
  /* The popover places itself against this box, so it must have one. */
  details { display: flex; }
  summary { list-style: none; cursor: pointer; color: var(--ink-2); }
  details[open] > summary { color: var(--lamp); }
  summary::-webkit-details-marker { display: none; }
  summary.disabled { pointer-events: none; opacity: .5; }
  [popover] { position: fixed; inset: auto; margin: 0; padding: 0; border: 1px solid var(--line-strong); border-radius: var(--r-lg); background: var(--bg-2); box-shadow: 0 14px 44px var(--shadow); overflow: hidden; }
  .picker { width: min(400px, calc(100vw - 16px)); height: min(460px, calc(100vh - 120px)); display: flex; flex-direction: column; }
  /* An iOS search field: filled, no outline, the placeholder in app.css's colour. */
  .search { display: flex; align-items: center; gap: 6px; margin: 10px 10px 8px; padding: 0 8px; height: 36px; flex: none; border-radius: max(10px, var(--r)); background: color-mix(in srgb, var(--ink-3) 16%, transparent); color: var(--ink-3); transition: box-shadow var(--t-fast); }
  :global(:where(:root:not([data-input='touch']))) .search:focus-within { box-shadow: 0 0 0 2px var(--lamp-glow); }
  .search input { flex: 1; min-width: 0; background: none; border: 0; outline: 0; color: var(--ink); }
  .clear { display: grid; padding: 4px; border-radius: var(--r); color: var(--ink-3); }
  @media (hover: hover) { .clear:hover { color: var(--ink); background: var(--bg-2); } }
  .results { flex: 1; min-height: 0; overflow-y: auto; padding: 0 10px; overscroll-behavior: contain; }
  .grid { display: flex; gap: 6px; align-items: flex-start; transition: opacity var(--t); }
  .grid.waiting { opacity: .5; }
  .col { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 6px; }
  .tile { position: relative; display: block; width: 100%; padding: 0; overflow: hidden; border-radius: calc(var(--r) * .75); background: var(--bg-3) center / cover no-repeat; transition: transform var(--t-fast) var(--ease-out), box-shadow var(--t-fast); }
  .tile img { display: block; width: 100%; height: 100%; object-fit: cover; }
  :global(:where(:root:not([data-input]))) .tile:focus-visible { transform: scale(1.02); box-shadow: 0 0 0 2px var(--lamp), var(--glow); z-index: 1; }
  @media (hover: hover) { .tile:hover:not(:disabled) { transform: scale(1.02); box-shadow: 0 0 0 2px var(--lamp), var(--glow); z-index: 1; } }
  .tile:focus-visible { outline: none; }
  .tile:disabled:not(.busy) { opacity: .6; }
  .tile.busy::after { content: ''; position: absolute; inset: 0; background: color-mix(in srgb, var(--bg) 45%, transparent); }
  .spin { position: absolute; z-index: 1; top: 50%; left: 50%; width: 20px; height: 20px; margin: -10px 0 0 -10px; border-radius: 50%; border: 2px solid var(--ink); border-right-color: transparent; animation: spin .7s linear infinite; }
  @keyframes spin { to { transform: rotate(1turn); } }
  .note { margin: 28px 12px; text-align: center; color: var(--ink-2); }
  /* Nothing to show: a mark, a title and one line, in the middle. */
  .empty { min-height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; padding: 24px 28px; text-align: center; }
  .empty .mark { display: grid; place-items: center; width: 56px; height: 56px; margin-bottom: 6px; border-radius: 50%; background: var(--bg-3); color: var(--ink-3); }
  .empty .badge { font: 800 12px/1 var(--body); letter-spacing: .04em; padding: 4px 5px 3.5px; border: 2px solid currentColor; border-radius: 5px; }
  .empty p { margin: 0; }
  .empty .title { font-size: var(--fs-body); font-weight: 600; color: var(--ink); }
  .empty .line { max-width: 30ch; font-size: var(--fs-sm); color: var(--ink-2); text-wrap: pretty; }
  .foot.hidden { display: none; }
  .sentinel { height: 1px; }
  .foot { display: flex; justify-content: space-between; align-items: center; padding: 7px 12px; border-top: 1px solid var(--line); font-size: 11px; }
  .powered { color: var(--ink-3); letter-spacing: .02em; }
  .powered b { color: var(--ink-2); font-weight: 700; letter-spacing: .06em; }
  @media (max-width: 600px) { .picker { width: 100%; height: min(520px, 70vh); } }
  @media (prefers-reduced-motion: reduce) { .tile { transform: none; }
 @media (hover: hover) { .tile:hover:not(:disabled) { transform: none; } } }
</style>
