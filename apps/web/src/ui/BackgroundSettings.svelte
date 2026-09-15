<script lang="ts">
  import { themes } from '../lib/theme.svelte'
  import { builtinBackgrounds, builtinBackgroundImage, bumpBackground, backgroundImageUrl } from '../lib/theme-runtime'
  import { apiFor } from '../lib/api'
  import { activeOrigin } from '../lib/native'
  import type { AppearanceBackground } from '../lib/types'
  import SettingRow from './SettingRow.svelte'

  let fileInput: HTMLInputElement
  let busy = $state(false)
  let error = $state('')
  let uploadedAt = $state(0)

  const bg = $derived(themes.appearance.background ?? null)
  const own = $derived(bg?.source.type === 'upload')
  const colors = $derived(themes.active[themes.half])
  const defaults: Omit<AppearanceBackground, 'source'> = { blur: 0, dim: 30, saturate: 100, scope: 'app', fit: 'cover' }
  const labels: Record<string, string> = {
    aurora: 'Aurora', dunes: 'Dunes', harbor: 'Harbor',
    'ember-sky': 'Ember sky', 'slate-mist': 'Slate mist', grain: 'Grain',
  }

  function set(patch: Partial<AppearanceBackground>) {
    if (!bg) return
    themes.background({ ...bg, ...patch })
  }
  function pickBuiltin(name: string) {
    themes.background({ ...(bg ?? defaults), source: { type: 'builtin', name } } as AppearanceBackground)
  }
  function clear() { themes.background(null) }

  async function upload(e: Event) {
    const file = (e.target as HTMLInputElement).files?.[0]
    fileInput.value = ''
    if (!file) return
    error = ''
    if (file.size > 8 * 1024 * 1024) { error = 'Images must be at most 8 MB.'; return }
    busy = true
    try {
      await apiFor(activeOrigin()).putRaw('/users/me/background/image', file, { 'content-type': file.type || 'application/octet-stream' })
      bumpBackground()
      uploadedAt = Date.now()
      themes.background({ ...(bg ?? defaults), source: { type: 'upload', id: String(uploadedAt) } } as AppearanceBackground)
    } catch (err) {
      error = (err as Error).message || "That image couldn't be saved."
    } finally { busy = false }
  }
</script>

<section class="bgsec">
  <div class="head">
    <h3>Background</h3>
    <p class="muted">A wallpaper behind the room. Presets are painted from your theme, so they follow it.</p>
  </div>

  <div class="swatches">
    <button class="swatch none" class:chosen={!bg} onclick={clear} aria-pressed={!bg}>
      <span class="x" aria-hidden="true">
        <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </span>
      <span class="label">None</span>
    </button>
    {#each builtinBackgrounds as name (name)}
      {@const active = bg?.source.type === 'builtin' && bg.source.name === name}
      <button class="swatch" class:chosen={active} aria-pressed={active} onclick={() => pickBuiltin(name)}>
        <span class="chip" style={`background-image:${builtinBackgroundImage(name, colors)}`}></span>
        <span class="label">{labels[name] ?? name}</span>
      </button>
    {/each}
    <button class="swatch" class:chosen={own} aria-pressed={own} onclick={() => fileInput.click()} disabled={busy}>
      <span class="chip own" style={own ? `background-image:url("${backgroundImageUrl()}")` : ''}>
        {#if !own}
          <svg viewBox="0 0 16 16" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M8 3.2v9.6M3.2 8h9.6" /></svg>
        {/if}
      </span>
      <span class="label">{busy ? 'Uploading…' : own ? 'Your image' : 'Your image'}</span>
    </button>
    <input class="sr-only" tabindex="-1" bind:this={fileInput} type="file" accept="image/png,image/jpeg,image/webp,image/gif" aria-label="Choose a background image" onchange={upload} />
  </div>
  <p class="note faint">Up to 8 MB. Animated GIFs work, but a still image is easier to read over.</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}

  {#if bg}
    <div class="group">
      <SettingRow label="Blur" hint="Softens the image so text stays legible.">
        {#snippet control()}
          <input type="range" min="0" max="40" value={bg.blur} oninput={e => set({ blur: +e.currentTarget.value })} aria-label="Background blur" />
          <output class="mono">{bg.blur}px</output>
        {/snippet}
      </SettingRow>
      <SettingRow label="Dim" hint="Darkens or lightens the image behind your text.">
        {#snippet control()}
          <input type="range" min="0" max="80" value={bg.dim} oninput={e => set({ dim: +e.currentTarget.value })} aria-label="Background dim" />
          <output class="mono">{bg.dim}%</output>
        {/snippet}
      </SettingRow>
      <SettingRow label="Saturation">
        {#snippet control()}
          <input type="range" min="50" max="150" value={bg.saturate} oninput={e => set({ saturate: +e.currentTarget.value })} aria-label="Background saturation" />
          <output class="mono">{bg.saturate}%</output>
        {/snippet}
      </SettingRow>
      <SettingRow label="Where it shows">
        {#snippet control()}
          <div class="segments" role="group" aria-label="Background area">
            {#each [['app','Everywhere'],['sidebar','Sidebar'],['chat','Conversation']] as [value, text] (value)}
              <button aria-pressed={bg.scope === value} onclick={() => set({ scope: value as AppearanceBackground['scope'] })}>{text}</button>
            {/each}
          </div>
        {/snippet}
      </SettingRow>
      {#if bg.source.type === 'upload'}
        <SettingRow label="Fit">
          {#snippet control()}
            <div class="segments" role="group" aria-label="Background fit">
              {#each [['cover','Fill'],['contain','Fit'],['tile','Tile']] as [value, text] (value)}
                <button aria-pressed={bg.fit === value} onclick={() => set({ fit: value as AppearanceBackground['fit'] })}>{text}</button>
              {/each}
            </div>
          {/snippet}
        </SettingRow>
      {/if}
    </div>
  {/if}
</section>

<style>
  .bgsec { margin-top: 34px; }
  .head h3 { font-size: 15px; font-weight: 700; margin: 0 0 3px; }
  .head p { margin: 0 0 14px; font-size: 13px; }
  .swatches { display: grid; grid-template-columns: repeat(auto-fill, minmax(104px, 1fr)); gap: 10px; }
  .swatch { display: grid; gap: 6px; justify-items: stretch; text-align: start; }
  .chip, .none .x {
    display: grid; place-items: center; height: 58px; border-radius: var(--r);
    border: 1px solid var(--line); background-size: cover; background-position: center; color: var(--ink3);
  }
  .none .x { background: var(--bg2); }
  .chip.own { background-color: var(--bg2); }
  .swatch:hover .chip, .swatch:hover .x { border-color: var(--ink3); }
  .swatch.chosen .chip, .swatch.chosen .x { box-shadow: 0 0 0 2px var(--accent); border-color: transparent; }
  .swatch .label { font-size: 12.5px; color: var(--ink2); }
  .swatch.chosen .label { color: var(--ink); font-weight: 600; }
  .note { font-size: 12px; margin: 10px 0 0; }
  .error { color: var(--danger); font-size: 13px; margin: 8px 0 0; }
  .group { margin-top: 18px; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg2); }
  input[type=range] { width: 170px; accent-color: var(--accent); }
  output { font-size: 12px; color: var(--ink2); min-width: 44px; text-align: end; }
  .segments { display: inline-flex; padding: 3px; gap: 3px; background: var(--bg); border: 1px solid var(--line); border-radius: var(--r); }
  .segments button { min-height: 32px; padding: 5px 12px; border-radius: var(--r); color: var(--ink2); font-size: 13px; }
  .segments button[aria-pressed=true] { background: var(--bg3); color: var(--ink); font-weight: 600; }
  .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  @media (max-width: 720px) { input[type=range] { width: 100%; } }
</style>
