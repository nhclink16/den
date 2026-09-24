<script lang="ts">
  import { customizer } from '../lib/customizer.svelte'
  import { modal, outsideDialog } from '../lib/modal'
  import { plugins } from '../plugins'
  import { callLayouts } from '../lib/call-layout.svelte'
  import { call } from '../lib/call.svelte'
  import { store, instances, type Store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'

  let { onclose }: { onclose: () => void } = $props()
  let q = $state('')
  let cursor = $state(0)
  let input: HTMLInputElement

  type Item = { id: string; label: string; hint?: string; disabled?: boolean; kind: 'channel' | 'dm' | 'person' | 'action'; run: () => void; userId?: string; instance?: Store; server?: string }

  const items = $derived.by<Item[]>(() => {
    const out: Item[] = plugins.flatMap((p) => p.paletteActions.map((a) => ({ ...a, kind: 'action' as const })))
    if (call.channel) out.unshift({ id: 'a-call-reset', label: 'Reset layout', kind: 'action', run: () => callLayouts.reset(call.channel?.id) })
    for (const s of instances.all) {
    for (const c of s.textChannels) out.push({ id: s.origin + c.id, instance: s, server: instances.all.length > 1 ? s.settings.instance_name : undefined, label: c.name, hint: s.categories.find((x) => x.id === c.category_id)?.name, kind: 'channel', run: () => { instances.select(s); if (c.kind === 'voice') void call.join(c); else router.go(`/c/${c.id}`) } })
    for (const c of s.dms) out.push({ id: s.origin + c.id, instance: s, server: instances.all.length > 1 ? s.settings.instance_name : undefined, label: s.title(c), hint: 'direct', kind: 'dm', run: () => { instances.select(s); router.go(`/c/${c.id}`) }, userId: (c.member_ids || []).find((id) => id !== s.me?.id) })
    for (const u of s.users.values()) if (u.id !== s.me?.id) out.push({ id: `${s.origin}:u-${u.id}`, instance: s, server: instances.all.length > 1 ? s.settings.instance_name : undefined, label: u.display_name || u.username, hint: `@${u.username}`, kind: 'person', userId: u.id, run: async () => { const c = await s.openDm([u.id]); instances.select(s); router.go(`/c/${c.id}`) } })
    }
    out.push(
      { id: 'a-inbox', label: 'Inbox', hint: 'action', kind: 'action', run: () => router.go('/inbox') },
      ...(q.trim().length > 1 ? [{ id: 'a-search', label: `Search messages for "${q.trim()}"`, hint: 'search', kind: 'action' as const, run: () => router.go(`/find?q=${encodeURIComponent(q.trim())}`) }] : []),
      { id: 'a-settings', label: 'Settings', hint: 'action', kind: 'action', run: () => router.go('/settings') },
      { id: 'a-appearance', label: 'Appearance: themes and wallpaper', hint: 'action', kind: 'action', run: () => customizer.show() },
      { id: 'a-sidebar', label: store.layout.sidebar ? 'Hide room list' : 'Show room list', hint: 'Ctrl+\\', kind: 'action', run: () => store.saveLayout({ sidebar: !store.layout.sidebar }) },
      { id: 'a-members', label: store.layout.members ? 'Hide people' : 'Show people', hint: 'Ctrl+Shift+M', kind: 'action', run: () => store.saveLayout({ members: !store.layout.members }) },
      { id: 'a-logout', label: 'Log out', hint: 'action', kind: 'action', run: () => { store.logout(); router.go('/login') } },
    )
    return out
  })

  // Subsequence match, score by tightness. Good enough for a few dozen items.
  function score(s: string, needle: string): number {
    const hay = s.toLowerCase(); let i = 0, gaps = 0
    for (const ch of needle) { const j = hay.indexOf(ch, i); if (j < 0) return -1; gaps += j - i; i = j + 1 }
    return 1000 - gaps - (hay.startsWith(needle) ? -100 : 0)
  }
  const results = $derived.by(() => {
    const n = q.trim().toLowerCase().replace(/^[#@]/, '')
    if (!n) return items.slice(0, 12)
    return items.map((it) => ({ it, s: Math.max(score(it.label, n), it.hint ? score(it.hint, n) - 50 : -1) })).filter((x) => x.s >= 0).sort((a, b) => b.s - a.s).slice(0, 12).map((x) => x.it)
  })
  $effect(() => { void results; cursor = 0 })
  const optionId = (id: string) => `palette-option-${encodeURIComponent(id)}`
  $effect(() => { if (results[cursor]) document.getElementById(optionId(results[cursor]!.id))?.scrollIntoView({ block: 'nearest' }) })

  function pick(it: Item) { if (it.disabled) return; it.run(); onclose() }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') { e.preventDefault(); cursor = Math.min(cursor + 1, results.length - 1) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); cursor = Math.max(cursor - 1, 0) }
    else if (e.key === 'Enter') { e.preventDefault(); const it = results[cursor]; if (it) pick(it) }

  }
</script>

<dialog class="palette" use:modal aria-label="Go to" aria-modal="true" oncancel={(e) => { e.preventDefault(); onclose() }} onclick={(e) => { if (outsideDialog(e)) onclose() }}>
  <div class="search"><Icon name="search" /><input data-initial-focus role="combobox" aria-label="Jump to a room, a person, or an action" aria-autocomplete="list" aria-expanded="true" aria-controls="palette-results" aria-activedescendant={results[cursor] ? optionId(results[cursor]!.id) : undefined} bind:this={input} bind:value={q} placeholder="Jump to a room, a person, or an action" onkeydown={key} /></div>
  <ul id="palette-results" role="listbox" aria-label="Rooms, people, and actions">
    {#each results as it, i (it.id)}
      <li role="presentation">
        <button id={optionId(it.id)} role="option" aria-selected={i === cursor} tabindex="-1" disabled={it.disabled} class:active={i === cursor} onmousemove={() => (cursor = i)} onclick={() => pick(it)}>
          {#if it.kind === 'channel'}<Icon name="hash" />{:else if it.userId}<Avatar instance={it.instance} userId={it.userId} size={18} />{:else}<Icon name="gear" />{/if}
          <span class="label">{it.label}</span>
          {#if it.server}<span class="hint">{it.server}</span>{/if}
          {#if it.hint}<span class="hint">{it.hint}</span>{/if}
        </button>
      </li>
    {/each}
    {#if !results.length}<li role="presentation" class="none faint">Nothing matches</li>{/if}
  </ul>
</dialog>

<style>
  .palette::backdrop { background: var(--scrim); }
  .palette {
    margin: 0; padding: 0; color: var(--ink); overscroll-behavior: contain;
    position: fixed; z-index: 31; top: 12vh; left: 50%; transform: translateX(-50%);
    width: min(560px, calc(100vw - 32px)); background: var(--bg-2); border: 1px solid var(--line);
    border-radius: var(--r-lg); box-shadow: 0 20px 60px var(--shadow-lg), 0 0 0 1px color-mix(in srgb, var(--bg) 50%, transparent); overflow: hidden;
  }
  @media (prefers-reduced-motion: no-preference) {
    .palette[open] { animation: pop .18s var(--ease-out); }
    .palette[open]::backdrop { animation: fade .18s ease-out; }
  }
  @keyframes pop { from { opacity: 0; transform: translateX(-50%) translateY(-6px) scale(.98); } }
  @keyframes fade { from { opacity: 0; } }
  .search { display: flex; align-items: center; gap: 10px; padding: 12px 14px; border-bottom: 1px solid var(--line); color: var(--ink-3); }
  .search input { flex: 1; background: none; border: 0; outline: 0; font-size: 16px; color: var(--ink); }
  .search:focus-within { box-shadow: inset 0 -1px 0 var(--accent); }
  ul { list-style: none; margin: 0; padding: 6px; max-height: 50vh; overflow-y: auto; }
  li button { width: 100%; display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: var(--r); text-align: left; color: var(--ink-2); }
  li button { position: relative; }
  li button.active { background: var(--bg-3); color: var(--ink); }
  li button.active::before { content: ''; position: absolute; left: -6px; top: 20%; bottom: 20%; width: 3px; border-radius: 0 3px 3px 0; background: var(--lamp); box-shadow: var(--glow); }
  li button :global(svg) { color: var(--ink-3); }
  li button.active :global(svg) { color: var(--lamp); }
  .label { flex: 1; }
  .hint { font-family: var(--mono); font-size: 11px; color: var(--ink-3); }
  .none { padding: 12px; text-align: center; }
</style>
