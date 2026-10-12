<script lang="ts">
  import type { Snippet } from 'svelte'
  import { store } from '../lib/store.svelte'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import { loadEmoji, pushRecent, readRecent, saveRecent, searchEmoji, type EmojiEntry } from '../lib/emoji'
  import Icon from './Icon.svelte'

  // A button that opens a searchable emoji picker. `quick` is an optional first
  // row (a message's quick reactions). Picking calls `onpick` and closes. On a
  // phone it is a sheet along the bottom of the screen.
  let { label, onpick, children, quick = [], align = 'start', disabled = false }: {
    label: string; onpick: (emoji: string) => void; children: Snippet; quick?: string[]; align?: 'start' | 'end'; disabled?: boolean
  } = $props()

  let open = $state(false)
  let details = $state<HTMLDetailsElement>()
  let scroller = $state<HTMLElement>()
  let entries = $state<EmojiEntry[]>([])
  let recent = $state<string[]>([])
  let failed = $state(false)
  let q = $state('')

  $effect(() => {
    if (!open) { q = ''; return }
    recent = store.me ? readRecent(store.origin, store.me.id) : []
    failed = false
    loadEmoji().then((all) => (entries = all), () => (failed = true))
  })

  // The tab for each category. A category this does not know (custom emoji,
  // one day) uses its own first emoji.
  const ICONS: Record<string, string> = {
    Recent: '🕘', 'Smileys & emotion': '😀', 'People & body': '👋', 'Animals & nature': '🐻', 'Food & drink': '🍔',
    'Travel & places': '✈️', Activities: '⚽', Objects: '💡', Symbols: '💕', Flags: '🏁',
  }
  const known = $derived(new Map(entries.map((e) => [e.emoji, e])))
  const sections = $derived.by(() => {
    const out: { name: string; items: EmojiEntry[] }[] = []
    // Recent shows straight away, before the full list has loaded.
    if (recent.length) out.push({ name: 'Recent', items: recent.map((e) => known.get(e) ?? { emoji: e, name: e, keywords: [], category: 'Recent' }) })
    for (const e of entries) {
      const last = out.at(-1)
      if (last && last.name === e.category) last.items.push(e)
      else out.push({ name: e.category, items: [e] })
    }
    return out
  })
  const results = $derived(q.trim() ? searchEmoji(entries, q) : null)

  function pick(emoji: string) {
    // Re-read first: another tab may have picked something since this opened.
    if (store.me) { recent = pushRecent(readRecent(store.origin, store.me.id), emoji); saveRecent(store.origin, store.me.id, recent) }
    onpick(emoji)
    open = false
  }
  function jump(name: string) {
    const section = scroller?.querySelector<HTMLElement>(`[data-section="${CSS.escape(name)}"]`)
    if (section) scroller!.scrollTo({ top: section.offsetTop })
  }
  // A phone keeps its keyboard down until someone taps the search box: most
  // picks are a scroll and a tap. The box can exist a moment before its popover
  // is shown, and a hidden box can't take focus, so wait a frame.
  function focusOnDesktop(node: HTMLInputElement) {
    if (!matchMedia('(pointer: coarse)').matches) requestAnimationFrame(() => node.focus())
  }
</script>

{#snippet grid(items: EmojiEntry[])}
  <div class="grid">
    {#each items as e (e.emoji)}
      <button type="button" title={e.name} aria-label={e.name} onclick={() => pick(e.emoji)}>{e.emoji}</button>
    {/each}
  </div>
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes this native disclosure.) -->
<details bind:this={details} bind:open onkeydown={(e) => { if (e.key === 'Escape') { e.preventDefault(); open = false; details?.querySelector('summary')?.focus() } }}>
  <summary class:disabled title={label} aria-label={label}>{@render children()}</summary>
  <div popover="auto" use:disclosurePopover={align}>
    {#if open}
      <div class="picker">
        {#if quick.length}
          <div class="quick">
            {#each quick as e (e)}<button type="button" onclick={() => pick(e)} aria-label={known.get(e)?.name ?? e}>{e}</button>{/each}
          </div>
        {/if}
        <label class="search">
          <Icon name="search" size={14} />
          <input
            bind:value={q} placeholder="Search emoji" aria-label="Search emoji" spellcheck="false" autocomplete="off" autocapitalize="off" autocorrect="off" use:focusOnDesktop
            onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); if (results?.[0]) pick(results[0].emoji) } }}
          />
          {#if q}<button type="button" class="clear" onclick={() => (q = '')} aria-label="Clear search"><Icon name="x" size={12} /></button>{/if}
        </label>
        {#if !results}
          <div class="tabs">
            {#each sections as s (s.name)}
              <button type="button" title={s.name} aria-label={s.name} onclick={() => jump(s.name)}>{ICONS[s.name] ?? s.items[0]?.emoji}</button>
            {/each}
          </div>
        {/if}
        <div class="results" bind:this={scroller}>
          {#if results}
            {#if results.length}{@render grid(results)}{:else}<p class="note">No emoji for “{q.trim()}”.</p>{/if}
          {:else}
            {#each sections as s (s.name)}
              <section data-section={s.name}>
                <h4>{s.name}</h4>
                {@render grid(s.items)}
              </section>
            {/each}
            {#if failed}<p class="note">Emoji didn’t load. Close this and try again.</p>
            {:else if !entries.length}<p class="note">Loading emoji…</p>{/if}
          {/if}
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
  [popover] { position: fixed; inset: auto; margin: 0; padding: 0; border: 1px solid var(--line-strong); border-radius: var(--r-lg); background: var(--bg-2); color: var(--ink); box-shadow: 0 14px 44px var(--shadow); overflow: hidden; }
  .picker { width: min(360px, calc(100vw - 16px)); height: min(420px, calc(100vh - 120px)); display: flex; flex-direction: column; }
  .quick { display: flex; justify-content: space-between; padding: 8px 10px 0; }
  .quick button { font-size: 24px; width: 44px; height: 44px; border-radius: var(--r); transition: transform var(--t) var(--ease-out), background-color var(--t-fast); }
  .quick button:hover { background: var(--bg-3); transform: scale(1.12); }
  .search { display: flex; align-items: center; gap: 8px; margin: 10px 10px 6px; padding: 0 10px; height: 36px; flex: none; border-radius: var(--r); background: var(--bg-3); border: 1px solid var(--line); color: var(--ink-3); transition: border-color var(--t-fast), box-shadow var(--t-fast); }
  .search:focus-within { border-color: var(--lamp); box-shadow: 0 0 0 3px var(--lamp-glow); }
  .search input { flex: 1; min-width: 0; background: none; border: 0; outline: 0; color: var(--ink); }
  .search input::placeholder { color: var(--ink-3); }
  .clear { display: grid; padding: 4px; border-radius: var(--r); color: var(--ink-3); }
  .clear:hover { color: var(--ink); background: var(--bg-2); }
  .tabs { display: flex; gap: 2px; padding: 0 8px 6px; border-bottom: 1px solid var(--line); overflow-x: auto; scrollbar-width: none; flex: none; }
  .tabs button { flex: 1 0 auto; min-width: 32px; height: 32px; font-size: 17px; border-radius: var(--r); filter: grayscale(.6); opacity: .8; transition: filter var(--t-fast), opacity var(--t-fast), background-color var(--t-fast); }
  .tabs button:hover { filter: none; opacity: 1; background: var(--bg-3); }
  .results { position: relative; flex: 1; min-height: 0; overflow-y: auto; padding: 0 8px 8px; overscroll-behavior: contain; }
  h4 { position: sticky; top: 0; z-index: 1; margin: 0; padding: 8px 4px 4px; background: var(--bg-2); color: var(--ink-3); font-size: 11px; font-weight: 600; letter-spacing: .04em; text-transform: uppercase; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(38px, 1fr)); padding-top: 4px; }
  .grid button { height: 38px; font-size: 24px; line-height: 1; border-radius: var(--r); transition: background-color var(--t-fast); }
  .grid button:hover, .grid button:focus-visible { background: var(--bg-3); }
  .note { margin: 24px 12px; text-align: center; color: var(--ink-2); }
  /* A phone gets a sheet the width of the screen, with thumb-sized targets. */
  @media (max-width: 600px) {
    [popover] { left: 0 !important; top: auto !important; right: 0; bottom: 0; width: 100%; border-radius: var(--r-lg) var(--r-lg) 0 0; border-width: 1px 0 0; padding-bottom: env(safe-area-inset-bottom); }
    [popover]::backdrop { background: rgb(0 0 0 / .35); }
    .picker { width: 100%; height: min(440px, 62vh); }
    .grid { grid-template-columns: repeat(auto-fill, minmax(44px, 1fr)); }
    .grid button { height: 44px; font-size: 28px; }
    .tabs button { min-width: 36px; height: 44px; }
    .clear { padding: 16px; margin: -4px -14px -4px 0; }
  }
  @media (prefers-reduced-motion: reduce) { .quick button:hover { transform: none; } }
</style>
