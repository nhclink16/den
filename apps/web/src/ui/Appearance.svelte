<script lang="ts">
  import { onDestroy, untrack } from 'svelte'
  import { fontStack, themeFor } from '../lib/theme-runtime'
  import { accentPresets, colorRoles, deriveHalf, resolveAccent } from '../lib/theme-color'
  import { themes, fontFamilies, validateTheme, validName, webFonts } from '../lib/theme.svelte'
  import type { Theme, ThemeFonts } from '../lib/types'
  import ThemePreview from './ThemePreview.svelte'
  import ThemeCard from './ThemeCard.svelte'
  import BackgroundSettings from './BackgroundSettings.svelte'
  import SettingRow from './SettingRow.svelte'
  import BubblePreview from './BubblePreview.svelte'
  import AppearancePreview from './AppearancePreview.svelte'
  import { store, type AvatarShape, type PresenceStyle } from '../lib/store.svelte'
  import { customizer } from '../lib/customizer.svelte'

  /** `panel`: shown in the live customizer over the app. Otherwise it is the phone's Appearance page, with its own preview. */
  let { panel = false }: { panel?: boolean } = $props()
  type Half = 'light' | 'dark'

  let editor = $state(false)
  let editingHalf = $state<Half>('dark')
  let naming = $state(false)
  let name = $state('')
  let message = $state('')
  let fileInput: HTMLInputElement
  let picker = $state<HTMLInputElement>()

  const active = $derived(themes.active)
  const appearance = $derived(themes.appearance)
  /** A theme's half as it shows with the chosen accent. */
  const shown = (t: Theme, half: Half) => ({ ...t[half], accent: resolveAccent(t[half], appearance.accent, half) })
  const custom = $derived(appearance.accent?.startsWith('#') ? appearance.accent : null)
  const swatch = (choice: string | null) => resolveAccent(themeFor(appearance, themes.half)[themes.half], choice, themes.half)
  const roleLabels = {
    bg: ['Page background', 'The main conversation surface.'],
    bg2: ['Panel background', 'Sidebar and settings panels.'],
    bg3: ['Raised background', 'Controls, chips, and raised surfaces.'],
    line: ['Dividers', 'Hairline separators between panels.'],
    ink: ['Text', 'Messages and primary labels.'],
    ink2: ['Secondary text', 'Descriptions and supporting labels.'],
    ink3: ['Faint text', 'Timestamps and hints.'],
    accent: ['Accent', 'Links, mentions, and selected controls.'],
    success: ['Success', 'Successful actions and positive status.'],
    danger: ['Danger', 'Errors and destructive actions.'],
  } as const
  const fontRoles: (keyof ThemeFonts)[] = ['display', 'body', 'mono']
  const bubbleShapes = [['corners', 'Match corners'], ['round', 'Round'], ['soft', 'Soft'], ['sharp', 'Sharp'], ['none', 'No bubbles']] as const
  const bubbleHint = $derived({
    everyone: 'Each name in that person\'s colour, picked in Profile. Yours in the accent.',
    mine: 'Your bubbles in your own colour. Names grey.',
    plain: 'Your bubbles in the accent. Names grey.',
  }[appearance.bubble_colors ?? 'everyone'] + (appearance.bubble_shape === 'none' ? ' With no bubbles, the colour goes on names.' : ''))
  const isCustom = (t: Theme) => appearance.custom_themes.some(c => c.id === t.id)

  const schemes = [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']] as const

  function change(patch: Partial<Theme>) {
    message = ''
    themes.preview({ ...(JSON.parse(JSON.stringify(active)) as Theme), ...patch })
  }
  /** Corners, density and fonts. Your own theme saves as you go; a built-in one stays a draft to save as your own. */
  function shape(patch: Partial<Theme>) {
    if (editor || themes.draft || !isCustom(active)) return change(patch)
    message = ''
    void themes.update({ ...(JSON.parse(JSON.stringify(active)) as Theme), ...patch })
  }
  function color(role: (typeof colorRoles)[number], value: string) {
    if (!/^#[0-9a-f]{6}$/i.test(value)) return
    const { generated: _drop, ...colors } = active[editingHalf]
    change({ [editingHalf]: { ...colors, [role]: value } })
  }
  function font(role: keyof ThemeFonts, value: string) {
    if (validName(value)) shape({ fonts: { ...active.fonts, [role]: value } })
    else themes.error = 'Font names must be 1–40 letters, numbers, spaces, hyphens or underscores.'
  }
  function openEditor(t: Theme) {
    themes.preview(JSON.parse(JSON.stringify(t)) as Theme)
    editingHalf = themes.half
    editor = true
  }
  async function save(e: SubmitEvent) {
    e.preventDefault()
    try {
      await themes.add({ ...(JSON.parse(JSON.stringify(active)) as Theme), id: crypto.randomUUID(), name: name.trim() })
      naming = false
      editor = false
      message = 'Theme saved.'
    } catch (err) { themes.error = (err as Error).message }
  }
  function exportTheme() {
    const blob = new Blob([JSON.stringify(active, null, 2) + '\n'], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${active.name}.den-theme.json`
    a.click()
    setTimeout(() => URL.revokeObjectURL(url), 1000)
  }
  async function importTheme(e: Event) {
    const file = (e.target as HTMLInputElement).files?.[0]
    if (!file) return
    try {
      if (file.size > 16384) throw Error('Theme files must be at most 16 KiB.')
      const t = validateTheme(JSON.parse(await file.text()))
      if (themes.all.some(v => v.id === t.id)) t.id = crypto.randomUUID()
      await themes.add(t)
      message = 'Theme imported.'
    } catch (err) { themes.error = (err as Error).message }
    fileInput.value = ''
  }

  // Preview the actual heading, body, and monospace faces for every theme.
  $effect(() => {
    const families = webFonts(themes.all.flatMap(t => Object.values(t.fonts)))
    if (!families.length) return
    const link = document.createElement('link')
    link.rel = 'stylesheet'
    link.href = `https://fonts.googleapis.com/css2?${families.map(f => `family=${encodeURIComponent(f)}:wght@400;600`).join('&')}&display=swap`
    document.head.append(link)
    return () => link.remove()
  })

  // A phone has no app behind the editor, so the whole page shows the half being edited.
  $effect(() => {
    if (panel || !editor) return
    const half = editingHalf
    untrack(() => themes.showHalf(half))
    return () => untrack(() => themes.showHalf(null))
  })

  // Widening the window swaps this page for the panel over the room; unsaved theme edits go with it.
  onDestroy(() => { if (!panel && customizer.open) return; themes.reset() })
</script>

<div class="appearance" class:panel>
  {#if !panel}
    <h2 class="display">Appearance</h2>
    <AppearancePreview />
  {/if}

  <div class="slist first">
    <SettingRow find="mode" label="Light or dark" hint={appearance.mode === 'system' ? 'Follows your device.' : undefined}>
      {#snippet control()}
        <div class="segments" role="group" aria-label="Light or dark">
          {#each schemes as [id, label] (id)}
            <button aria-pressed={appearance.mode === id} onclick={() => themes.mode(id)}>{label}</button>
          {/each}
        </div>
      {/snippet}
    </SettingRow>
  </div>

  <h3 class="list-head">Theme</h3>
  <div class="slist" data-find="themes">
    <div class="theme-grid">
      {#each themes.all as t (t.id)}
        <ThemeCard
          theme={t}
          lightChosen={appearance.light_theme === t.id}
          darkChosen={appearance.dark_theme === t.id}
          dimmed={appearance.mode === 'system' ? undefined : appearance.mode}
          only={!panel && appearance.mode !== 'system' ? appearance.mode : undefined}
          accent={appearance.accent}
          onpick={half => { themes.select(t, half); message = '' }}
          oncustomise={panel ? () => openEditor(t) : undefined}
          onremove={isCustom(t) ? () => themes.remove(t.id) : undefined}
        />
      {/each}
    </div>
    {#if panel}
      <SettingRow find="custom" label="Your own theme" hint="Start from the one you're using, or import a theme file.">
        {#snippet control()}
          <button class="btn" onclick={() => openEditor(active)}>Create theme</button>
          <button class="btn" onclick={() => fileInput.click()}>Import</button>
        {/snippet}
      </SettingRow>
    {:else}
      <!-- A phone edits the theme it shows: start from it, or bring one in. -->
      <button class="action-row" data-find="custom" onclick={() => openEditor(active)}>Edit {active.name} as Your Own</button>
      <button class="action-row" onclick={() => fileInput.click()}>Import a Theme</button>
    {/if}
    <input class="sr-only" tabindex="-1" bind:this={fileInput} type="file" accept=".json,application/json" aria-label="Import theme file" onchange={importTheme} />
  </div>
  <p class="list-foot">{!panel && appearance.mode !== 'system' ? `The theme for ${appearance.mode}. With System, each theme has a light and a dark side to pick on its own.` : "Each theme has a light and a dark half. Pick one theme for light and another for dark; they don't have to match."}</p>

  {#if editor}
    <h3 class="list-head"><span>Editing {active.name}</span><button class="head-btn" onclick={() => { editor = false; themes.reset() }}>Close</button></h3>
    <section class="slist editor" aria-label="Theme editor">
      <div class="editor-side">
        <p class="hint muted">Every change shows {panel ? 'in the app behind this panel' : 'in the preview at the top'} as you make it.</p>
        <div class="segments" role="group" aria-label="Edit appearance">
          {#each ['light', 'dark'] as half (half)}
            <button aria-pressed={editingHalf === half} onclick={() => (editingHalf = half as Half)}>{half === 'light' ? 'Light' : 'Dark'}</button>
          {/each}
        </div>
        {#if panel}<div class="editor-preview"><ThemePreview colors={shown(active, editingHalf)} fonts={active.fonts} /></div>{/if}
        {#if active[editingHalf].generated}
          <p class="hint muted">Generated from the {editingHalf === 'light' ? 'dark' : 'light'} half. Adjust to taste.</p>
        {/if}
        <button class="link" onclick={() => change({ [editingHalf]: deriveHalf(active[editingHalf === 'light' ? 'dark' : 'light']) })}>
          Copy from {editingHalf === 'light' ? 'dark' : 'light'}
        </button>
      </div>
      {#each colorRoles as role (role)}
        <SettingRow label={roleLabels[role][0]} hint={roleLabels[role][1]}>
          {#snippet control()}
            <input type="color" class="swatch" aria-label={`${roleLabels[role][0]} color`} value={active[editingHalf][role]} oninput={e => color(role, e.currentTarget.value)} />
            <input class="field mono hex" aria-label={`${roleLabels[role][0]} hex`} value={active[editingHalf][role]} maxlength="7" pattern="#[0-9a-fA-F]{'{'}6{'}'}" oninput={e => color(role, e.currentTarget.value)} />
          {/snippet}
        </SettingRow>
      {/each}
      <div class="editor-actions">
        <button class="btn lit" onclick={() => { naming = true; name = `${active.name} custom`.slice(0, 40) }}>Save as new theme</button>
        <button class="btn" onclick={exportTheme}>Export</button>
        <button class="btn quiet" onclick={() => { themes.reset(); message = ''; themes.error = '' }}>Discard changes</button>
      </div>
      {#if naming}
        <form class="save-name" onsubmit={save}>
          <label>Theme name<input class="field" bind:value={name} maxlength="40" required /></label>
          <button class="btn lit" type="submit">Save</button>
          <button class="btn quiet" type="button" onclick={() => (naming = false)}>Cancel</button>
        </form>
      {/if}
    </section>
  {/if}

  <h3 class="list-head">Accent</h3>
  {#if !panel}
    <!-- A phone: one row of colour circles, the chosen one ringed. -->
    <div class="slist" data-find="accent">
      <div class="pad circles" role="radiogroup" aria-label="Accent">
        <!-- Real radios: arrow keys move between colours, one Tab stop. Custom opens the colour picker instead. -->
        {#each [[null, "Theme's own"], ...accentPresets.map(p => [p.id, p.name])] as [id, name] (id ?? 'theme')}
          <label class="circle" class:on={(appearance.accent ?? null) === id} title={name} style:--c={swatch(id)}><input type="radio" class="sr-only" name="accent" aria-label={name} checked={(appearance.accent ?? null) === id} onchange={() => themes.accent(id)} /></label>
        {/each}
        <button class="circle custom" class:on={!!custom} aria-pressed={!!custom} aria-label="Custom accent" title="Custom" style:--c={custom ? swatch(custom) : 'conic-gradient(#e5484d, #f5a524, #3fcf7a, #1d63d8, #8f8cf9, #e5484d)'} onclick={() => picker?.click()}></button>
        <input type="color" class="sr-only" tabindex="-1" aria-label="Custom accent colour" bind:this={picker} value={custom ?? swatch(null)} onchange={e => themes.accent(e.currentTarget.value)} />
      </div>
    </div>
    <p class="list-foot">{custom ? 'Custom' : appearance.accent ? accentPresets.find(p => p.id === appearance.accent)?.name : "The theme's own"}. Buttons, links, unread counts and mentions, kept readable in light and dark.</p>
  {:else}
  <div class="slist" data-find="accent">
    <div class="pad accents" role="group" aria-label="Accent">
      <button class="pill" aria-pressed={!appearance.accent} onclick={() => themes.accent(null)}><span class="dot" style:background={swatch(null)}></span>Theme's own</button>
      {#each accentPresets as p (p.id)}
        <button class="pill" aria-pressed={appearance.accent === p.id} onclick={() => themes.accent(p.id)}><span class="dot" style:background={swatch(p.id)}></span>{p.name}</button>
      {/each}
      <button class="pill" aria-pressed={!!custom} onclick={() => picker?.click()}>
        <span class="dot" style:background={custom ? swatch(custom) : 'conic-gradient(#e5484d, #f5a524, #3fcf7a, #1d63d8, #8f8cf9, #e5484d)'}></span>Custom
      </button>
      <input type="color" class="sr-only" tabindex="-1" aria-label="Custom accent colour" bind:this={picker} value={custom ?? swatch(null)} onchange={e => themes.accent(e.currentTarget.value)} />
    </div>
  </div>
  <p class="list-foot">Buttons, links, unread counts and mentions, in every theme. Den keeps it readable in light and dark.</p>
  {/if}

  <h3 class="list-head">Shape and size</h3>
  <div class="slist">
    <SettingRow find="shape" label="Corners">
      {#snippet control()}
        <div class="segments" role="group" aria-label="Corner radius">
          {#each ['sharp', 'soft', 'round'] as radius (radius)}
            <button aria-pressed={active.radius === radius} onclick={() => shape({ radius: radius as Theme['radius'] })}>{radius[0]!.toUpperCase() + radius.slice(1)}</button>
          {/each}
        </div>
      {/snippet}
    </SettingRow>
    <SettingRow find="density" label="Density">
      {#snippet control()}
        <div class="segments" role="group" aria-label="Density">
          {#each ['compact', 'comfortable'] as density (density)}
            <button aria-pressed={active.density === density} onclick={() => shape({ density: density as Theme['density'] })}>{density[0]!.toUpperCase() + density.slice(1)}</button>
          {/each}
        </div>
      {/snippet}
    </SettingRow>
    <SettingRow find="contrast" label="Contrast">
      {#snippet control()}
        <input type="range" min="80" max="120" step="5" value={appearance.contrast ?? 100} oninput={e => themes.contrast(+e.currentTarget.value)} aria-label="Contrast" />
        <output>{appearance.contrast ?? 100}%</output>
      {/snippet}
    </SettingRow>
    <SettingRow find="avatar-shape" label="Avatar shape">
      {#snippet control()}
        <div class="segments" role="group" aria-label="Avatar shape">
          {#each [['theme', panel ? 'Match corners' : 'Theme'], ['sharp', 'Sharp'], ['soft', 'Soft'], ['round', 'Round']] as [value, name] (value)}
            <button aria-pressed={store.layout.avatarShape === value} onclick={() => store.saveLayout({ avatarShape: value as AvatarShape })}>{name}</button>
          {/each}
        </div>
      {/snippet}
    </SettingRow>
    <SettingRow find="presence" label="Online indicator">
      {#snippet control()}
        <div class="segments" role="group" aria-label="Online indicator">
          {#each [['dot', 'Dot'], ['ring', 'Ring'], ['off', 'Off']] as [value, name] (value)}
            <button aria-pressed={store.layout.presence === value} onclick={() => store.saveLayout({ presence: value as PresenceStyle })}>{name}</button>
          {/each}
        </div>
      {/snippet}
    </SettingRow>
  </div>
  <p class="list-foot">Corners and density are part of the theme. Your own themes save as you go; save a built-in one as your own to keep a change. Avatar shape and the online indicator are for this device only.</p>
  {#if themes.draft && !editor}
    <div class="slist unsaved">
      <SettingRow label="Unsaved theme changes" hint="Keep them as a theme of your own, or put the theme back.">
        {#snippet control()}
          <button class="btn lit" onclick={() => { editor = true; editingHalf = themes.half; naming = true; name = `${active.name} custom`.slice(0, 40) }}>Save as theme</button>
          <button class="btn quiet" onclick={() => themes.reset()}>Discard</button>
        {/snippet}
      </SettingRow>
    </div>
  {/if}

  <h3 class="list-head">Messages</h3>
  <div class="slist">
    <SettingRow find="room-icons" label="Room icons">
      {#snippet control()}
        <div class="segments" role="group" aria-label="Room icons">
          {#each [['icons', 'Icons'], ['hash', '# signs'], ['none', 'Nothing']] as const as [mode, label] (mode)}
            <button aria-pressed={(appearance.room_icons ?? 'icons') === mode} onclick={() => themes.roomIcons(mode)}>{label}</button>
          {/each}
        </div>
      {/snippet}
    </SettingRow>
    {#if !panel}
      <SettingRow find="bubble-shape" label="Bubble shape">
        {#snippet control()}
          <select class="field" aria-label="Bubble shape" value={appearance.bubble_shape ?? 'corners'} onchange={e => themes.bubbles({ bubble_shape: e.currentTarget.value as NonNullable<typeof appearance.bubble_shape> })}>
            {#each bubbleShapes as [shape, label] (shape)}<option value={shape}>{label}</option>{/each}
          </select>
        {/snippet}
      </SettingRow>
    {:else}
    <SettingRow find="bubble-shape" label="Bubble shape" hint="Match corners follows Corners above." wide>
      {#snippet control()}
        <div class="accents" role="group" aria-label="Bubble shape">
          {#each bubbleShapes as [shape, label] (shape)}
            <button class="pill" aria-pressed={(appearance.bubble_shape ?? 'corners') === shape} onclick={() => themes.bubbles({ bubble_shape: shape })}>
              <span class="shape {shape === 'corners' ? active.radius : shape}" aria-hidden="true"></span>{label}
            </button>
          {/each}
        </div>
      {/snippet}
    </SettingRow>
    {/if}
    <!-- Since own bubbles took the accent, the choice is where people's colours go: their names, your own bubble, or nowhere. -->
    <SettingRow find="bubble-colours" label="Colours" hint={bubbleHint}>
      {#snippet control()}
        <div class="segments" role="group" aria-label="Colours">
          {#each [['everyone', 'Names'], ['mine', 'My colour'], ['plain', 'Plain']] as const as [mode, label] (mode)}
            <button aria-pressed={(appearance.bubble_colors ?? 'everyone') === mode} onclick={() => themes.bubbles({ bubble_colors: mode })}>{label}</button>
          {/each}
        </div>
      {/snippet}
    </SettingRow>
    <!-- A phone's pinned preview already shows them. -->
    {#if panel}<div class="pad"><BubblePreview /></div>{/if}
  </div>
  <p class="list-foot">How messages look, on your screens only.</p>

  <BackgroundSettings />

  <h3 class="list-head">Fonts</h3>
  <div class="slist" data-find="fonts">
    {#each fontRoles as role (role)}
      <SettingRow label={role === 'display' ? 'Titles' : role === 'body' ? 'Text' : 'Code'}>
        {#snippet control()}
          <select class="field" aria-label={`${role} font`} value={active.fonts[role]} onchange={e => font(role, e.currentTarget.value)}>
            {#each [...new Set([...fontFamilies, active.fonts[role]])] as family (family)}
              <option style={`font-family:${fontStack(family, role)}`} value={family}>{family === 'System' ? (panel ? 'System (this device’s own)' : 'System') : family}</option>
            {/each}
          </select>
          <input class="field custom-font" class:desk-only={!panel} aria-label={`Use any Google Font for ${role}`} placeholder="Or any Google Font" maxlength="40" onchange={e => font(role, e.currentTarget.value)} />
        {/snippet}
      </SettingRow>
    {/each}
    <div class="specimen">
      <b style={`font-family:${fontStack(active.fonts.display, 'display')}`}>The den is open</b>
      <span style={`font-family:${fontStack(active.fonts.body, 'body')}`}>Evening, everyone. On in 20, back by 10. Pull up a chair.</span>
      <code style={`font-family:${fontStack(active.fonts.mono, 'mono')}`}>den send general "on my way"</code>
    </div>
  </div>
  <p class="list-foot">Part of the theme, like corners and density.</p>

  <p class="status muted" role="status">{themes.saving ? 'Saving…' : message}</p>
  {#if themes.error}<p class="error" role="alert">{themes.error}</p>{/if}
</div>

<style>
  .appearance { padding-bottom: 24px; }
  .first { margin-top: 20px; }
  .panel .first { margin-top: 16px; }
  .pad { padding: 12px 16px; }
  .circles { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 10px; }
  /* The chosen colour gets a ring with a gap, like iOS's colour wells. */
  .circle { width: 30px; height: 30px; flex: none; border-radius: 50%; background: var(--c); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--ink) 12%, transparent); }
  .circle { position: relative; cursor: pointer; }
  .circle.on { box-shadow: 0 0 0 2px var(--bg-2), 0 0 0 4px var(--c); }
  .circle.custom.on { box-shadow: 0 0 0 2px var(--bg-2), 0 0 0 4px var(--ink-3); }
  :global(:where(:root:not([data-input='touch']))) .circle:has(input:focus-visible) { outline: 2px solid var(--lamp); outline-offset: 4px; }
  .circle:active { transform: scale(.92); }
  @media (max-width: 650px) { .desk-only { display: none; } }
  .unsaved { margin-top: 12px; box-shadow: inset 0 0 0 1px var(--lamp); }
  .head-btn { color: var(--lamp); font: 600 var(--fs-sm)/1 var(--body); text-transform: none; letter-spacing: 0; padding: 4px; }
  .theme-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(196px, 1fr)); gap: 14px 12px; padding: 14px 14px 10px; }

  .accents { display: flex; flex-wrap: wrap; gap: 8px; }
  .accents .dot { width: 14px; height: 14px; border-radius: 50%; flex: none; box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--ink) 15%, transparent); }
  /* A tiny bubble in each shape. Match corners shows the shape it ends up as. */
  .shape { width: 18px; height: 12px; border: 1.5px solid currentColor; border-radius: 6px 6px 6px 2px; opacity: .8; }
  .shape.soft { border-radius: 4px 4px 4px 1px; }
  .shape.sharp { border-radius: 1px; }
  .shape.none { border: 0; border-radius: 0; height: 10px; background: linear-gradient(currentColor 0 0) 0 1px / 100% 1.5px no-repeat, linear-gradient(currentColor 0 0) 0 7px / 70% 1.5px no-repeat; }
  .pad :global(.preview) { margin: 0; }

  .editor-side { display: grid; gap: 10px; justify-items: start; padding: 14px 16px; }
  .editor-preview { width: min(260px, 100%); aspect-ratio: 8 / 5; border: 1px solid var(--line); border-radius: var(--r); overflow: hidden; }
  .swatch { width: 34px; height: 32px; padding: 2px; border: 1px solid var(--line); background: var(--bg); border-radius: var(--r); }
  .hex { width: 96px; min-width: 0; }
  .editor-actions { display: flex; flex-wrap: wrap; gap: 8px; padding: 12px 16px; }
  .save-name { display: flex; align-items: flex-end; flex-wrap: wrap; gap: 10px; padding: 0 16px 14px; }
  .save-name label { display: grid; gap: 6px; font-size: var(--fs-sm); }
  .link { color: var(--ink-2); font-size: var(--fs-sm); text-decoration: underline; text-underline-offset: 3px; }
  @media (hover: hover) { .link:hover { color: var(--ink); } }
  .hint { font-size: var(--fs-xs); margin: 0; }

  .specimen { display: grid; gap: 8px; padding: 16px; background: var(--bg); margin: 0 12px 12px; border-radius: var(--r); }
  .specimen b { font-size: 21px; font-weight: 600; }
  .specimen span { font-size: var(--fs-sm); color: var(--ink-2); }
  .specimen code { font-size: var(--fs-xs); color: var(--ink-3); background: none; border: 0; padding: 0; }

  select.field, .custom-font { width: auto; min-width: 150px; }
  input[type=range] { width: 150px; accent-color: var(--accent); }
  output { font-size: var(--fs-xs); color: var(--ink-2); min-width: 38px; text-align: end; font-variant-numeric: tabular-nums; }
  .status { font-size: var(--fs-xs); margin: 16px 4px 0; min-height: 18px; }
  .error { color: var(--danger); font-size: var(--fs-sm); }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }

  /* The panel over the app, and phones: two theme cards a row, fields full width. */
  @container panel (max-width: 460px) {
    .theme-grid { grid-template-columns: 1fr 1fr; gap: 12px 10px; padding: 12px; }
    select.field, .custom-font { width: 100%; min-width: 0; }
    input[type=range] { flex: 1; width: auto; min-width: 120px; }
  }
  @media (max-width: 650px) {
    .theme-grid { grid-template-columns: 1fr 1fr; gap: 12px 10px; padding: 12px; }
    select.field, .custom-font { width: 100%; min-width: 0; }
    input[type=range] { flex: 1; width: auto; min-width: 120px; }
  }
</style>
