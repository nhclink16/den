<script lang="ts">
  import type { Snippet } from 'svelte'
  import { store } from '../lib/store.svelte'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import { loadEmoji, pushRecent, readRecent, saveRecent, searchEmoji, type EmojiEntry } from '../lib/emoji'
  import Icon from './Icon.svelte'

  // A button that opens a searchable emoji picker. `quick` is an optional first
  // row (a message's quick reactions). Picking calls `onpick` and closes. On a
  // phone it is a sheet along the bottom of the screen.
  let { label, onpick, children, quick = [], align = 'start', disabled = false, open = $bindable(false) }: {
    label: string; onpick: (emoji: string) => void; children: Snippet; quick?: string[]; align?: 'start' | 'end'; disabled?: boolean; open?: boolean
  } = $props()

  let details = $state<HTMLDetailsElement>()
  let scroller = $state<HTMLElement>()
  let entries = $state<EmojiEntry[]>([])
  let recent = $state<string[]>([])
  let failed = $state(false)
  let q = $state('')
  let current = $state('Recent')

  $effect(() => {
    if (!open) { q = ''; current = 'Recent'; return }
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
    // Recent shows straight away, before the full list has loaded, and always has a tab.
    out.push({ name: 'Recent', items: recent.map((e) => known.get(e) ?? { emoji: e, name: e, keywords: [], category: 'Recent' }) })
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
  // The tab for the section at the top of the list is lit, as on the iOS emoji keyboard.
  function track() {
    let at = sections[0]?.name ?? ''
    for (const section of scroller!.querySelectorAll<HTMLElement>('[data-section]')) {
      if (section.offsetTop > scroller!.scrollTop + 8) break
      at = section.dataset.section!
    }
    current = at
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
<details bind:this={details} bind:open onkeydown={(e) => { if (e.key === 'Escape' && open) { e.preventDefault(); open = false; details?.querySelector('summary')?.focus() } }}>
  <summary class:disabled title={label} aria-label={label}>{@render children()}</summary>
  <div class="sheet" popover="auto" use:disclosurePopover={align}>
    {#if open}
      <div class="picker">
        {#if quick.length}
          <div class="quick">
            {#each quick as e (e)}<button type="button" onclick={() => pick(e)} aria-label={known.get(e)?.name ?? e}>{e}</button>{/each}
          </div>
        {/if}
        <label class="search">
          <Icon name="search" size={17} />
          <input
            bind:value={q} placeholder="Search emoji" aria-label="Search emoji" spellcheck="false" autocomplete="off" autocapitalize="off" autocorrect="off" use:focusOnDesktop
            onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); if (results?.[0]) pick(results[0].emoji) } }}
          />
          {#if q}<button type="button" class="clear" onclick={() => (q = '')} aria-label="Clear search"><Icon name="x" size={12} /></button>{/if}
        </label>
        {#if !results}
          <div class="tabs">
            {#each sections as s (s.name)}
              <button type="button" title={s.name} aria-label={s.name} aria-current={current === s.name} onclick={() => jump(s.name)}>{ICONS[s.name] ?? s.items[0]?.emoji}</button>
            {/each}
          </div>
        {/if}
        <div class="results" bind:this={scroller} onscroll={() => { if (!results) track() }}>
          {#if results}
            {#if results.length}{@render grid(results)}{:else}<p class="note">No emoji for “{q.trim()}”.</p>{/if}
          {:else}
            {#each sections as s (s.name)}
              <section data-section={s.name}>
                <h4>{s.name}</h4>
                {#if s.items.length}{@render grid(s.items)}{:else}<p class="note quiet">Emoji you use show up here.</p>{/if}
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
  @media (hover: hover) { .quick button:hover { background: var(--bg-3); transform: scale(1.12); } }
  /* An iOS search field: filled, no outline, the placeholder in app.css's colour. */
  .search { display: flex; align-items: center; gap: 6px; margin: 10px 10px 6px; padding: 0 8px; height: 36px; flex: none; border-radius: max(10px, var(--r)); background: color-mix(in srgb, var(--ink-3) 16%, transparent); color: var(--ink-3); transition: box-shadow var(--t-fast); }
  :global(:where(:root:not([data-input='touch']))) .search:focus-within { box-shadow: 0 0 0 2px var(--lamp-glow); }
  .search input { flex: 1; min-width: 0; background: none; border: 0; outline: 0; color: var(--ink); }
  .clear { display: grid; padding: 4px; border-radius: var(--r); color: var(--ink-3); }
  @media (hover: hover) { .clear:hover { color: var(--ink); background: var(--bg-2); } }
  .tabs { display: flex; gap: 2px; padding: 0 8px 6px; border-bottom: 1px solid var(--line); overflow-x: auto; scrollbar-width: none; flex: none; }
  .tabs button { flex: 1 0 auto; min-width: 32px; height: 32px; font-size: 17px; border-radius: var(--r); transition: background-color var(--t-fast); }
  .tabs button[aria-current=true] { background: var(--bg-3); box-shadow: inset 0 -2px 0 var(--lamp); }
  @media (hover: hover) { .tabs button:hover { background: var(--bg-3); } }
  .results { position: relative; flex: 1; min-height: 0; overflow-y: auto; padding: 0 8px 8px; overscroll-behavior: contain; }
  h4 { position: sticky; top: 0; z-index: 1; margin: 0; padding: 8px 4px 4px; background: var(--bg-2); color: var(--ink-3); font-size: var(--fs-xs); font-weight: 600; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(38px, 1fr)); padding-top: 4px; }
  .grid button { height: 38px; font-size: 24px; line-height: 1; border-radius: var(--r); transition: background-color var(--t-fast); }
  .grid button:focus-visible { background: var(--bg-3); }
  @media (hover: hover) { .grid button:hover { background: var(--bg-3); } }
  .quick button:active, .tabs button:active, .grid button:active { background: var(--press); }
  .note { margin: 24px 12px; text-align: center; color: var(--ink-2); }
  .note.quiet { margin: 8px 4px 16px; color: var(--ink-3); font-size: var(--fs-sm); }
  /* A phone gets a sheet the width of the screen, with thumb-sized targets. */
  @media (max-width: 600px) {
    .picker { width: 100%; height: min(440px, 62vh); }
    .grid { grid-template-columns: repeat(auto-fill, minmax(44px, 1fr)); }
    .grid button { height: 44px; font-size: 28px; }
    .tabs button { min-width: 36px; height: 44px; }
    .clear { padding: 16px; margin: -4px -14px -4px 0; }
  }
  @media (prefers-reduced-motion: reduce) { @media (hover: hover) { .quick button:hover { transform: none; } } }
</style>
