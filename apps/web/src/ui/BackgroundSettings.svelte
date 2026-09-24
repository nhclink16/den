<script lang="ts">
  import { onMount } from 'svelte'
  import { themes } from '../lib/theme.svelte'
  import { uploadedBackgroundUrl } from '../lib/theme-runtime'
  import { wallpapers, wallpaperNames, wallpaperImage, legacyWallpapers, savedWallpaper, type Wallpaper } from '../lib/wallpapers'
  import { apiFor } from '../lib/api'
  import { activeOrigin } from '../lib/native'
  import type { AppearanceBackground, BackgroundBuiltin, BackgroundImage } from '../lib/types'
  import SettingRow from './SettingRow.svelte'

  let fileInput: HTMLInputElement
  let busy = $state(false)
  let error = $state('')
  let uploads = $state<BackgroundImage[]>([])
  let confirming = $state<string | null>(null)

  // Which wallpaper the gallery edits: the main one, or the sidebar's own.
  let target = $state<'main' | 'sidebar'>('main')
  const bg = $derived((target === 'main' ? themes.appearance.background : themes.appearance.sidebar_background) ?? null)
  const sidebarOwn = $derived(!!themes.appearance.sidebar_background)
  const put = (b: AppearanceBackground | null) => target === 'main' ? themes.background(b) : themes.sidebarBackground(b)
  const colors = $derived(themes.active[themes.half])
  const chosenPreset = $derived(bg?.source.type === 'builtin' ? (legacyWallpapers[bg.source.name] ?? bg.source.name) : null)
  const chosenUpload = $derived(bg?.source.type === 'upload' ? bg.source.id : null)
  const defaults: Omit<AppearanceBackground, 'source'> = { blur: 0, dim: 30, saturate: 100, scope: 'app', fit: 'cover' }
  const api = () => apiFor(activeOrigin())

  async function load() { uploads = await api().get<BackgroundImage[]>('/users/me/backgrounds').catch(() => uploads) }
  onMount(load)

  function set(patch: Partial<AppearanceBackground>) {
    if (!bg) return
    put({ ...bg, ...patch })
  }
  function pickPreset(name: Wallpaper) { put({ ...(bg ?? defaults), source: { type: 'builtin', name: savedWallpaper[name] as BackgroundBuiltin } }) }
  function pickUpload(id: string) { put({ ...(bg ?? defaults), source: { type: 'upload', id } }) }
  function clear() { put(null) }

  const MAX_EDGE = 3840, KEEP_BYTES = 6 * 1024 * 1024, SERVER_BYTES = 16 * 1024 * 1024
  /**
   * Phones and displays make pictures far bigger than a wallpaper needs. Anything
   * the browser can open (HEIC too, where it can) is scaled to at most 4K on its
   * long edge and saved as a JPEG. GIFs go up untouched so they keep moving.
   */
  async function prepare(file: File): Promise<Blob> {
    if (file.type === 'image/gif') {
      if (file.size > SERVER_BYTES) throw Error('Animated GIFs can be up to 16 MB. Try a shorter or smaller one.')
      return file
    }
    const bitmap = await createImageBitmap(file).catch(() => null)
    if (!bitmap) throw Error("This browser can't open that file. Try a PNG, JPEG or WebP.")
    const scale = Math.min(1, MAX_EDGE / Math.max(bitmap.width, bitmap.height))
    if (scale === 1 && file.size <= KEEP_BYTES && ['image/png', 'image/jpeg', 'image/webp'].includes(file.type)) { bitmap.close(); return file }
    const canvas = document.createElement('canvas')
    canvas.width = Math.round(bitmap.width * scale); canvas.height = Math.round(bitmap.height * scale)
    const ctx = canvas.getContext('2d')!
    // JPEG has no transparency; sit see-through pixels on the page colour they would show over.
    ctx.fillStyle = colors.bg; ctx.fillRect(0, 0, canvas.width, canvas.height)
    ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height)
    bitmap.close()
    const blob = await new Promise<Blob | null>((ok) => canvas.toBlob(ok, 'image/jpeg', 0.9))
    if (!blob) throw Error("That image couldn't be prepared. Try a different file.")
    return blob
  }

  async function upload(e: Event) {
    const file = (e.target as HTMLInputElement).files?.[0]
    fileInput.value = ''
    if (!file) return
    error = ''; busy = true
    try {
      const image = await prepare(file)
      const saved = await api().putRaw<BackgroundImage>('/users/me/background/image', image, { 'content-type': image.type })
      await load()
      pickUpload(saved.id)
    } catch (err) {
      error = (err as Error).message || "That image couldn't be saved."
    } finally { busy = false }
  }

  async function remove(id: string) {
    if (confirming !== id) { confirming = id; return }
    confirming = null; error = ''
    try {
      await api().del(`/users/me/backgrounds/${id}`)
      uploads = uploads.filter((u) => u.id !== id)
      if (chosenUpload === id) clear()
    } catch (err) { error = (err as Error).message || "That picture couldn't be removed." }
  }
</script>
<section class="bgsec">
  <div class="head">
    <h3>Background</h3>
    <p class="muted">A wallpaper behind the room. Presets are painted from your theme, so they change with it.</p>
  </div>

  <div class="target">
    <div class="segments" role="group" aria-label="Which wallpaper to edit">
      <button aria-pressed={target === 'main'} onclick={() => (target = 'main')}>Main</button>
      <button aria-pressed={target === 'sidebar'} onclick={() => (target = 'sidebar')}>Sidebar{#if sidebarOwn}<span class="dot" aria-hidden="true"></span><span class="sr-only"> (has its own)</span>{/if}</button>
    </div>
    <p class="faint">{target === 'main' ? (sidebarOwn ? 'Behind the conversation. The sidebar has its own.' : 'Behind the room. Pick one for the sidebar too if you want two.') : 'Just the sidebar. None uses the main wallpaper there.'}</p>
  </div>

  <h4 class="eyebrow">Presets</h4>
  <div class="swatches">
    <button class="swatch none" class:chosen={!bg} onclick={clear} aria-pressed={!bg}>
      <span class="x" aria-hidden="true">
        <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M4 4l8 8M12 4l-8 8" /></svg>
      </span>
      <span class="label">None</span>
    </button>
    {#each wallpapers as name (name)}
      {@const active = chosenPreset === name}
      <button class="swatch" class:chosen={active} aria-pressed={active} onclick={() => pickPreset(name)}>
        <span class="chip" style={`background-image:${wallpaperImage(name, colors)}`}></span>
        <span class="label">{wallpaperNames[name]}</span>
      </button>
    {/each}
  </div>

  <h4 class="eyebrow">Your pictures</h4>
  <div class="swatches">
    <button class="swatch add" onclick={() => fileInput.click()} disabled={busy}>
      <span class="chip">
        {#if busy}<span class="spin" aria-hidden="true"></span>{:else}<svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"><path d="M8 3.2v9.6M3.2 8h9.6" /></svg>{/if}
      </span>
      <span class="label">{busy ? 'Uploading…' : 'Upload'}</span>
    </button>
    {#each uploads as image (image.id)}
      {@const active = chosenUpload === image.id}
      <div class="swatch mine" class:chosen={active}>
        <button class="pick" aria-pressed={active} aria-label={`Use picture from ${new Date((image.uploaded_at ?? 0) * 1000).toLocaleDateString()}`} onclick={() => pickUpload(image.id)}>
          <span class="chip" style={`background-image:url("${uploadedBackgroundUrl(image.id, true)}")`}></span>
          <span class="label">{image.width}×{image.height}</span>
        </button>
        <button class="remove" class:confirm={confirming === image.id} onclick={() => remove(image.id)} onblur={() => { if (confirming === image.id) confirming = null }} aria-label={confirming === image.id ? 'Confirm removing this picture' : 'Remove this picture'}>{confirming === image.id ? 'Remove?' : '×'}</button>
      </div>
    {/each}
    <input class="sr-only" tabindex="-1" bind:this={fileInput} type="file" accept="image/*" aria-label="Choose a background picture" onchange={upload} />
  </div>
  <p class="note faint">Big photos are shrunk to 4K before they upload. Your last 24 pictures stay here. GIFs keep their animation.</p>
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
      {#if target === 'main'}
      <SettingRow label="Where it shows">
        {#snippet control()}
          <div class="segments" role="group" aria-label="Background area">
            {#each sidebarOwn ? [['app','Everywhere else'],['chat','Conversation']] : [['app','Everywhere'],['sidebar','Sidebar'],['chat','Conversation']] as [value, text] (value)}
              <button aria-pressed={bg.scope === value} onclick={() => set({ scope: value as AppearanceBackground['scope'] })}>{text}</button>
            {/each}
          </div>
        {/snippet}
      </SettingRow>
      {/if}
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
  .target { display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px; margin: 4px 0 16px; }
  .target p { margin: 0; font-size: 13px; }
  .target .dot { display: inline-block; width: 6px; height: 6px; margin-left: 6px; border-radius: 50%; background: var(--lamp); vertical-align: middle; }
  .bgsec { margin-top: 34px; }
  .head h3 { margin: 0 0 3px; }
  .head p { margin: 0 0 14px; font-size: 13px; }
  .swatches { display: grid; grid-template-columns: repeat(auto-fill, minmax(104px, 1fr)); gap: 10px; }
  .swatch { display: grid; gap: 6px; justify-items: stretch; text-align: start; }
  .chip, .none .x {
    display: grid; place-items: center; height: 58px; border-radius: var(--r);
    border: 1px solid var(--line); background-size: cover; background-position: center; color: var(--ink3);
  }
  .none .x { background: var(--bg2); }
  .swatch:hover .chip, .swatch:hover .x { border-color: var(--ink3); }
  .swatch.chosen .chip, .swatch.chosen .x { box-shadow: 0 0 0 2px var(--accent); border-color: transparent; }
  .swatch .label { font-size: 12px; color: var(--ink2); }
  .eyebrow { margin: 18px 0 8px; }
  .add .chip { border-style: dashed; background: var(--bg2); color: var(--ink2); }
  .add:disabled { cursor: progress; }
  .mine { position: relative; }
  .mine .pick { display: grid; gap: 6px; text-align: start; }
  .mine .chip { background-color: var(--bg3); }
  .mine:hover .chip { border-color: var(--ink3); }
  .mine.chosen .chip { box-shadow: 0 0 0 2px var(--accent); border-color: transparent; }
  .remove {
    position: absolute; top: 4px; right: 4px; min-width: 22px; height: 22px; padding: 0 6px; border-radius: var(--r-pill, 999px);
    background: color-mix(in srgb, #000 55%, transparent); color: #fff; font-size: 13px; line-height: 22px;
    opacity: 0; transition: opacity var(--t-fast, .1s);
  }
  .mine:hover .remove, .mine:focus-within .remove, .remove.confirm { opacity: 1; }
  .remove.confirm { background: var(--danger); font-size: 12px; font-weight: 600; }
  @media (pointer: coarse) { .remove { opacity: 1; } }
  .spin { width: 16px; height: 16px; border-radius: 50%; border: 2px solid var(--line); border-top-color: var(--accent); animation: spin .8s linear infinite; }
  @keyframes spin { to { transform: rotate(1turn); } }
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
