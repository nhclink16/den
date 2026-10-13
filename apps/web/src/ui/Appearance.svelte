<script lang="ts">
  import { onDestroy } from 'svelte'
  import { fontStack, themeFor } from '../lib/theme-runtime'
  import { accentPresets, colorRoles, deriveHalf, resolveAccent } from '../lib/theme-color'
  import { themes, fontFamilies, validateTheme, validName, webFonts } from '../lib/theme.svelte'
  import type { Theme, ThemeFonts } from '../lib/types'
  import ThemePreview from './ThemePreview.svelte'
  import ThemeCard from './ThemeCard.svelte'
  import BackgroundSettings from './BackgroundSettings.svelte'
  import SettingRow from './SettingRow.svelte'
  import { store, type AvatarShape, type PresenceStyle } from '../lib/store.svelte'

  /** `panel`: shown in the live customizer rather than on the Settings page. */
  let { panel = false }: { panel?: boolean } = $props()
  type Half = 'light' | 'dark'

  let editor = $state(false)
  let editingHalf = $state<Half>('dark')
  let naming = $state(false)
  let name = $state('')
  let message = $state('')
  let fileInput: HTMLInputElement
  let picker: HTMLInputElement

  const active = $derived(themes.active)
  const appearance = $derived(themes.appearance)
  const lightTheme = $derived(themeFor(appearance, 'light'))
  const darkTheme = $derived(themeFor(appearance, 'dark'))
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
  const isCustom = (t: Theme) => appearance.custom_themes.some(c => c.id === t.id)

  const schemes = [
    { id: 'system', label: 'System', hint: 'Follows your device' },
    { id: 'light', label: 'Light', hint: 'Always light' },
    { id: 'dark', label: 'Dark', hint: 'Always dark' },
  ] as const

  function change(patch: Partial<Theme>) {
    message = ''
    themes.preview({ ...(JSON.parse(JSON.stringify(active)) as Theme), ...patch })
  }
  function color(role: (typeof colorRoles)[number], value: string) {
    if (!/^#[0-9a-f]{6}$/i.test(value)) return
    const { generated: _drop, ...colors } = active[editingHalf]
    change({ [editingHalf]: { ...colors, [role]: value } })
  }
  function font(role: keyof ThemeFonts, value: string) {
    if (validName(value)) change({ fonts: { ...active.fonts, [role]: value } })
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

  onDestroy(() => themes.reset())
</script>

<div class="appearance" class:panel>
  {#if !panel}
    <header class="intro">
      <h2 class="display">Appearance</h2>
      <p class="muted">Your theme follows you to every device signed in to this server.</p>
    </header>
  {/if}

  <section>
    <h3>Color scheme</h3>
    <div class="schemes">
      {#each schemes as s (s.id)}
        <button class="scheme" class:chosen={appearance.mode === s.id} aria-pressed={appearance.mode === s.id} onclick={() => themes.mode(s.id)}>
          <span class="scheme-art">
            {#if s.id === 'system'}
              <span class="split-light"><ThemePreview colors={shown(lightTheme, 'light')} fonts={lightTheme.fonts} /></span>
              <span class="split-dark"><ThemePreview colors={shown(darkTheme, 'dark')} fonts={darkTheme.fonts} /></span>
            {:else if s.id === 'light'}
              <ThemePreview colors={shown(lightTheme, 'light')} fonts={lightTheme.fonts} />
            {:else}
              <ThemePreview colors={shown(darkTheme, 'dark')} fonts={darkTheme.fonts} />
            {/if}
            <span class="check" aria-hidden="true"><svg viewBox="0 0 16 16" width="10" height="10" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.5l3 3 6-7" /></svg></span>
          </span>
          <span class="scheme-text"><b>{s.label}</b><small>{s.hint}</small></span>
        </button>
      {/each}
    </div>
  </section>

  <section>
    <div class="section-head">
      <div>
        <h3>Themes</h3>
        <p class="muted small">Pick one for light and another for dark. They don't have to match.</p>
      </div>
      <div class="head-actions">
        <button class="btn" onclick={() => openEditor(active)}>Create theme</button>
        <button class="btn" onclick={() => fileInput.click()}>Import</button>
        <input class="sr-only" tabindex="-1" bind:this={fileInput} type="file" accept=".json,application/json" aria-label="Import theme file" onchange={importTheme} />
      </div>
    </div>

    <div class="theme-grid">
      {#each themes.all as t (t.id)}
        <ThemeCard
          theme={t}
          lightChosen={appearance.light_theme === t.id}
          darkChosen={appearance.dark_theme === t.id}
          dimmed={appearance.mode === 'system' ? undefined : appearance.mode}
          accent={appearance.accent}
          onpick={half => { themes.select(t, half); message = '' }}
          oncustomise={() => openEditor(t)}
          onremove={isCustom(t) ? () => themes.remove(t.id) : undefined}
        />
      {/each}
    </div>
  </section>

  <section>
    <h3>Accent</h3>
    <p class="muted small">Buttons, links, unread counts and mentions, in every theme. Den keeps it readable in light and dark.</p>
    <div class="accents" role="group" aria-label="Accent">
      <button class="pill" aria-pressed={!appearance.accent} onclick={() => themes.accent(null)}><span class="dot" style:background={swatch(null)}></span>Theme's own</button>
      {#each accentPresets as p (p.id)}
        <button class="pill" aria-pressed={appearance.accent === p.id} onclick={() => themes.accent(p.id)}><span class="dot" style:background={swatch(p.id)}></span>{p.name}</button>
      {/each}
      <button class="pill" aria-pressed={!!custom} onclick={() => picker.click()}>
        <span class="dot" style:background={custom ? swatch(custom) : 'conic-gradient(#e5484d, #f5a524, #3fcf7a, #1d63d8, #8f8cf9, #e5484d)'}></span>Custom
      </button>
      <input type="color" class="sr-only" tabindex="-1" aria-label="Custom accent colour" bind:this={picker} value={custom ?? swatch(null)} onchange={e => themes.accent(e.currentTarget.value)} />
    </div>
  </section>

  <section>
    <h3>Room icons</h3>
    <p class="muted small">What sits before a room's name. Direct messages always show the person.</p>
    <div class="segments" role="group" aria-label="Room icons">
      {#each [['icons', 'Icons'], ['hash', '# signs'], ['none', 'Nothing']] as const as [mode, label] (mode)}
        <button aria-pressed={(appearance.room_icons ?? 'icons') === mode} onclick={() => themes.roomIcons(mode)}>{label}</button>
      {/each}
    </div>
  </section>

  {#if editor}
    <section class="editor" aria-label="Theme editor">
      <div class="section-head">
        <div>
          <h3>Editing {active.name}</h3>
          <p class="muted small">Every change shows in the app{panel ? ' behind this panel' : ''} as you make it.</p>
        </div>
        <button class="btn quiet" onclick={() => { editor = false; themes.reset() }}>Close</button>
      </div>

      <div class="editor-body">
        <div class="editor-side">
          <div class="segments" role="group" aria-label="Edit appearance">
            {#each ['light', 'dark'] as half (half)}
              <button aria-pressed={editingHalf === half} onclick={() => (editingHalf = half as Half)}>{half === 'light' ? 'Light' : 'Dark'}</button>
            {/each}
          </div>
          <div class="editor-preview"><ThemePreview colors={shown(active, editingHalf)} fonts={active.fonts} /></div>
          {#if active[editingHalf].generated}
            <p class="hint muted">Generated from the {editingHalf === 'light' ? 'dark' : 'light'} half. Adjust to taste.</p>
          {/if}
          <button class="link" onclick={() => change({ [editingHalf]: deriveHalf(active[editingHalf === 'light' ? 'dark' : 'light']) })}>
            Copy from {editingHalf === 'light' ? 'dark' : 'light'}
          </button>
        </div>

        <div class="roles">
          {#each colorRoles as role (role)}
            <SettingRow label={roleLabels[role][0]} hint={roleLabels[role][1]}>
              {#snippet control()}
                <input type="color" aria-label={`${roleLabels[role][0]} color`} value={active[editingHalf][role]} oninput={e => color(role, e.currentTarget.value)} />
                <input class="field mono" aria-label={`${roleLabels[role][0]} hex`} value={active[editingHalf][role]} maxlength="7" pattern="#[0-9a-fA-F]{'{'}6{'}'}" oninput={e => color(role, e.currentTarget.value)} />
              {/snippet}
            </SettingRow>
          {/each}
        </div>
      </div>

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

  <BackgroundSettings />

  <section>
    <h3>Typography</h3>
    <div class="group">
      {#each fontRoles as role (role)}
        <SettingRow label={role === 'display' ? 'Display' : role === 'body' ? 'Body' : 'Monospace'} hint={role === 'display' ? 'Big titles.' : role === 'body' ? 'Messages, times, labels and everything else.' : 'Code and terminals.'}>
          {#snippet control()}
            <select class="field" aria-label={`${role} font`} value={active.fonts[role]} onchange={e => font(role, e.currentTarget.value)}>
              {#each [...new Set([...fontFamilies, active.fonts[role]])] as family (family)}
                <option style={`font-family:${fontStack(family, role)}`} value={family}>{family === 'System' ? 'System (this device’s own)' : family}</option>
              {/each}
            </select>
            <input class="field custom-font" aria-label={`Use any Google Font for ${role}`} placeholder="Or any Google Font" maxlength="40" onchange={e => font(role, e.currentTarget.value)} />
          {/snippet}
        </SettingRow>
      {/each}
      <div class="specimen">
        <b style={`font-family:${fontStack(active.fonts.display, 'display')}`}>The den is open</b>
        <span style={`font-family:${fontStack(active.fonts.body, 'body')}`}>Evening, everyone. On in 20, back by 10. Pull up a chair.</span>
        <code style={`font-family:${fontStack(active.fonts.mono, 'mono')}`}>den send general "on my way"</code>
      </div>
    </div>
  </section>

  <section>
    <h3>Interface</h3>
    <div class="group">
      <SettingRow label="Corners" hint="How rounded panels, cards and buttons are.">
        {#snippet control()}
          <div class="segments" role="group" aria-label="Corner radius">
            {#each ['sharp', 'soft', 'round'] as radius (radius)}
              <button aria-pressed={active.radius === radius} onclick={() => change({ radius: radius as Theme['radius'] })}>{radius[0]!.toUpperCase() + radius.slice(1)}</button>
            {/each}
          </div>
        {/snippet}
      </SettingRow>
      <SettingRow label="Density" hint="Comfortable gives messages more room to breathe.">
        {#snippet control()}
          <div class="segments" role="group" aria-label="Density">
            {#each ['compact', 'comfortable'] as density (density)}
              <button aria-pressed={active.density === density} onclick={() => change({ density: density as Theme['density'] })}>{density[0]!.toUpperCase() + density.slice(1)}</button>
            {/each}
          </div>
        {/snippet}
      </SettingRow>
      <SettingRow label="Avatar shape" hint="How profile pictures are cut, on this device only.">
        {#snippet control()}
          <div class="segments" role="group" aria-label="Avatar shape">
            {#each [['theme', 'Match corners'], ['sharp', 'Sharp'], ['soft', 'Soft'], ['round', 'Round']] as [value, name] (value)}
              <button aria-pressed={store.layout.avatarShape === value} onclick={() => store.saveLayout({ avatarShape: value as AvatarShape })}>{name}</button>
            {/each}
          </div>
        {/snippet}
      </SettingRow>
      <SettingRow label="Online indicator" hint="How people who are here show on their picture, on this device only.">
        {#snippet control()}
          <div class="segments" role="group" aria-label="Online indicator">
            {#each [['dot', 'Dot'], ['ring', 'Ring'], ['off', 'Off']] as [value, name] (value)}
              <button aria-pressed={store.layout.presence === value} onclick={() => store.saveLayout({ presence: value as PresenceStyle })}>{name}</button>
            {/each}
          </div>
        {/snippet}
      </SettingRow>
      <SettingRow label="Contrast" hint="Raises or softens secondary text and borders.">
        {#snippet control()}
          <input type="range" min="80" max="120" step="5" value={appearance.contrast ?? 100} oninput={e => themes.contrast(+e.currentTarget.value)} aria-label="Contrast" />
          <output>{appearance.contrast ?? 100}%</output>
        {/snippet}
      </SettingRow>
    </div>
  </section>

  <p class="status muted" role="status">{themes.saving ? 'Saving…' : message}</p>
  {#if themes.error}<p class="error" role="alert">{themes.error}</p>{/if}
</div>

<style>
  .appearance { padding-bottom: 48px; }
  .intro p { margin: 6px 0 0; }
  section { margin-top: 34px; }
  h3 { margin: 0 0 3px; }
  .small { font-size: 13px; margin: 0; }
  .section-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 14px; flex-wrap: wrap; }
  .head-actions { display: flex; gap: 8px; }

  .schemes { display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); gap: 12px; margin-top: 12px; }
  .scheme { display: grid; gap: 10px; padding: 10px; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg2); text-align: start; }
  .scheme:hover { border-color: var(--ink3); }
  .scheme.chosen { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }
  /* The same chosen marker the theme cards use. */
  .scheme-art > .check { display: none; position: absolute; inset: auto 4px 4px auto; z-index: 3; width: 17px; height: 17px; border-radius: 50%; place-items: center; background: var(--accent); color: var(--on-lamp); }
  .scheme.chosen .scheme-art > .check { display: grid; }
  .accents { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }
  .accents .dot { width: 14px; height: 14px; border-radius: 50%; flex: none; box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--ink) 15%, transparent); }

  .scheme-art { position: relative; display: block; aspect-ratio: 16 / 10; border-radius: var(--r); overflow: hidden; border: 1px solid var(--line); }
  .scheme-art > :global(*) { position: absolute; inset: 0; }
  /* A vertical seam reads better than a diagonal one: each half still shows a
     sidebar, a message and a composer instead of an empty wedge. */
  .split-light { clip-path: inset(0 50% 0 0); }
  .split-dark { clip-path: inset(0 0 0 50%); }
  .split-dark::before { content: ''; position: absolute; inset: 0 0 0 50%; z-index: 2; border-inline-start: 1px solid var(--line); }
  .scheme-text { display: grid; gap: 1px; padding: 0 2px 2px; }
  .scheme-text b { font-size: 14px; }
  .scheme-text small { font-size: 12px; color: var(--ink2); }

  .theme-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(232px, 1fr)); gap: 14px; }

  .editor { border: 1px solid var(--accent); border-radius: var(--r-lg); background: var(--bg2); padding: 18px; }
  .editor-body { display: grid; grid-template-columns: 210px minmax(0, 1fr); gap: 24px; align-items: start; }
  .editor-side { display: grid; gap: 10px; }
  .editor-preview { aspect-ratio: 8 / 5; border: 1px solid var(--line); border-radius: var(--r); overflow: hidden; }
  .roles { border: 1px solid var(--line); border-radius: var(--r); overflow: hidden; }
  .roles :global(.field) { width: 100px; min-width: 0; }
  .roles input[type=color] { width: 34px; height: 32px; padding: 2px; border: 1px solid var(--line); background: var(--bg); border-radius: var(--r); }
  .editor-actions { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 18px; }
  .save-name { display: flex; align-items: flex-end; flex-wrap: wrap; gap: 10px; margin-top: 14px; }
  .save-name label { display: grid; gap: 6px; font-size: 13px; }
  .link { color: var(--ink2); font-size: 12.5px; text-decoration: underline; text-underline-offset: 3px; justify-self: start; }
  .link:hover { color: var(--ink); }
  .hint { font-size: 12px; margin: 0; }

  .group { border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg2); margin-top: 12px; }
  .specimen { display: grid; gap: 8px; padding: 16px; border-top: 1px solid var(--line); background: var(--bg); border-radius: 0 0 var(--r-lg) var(--r-lg); }
  .specimen b { font-size: 21px; font-weight: 600; }
  .specimen span { font-size: 14px; color: var(--ink2); }
  .specimen code { font-size: 12.5px; color: var(--ink3); }

  select.field, .custom-font { width: auto; min-width: 150px; }
  input[type=range] { width: 160px; accent-color: var(--accent); }
  output { font-size: 12px; color: var(--ink2); min-width: 42px; text-align: end; }
  .status { font-size: 12.5px; margin-top: 22px; min-height: 18px; }
  .error { color: var(--danger); font-size: 13px; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }

  .appearance.panel { padding-bottom: 16px; }
  .appearance.panel > section:first-child { margin-top: 16px; }
  .appearance.panel section { margin-top: 26px; }

  @media (max-width: 900px) { .editor-body { grid-template-columns: 1fr; } .editor-side { max-width: 260px; } }
  /* The narrow arrangement, when shown in the customizer panel. */
  @container panel (max-width: 460px) {
    .editor-body { grid-template-columns: 1fr; } .editor-side { max-width: 260px; }
    .editor { padding: 14px; }
    .schemes { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
    .scheme { padding: 6px; gap: 6px; min-width: 0; }
    .scheme-art { aspect-ratio: 16 / 10; }
    .scheme-text b { font-size: 13px; }
    .scheme-text small { display: none; }
    .theme-grid { grid-template-columns: 1fr 1fr; gap: 10px; }
    select.field, .custom-font { width: 100%; min-width: 0; }
    input[type=range] { width: 100%; }
  }
  @media (max-width: 650px) {
    .schemes { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; }
    .scheme { padding: 6px; gap: 6px; min-width: 0; }
    .scheme-art { aspect-ratio: 16 / 9; max-height: 90px; }
    .scheme-text small { font-size: 11px; }
    .theme-grid { grid-template-columns: 1fr 1fr; gap: 10px; }
    select.field, .custom-font { width: 100%; min-width: 0; }
    input[type=range] { width: 100%; }
    .field { font-size: 16px; }
  }
</style>
