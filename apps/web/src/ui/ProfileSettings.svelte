<script lang="ts">
  // Your profile, edited beside the card everyone else sees. Text fields save
  // together; pictures upload as soon as they are chosen, because the server
  // needs the bytes to answer with the new URL anyway.
  import { untrack as untracked } from 'svelte'
  import { store } from '../lib/store.svelte'
  import type { User, UserStatus } from '../lib/types'
  import UserCard from './UserCard.svelte'
  import SettingRow from './SettingRow.svelte'
  import Icon from './Icon.svelte'
  import { native } from '../lib/native'
  import { router } from '../lib/router.svelte'
  import { activityShare } from '../lib/activity-share.svelte'
  import { activityVerb } from '../lib/activity'

  const me = $derived(store.me!)
  let displayName = $state(''), bio = $state(''), accent = $state<string | null>(null)
  let emoji = $state(''), statusText = $state(''), clearAfter = $state('never')
  let saving = $state(false), message = $state(''), error = $state('')
  let busyImage = $state<'avatar' | 'banner' | null>(null)

  function reset() {
    displayName = me.display_name; bio = me.bio ?? ''; accent = me.accent ?? null
    emoji = me.status?.emoji ?? ''; statusText = me.status?.text ?? ''
    // A status that already clears later keeps that time unless you pick another.
    clearAfter = me.status?.expires_at ? 'keep' : 'never'
    error = ''
  }
  // Load once, and again only if the saved profile changes underneath an untouched
  // form: an edit from another device, or a status timing out, must not wipe typing.
  let loadedFor = ''
  $effect(() => {
    const key = JSON.stringify([me.display_name, me.bio, me.accent, me.status])
    if (key === loadedFor) return
    if (!loadedFor || !untracked(() => dirty)) reset()
    loadedFor = key
  })
  const keepUntil = $derived(me.status?.expires_at ? new Date(me.status.expires_at * 1000).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' }) : '')

  const graphemes = (s: string) => [...new Intl.Segmenter().segment(s)].length
  const BIO_MAX = 190, STATUS_MAX = 60
  const expiry = (): number | null => {
    const now = Math.floor(Date.now() / 1000)
    if (clearAfter === '30m') return now + 1800
    if (clearAfter === '1h') return now + 3600
    if (clearAfter === '4h') return now + 4 * 3600
    if (clearAfter === 'keep') return me.status?.expires_at ?? null
    if (clearAfter === 'today') { const d = new Date(); d.setHours(23, 59, 59, 0); return Math.floor(d.getTime() / 1000) }
    return null
  }
  const status = $derived<UserStatus | null>(emoji.trim() || statusText.trim() ? { emoji: emoji.trim() || null, text: statusText.trim() || null, expires_at: null } : null)
  const draft = $derived<Partial<User>>({ display_name: displayName.trim() || me.username, bio: bio.trim() || null, accent, status })
  const dirty = $derived(
    displayName !== me.display_name || (bio.trim() || null) !== (me.bio ?? null) || accent !== (me.accent ?? null)
    || (emoji.trim() || null) !== (me.status?.emoji ?? null) || (statusText.trim() || null) !== (me.status?.text ?? null) || clearAfter !== (me.status?.expires_at ? 'keep' : 'never'),
  )
  const problem = $derived(
    !displayName.trim() ? 'Your name can’t be empty.'
    : [...bio].length > BIO_MAX ? `Keep your bio under ${BIO_MAX} characters.`
    : [...statusText].length > STATUS_MAX ? `Keep your status under ${STATUS_MAX} characters.`
    : emoji.trim() && graphemes(emoji.trim()) !== 1 ? 'Use a single emoji for your status.'
    : '',
  )

  async function save(e: SubmitEvent) {
    e.preventDefault()
    if (problem || saving) return
    saving = true; error = ''; message = ''
    try {
      await store.saveProfile({
        display_name: displayName.trim(), bio: bio.trim() || null, accent,
        status: status ? { ...status, expires_at: expiry() } : null,
      })
      message = 'Saved. Everyone sees the new version now.'
    } catch (err) { error = err instanceof Error ? err.message : 'Could not save your profile.' }
    finally { saving = false }
  }

  // --- pictures ---------------------------------------------------------------
  // The server wants a square avatar. A square GIF goes up untouched so it keeps
  // its animation; anything else is framed here and sent as a PNG.
  let crop = $state<{ url: string; w: number; h: number; zoom: number; x: number; y: number } | null>(null)
  const FRAME = 240
  let drag: { x: number; y: number; ox: number; oy: number } | null = null
  const scale = $derived(crop ? (FRAME / Math.min(crop.w, crop.h)) * crop.zoom : 1)
  function clampCrop() {
    if (!crop) return
    const w = crop.w * scale, h = crop.h * scale
    crop.x = Math.min(0, Math.max(FRAME - w, crop.x)); crop.y = Math.min(0, Math.max(FRAME - h, crop.y))
  }
  async function choose(kind: 'avatar' | 'banner', file: File | undefined) {
    if (!file) return
    error = ''; message = ''
    const limit = kind === 'avatar' ? 4 : 8
    if (!/^image\/(png|jpeg|webp|gif)$/.test(file.type)) { error = 'Use a PNG, JPEG, WebP or GIF.'; return }
    const url = URL.createObjectURL(file)
    const img = await load(url).catch(() => null)
    if (!img) { URL.revokeObjectURL(url); error = 'That image could not be opened.'; return }
    if (kind === 'banner' || (file.type === 'image/gif' && img.naturalWidth === img.naturalHeight)) {
      URL.revokeObjectURL(url)
      if (file.size > limit * 1024 * 1024) { error = `Pictures can be up to ${limit} MB.`; return }
      return upload(kind, file)
    }
    const s = FRAME / Math.min(img.naturalWidth, img.naturalHeight)
    crop = { url, w: img.naturalWidth, h: img.naturalHeight, zoom: 1, x: (FRAME - img.naturalWidth * s) / 2, y: (FRAME - img.naturalHeight * s) / 2 }
  }
  const load = (url: string) => new Promise<HTMLImageElement>((ok, fail) => { const i = new Image(); i.onload = () => ok(i); i.onerror = fail; i.src = url })
  function zoom(value: number) {
    if (!crop) return
    // Zoom about the frame's centre so the face you framed stays put.
    const before = scale, cx = FRAME / 2 - crop.x, cy = FRAME / 2 - crop.y
    crop.zoom = value
    const ratio = scale / before
    crop.x = FRAME / 2 - cx * ratio; crop.y = FRAME / 2 - cy * ratio
    clampCrop()
  }
  async function confirmCrop() {
    if (!crop) return
    const img = await load(crop.url), size = 512, canvas = document.createElement('canvas')
    canvas.width = canvas.height = size
    const k = size / FRAME
    canvas.getContext('2d')!.drawImage(img, crop.x * k, crop.y * k, crop.w * scale * k, crop.h * scale * k)
    const blob = await new Promise<Blob | null>((ok) => canvas.toBlob(ok, 'image/png'))
    cancelCrop()
    if (blob) await upload('avatar', blob)
  }
  function cancelCrop() { if (crop) URL.revokeObjectURL(crop.url); crop = null }
  async function upload(kind: 'avatar' | 'banner', image: Blob | null) {
    busyImage = kind; error = ''
    try { await store.setProfileImage(kind, image); message = image ? (kind === 'avatar' ? 'Picture updated.' : 'Banner updated.') : (kind === 'avatar' ? 'Picture removed.' : 'Banner removed.') }
    catch (err) { error = err instanceof Error ? err.message : 'That upload did not go through.' }
    finally { busyImage = null }
  }
</script>

<h2 class="display">Profile</h2>
<p class="muted lede">What people see when they click your name. It follows you to every device.</p>

<div class="layout">
  <form class="form" onsubmit={save}>
    <section>
      <h3>Pictures</h3>
      <div class="group">
        <SettingRow label="Profile picture" hint="Square works best. You can frame it before it uploads.">
          {#snippet control()}
            <label class="btn" class:busy={busyImage === 'avatar'}><Icon name="plus" size={14} />{busyImage === 'avatar' ? 'Uploading…' : 'Upload'}<input class="sr-only" type="file" accept="image/png,image/jpeg,image/webp,image/gif" onchange={(e) => { void choose('avatar', e.currentTarget.files?.[0]); e.currentTarget.value = '' }} /></label>
            {#if me.avatar_url}<button type="button" class="btn quiet danger" onclick={() => upload('avatar', null)} disabled={!!busyImage}>Remove</button>{/if}
          {/snippet}
        </SettingRow>
        {#if crop}
          <div class="cropper">
            <!-- svelte-ignore a11y_no_static_element_interactions (the zoom slider and buttons are the keyboard path) -->
            <div class="frame" style="--f:{FRAME}px"
              onpointerdown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); drag = { x: e.clientX, y: e.clientY, ox: crop!.x, oy: crop!.y } }}
              onpointermove={(e) => { if (!drag || !crop) return; crop.x = drag.ox + e.clientX - drag.x; crop.y = drag.oy + e.clientY - drag.y; clampCrop() }}
              onpointerup={() => (drag = null)} onpointercancel={() => (drag = null)}>
              <img src={crop.url} alt="" draggable="false" style="width:{crop.w * scale}px; height:{crop.h * scale}px; transform:translate({crop.x}px, {crop.y}px)" />
              <span class="mask" aria-hidden="true"></span>
            </div>
            <div class="crop-controls">
              <p class="muted">Drag to move. Zoom to fill the circle with the part you want.</p>
              <label class="zoom">Zoom<input type="range" min="1" max="4" step="0.01" value={crop.zoom} oninput={(e) => zoom(+e.currentTarget.value)} /></label>
              <div class="row">
                <button type="button" class="btn lit" onclick={confirmCrop}>Use this picture</button>
                <button type="button" class="btn quiet" onclick={cancelCrop}>Cancel</button>
              </div>
            </div>
          </div>
        {/if}
        <SettingRow label="Banner" hint="The strip across the top of your card. Wide pictures work best.">
          {#snippet control()}
            <label class="btn" class:busy={busyImage === 'banner'}><Icon name="plus" size={14} />{busyImage === 'banner' ? 'Uploading…' : 'Upload'}<input class="sr-only" type="file" accept="image/png,image/jpeg,image/webp,image/gif" onchange={(e) => { void choose('banner', e.currentTarget.files?.[0]); e.currentTarget.value = '' }} /></label>
            {#if me.banner_url}<button type="button" class="btn quiet danger" onclick={() => upload('banner', null)} disabled={!!busyImage}>Remove</button>{/if}
          {/snippet}
        </SettingRow>
      </div>
    </section>

    <section>
      <h3>About you</h3>
      <div class="group">
        <SettingRow label="Name" hint="What everyone sees. Emoji and any language are fine." wide>
          {#snippet control()}<input class="field" bind:value={displayName} maxlength="100" autocomplete="nickname" />{/snippet}
        </SettingRow>
        <SettingRow label="Bio" hint="A line or two. Shown as plain text." wide>
          {#snippet control()}
            <div class="counted">
              <textarea class="field" rows="3" bind:value={bio} placeholder="Builds redstone that mostly works."></textarea>
              <span class="count mono" class:over={[...bio].length > BIO_MAX}>{[...bio].length}/{BIO_MAX}</span>
            </div>
          {/snippet}
        </SettingRow>
        <SettingRow label="Profile colour" hint="Tints your picture and card when you have no banner.">
          {#snippet control()}
            <input class="swatch" type="color" value={accent ?? '#888888'} oninput={(e) => (accent = e.currentTarget.value)} aria-label="Profile colour" />
            {#if accent}<button type="button" class="btn quiet" onclick={() => (accent = null)}>Use automatic</button>{:else}<span class="faint small">Automatic</span>{/if}
          {/snippet}
        </SettingRow>
      </div>
    </section>

    <section>
      <h3>Status</h3>
      <div class="group">
        <SettingRow label="What you're up to" hint="One emoji and a short line, shown on your card." wide>
          {#snippet control()}
            <div class="status-row">
              <input class="field emoji" bind:value={emoji} placeholder="🎮" aria-label="Status emoji" />
              <input class="field" bind:value={statusText} maxlength="80" placeholder="Grinding the End city" aria-label="Status text" />
            </div>
          {/snippet}
        </SettingRow>
        <SettingRow label="Clear after" hint="Statuses go away on their own if you like.">
          {#snippet control()}
            <select class="field" bind:value={clearAfter} aria-label="Clear status after">
              {#if keepUntil}<option value="keep">Keep current (clears {keepUntil})</option>{/if}
              <option value="never">Don't clear</option>
              <option value="30m">30 minutes</option>
              <option value="1h">1 hour</option>
              <option value="4h">4 hours</option>
              <option value="today">End of today</option>
            </select>
            {#if status}<button type="button" class="btn quiet" onclick={() => { emoji = ''; statusText = '' }}>Clear status</button>{/if}
          {/snippet}
        </SettingRow>
      </div>
    </section>

    <section>
      <h3>Activity</h3>
      <div class="group">
        {#if native}
          <SettingRow label="Share what I'm playing or using" hint="Only the app's name is shared, never window titles. Steam games and Minecraft show as Playing. It clears after 10 idle minutes or when you lock your screen. This device only.">
            {#snippet control()}
              <label class="switch"><input type="checkbox" checked={activityShare.prefs.share} onchange={(e) => activityShare.save({ share: e.currentTarget.checked })} /><span class="sr-only">Share activity</span></label>
            {/snippet}
          </SettingRow>
          <SettingRow label="Right now" hint={activityShare.detected ? (activityShare.shared ? 'Everyone can see this.' : 'Hidden. Nobody sees it.') : 'Nothing detected.'}>
            {#snippet control()}
              {#if activityShare.detected}<span class="now" class:off={!activityShare.shared}>{activityVerb(activityShare.detected.kind)} <b>{activityShare.detected.name}</b></span>{:else}<span class="faint small">—</span>{/if}
            {/snippet}
          </SettingRow>
          {#if activityShare.prefs.seen.length}
            <SettingRow label="Recently seen" hint="Hide any app you would rather not show." wide>
              {#snippet control()}
                <ul class="apps">
                  {#each activityShare.prefs.seen as name (name)}
                    {@const hidden = activityShare.prefs.hidden.includes(name)}
                    <li><span class:off={hidden}>{name}</span><button type="button" class="btn quiet" onclick={() => activityShare.hide(name, !hidden)}>{hidden ? 'Show' : 'Hide'}</button></li>
                  {/each}
                </ul>
              {/snippet}
            </SettingRow>
          {/if}
        {:else}
          <SettingRow label="Games and apps" hint="The Den desktop app shares what you're playing or using. Browsers can't see other apps.">
            {#snippet control()}<a class="btn quiet" href="https://github.com/nhclink16/den/releases/latest" target="_blank" rel="noopener noreferrer">Get the desktop app</a>{/snippet}
          </SettingRow>
        {/if}
        <SettingRow label="Music" hint={store.spotify.share_listening ? "People see what you're listening to on Spotify while you're here." : "Off. Connect Spotify and turn on sharing to show what you're listening to."}>
          {#snippet control()}<button type="button" class="btn quiet" onclick={() => router.go('/settings/spotify')}>Spotify settings</button>{/snippet}
        </SettingRow>
      </div>
    </section>

    <div class="actions">
      <button class="btn lit" disabled={!dirty || !!problem || saving}>{saving ? 'Saving…' : 'Save profile'}</button>
      {#if dirty}<button type="button" class="btn quiet" onclick={reset}>Discard changes</button>{/if}
      <p class="note" class:error={!!(problem || error)} role="status">{problem || error || message}</p>
    </div>
  </form>

  <aside class="preview" aria-label="Preview">
    <span class="eyebrow">Preview</span>
    <UserCard userId={me.id} {draft} />
  </aside>
</div>

<style>
  .lede { margin: 0 0 18px; }
  .layout { display: grid; grid-template-columns: minmax(0, 1fr) 300px; gap: 28px; align-items: start; }
  .preview { position: sticky; top: 0; display: grid; gap: 8px; }
  @media (max-width: 980px) { .layout { grid-template-columns: 1fr; } .preview { position: static; order: -1; } }
  section { margin-bottom: 22px; }
  .group { border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-2); overflow: hidden; }
  label.btn { cursor: pointer; }
  label.btn.busy { opacity: .6; pointer-events: none; }
  label.btn:focus-within { outline: 2px solid var(--lamp); outline-offset: 2px; }
  .counted { position: relative; width: 100%; }
  .counted textarea { width: 100%; }
  .counted textarea { resize: vertical; min-height: 72px; padding-bottom: 22px; }
  .count { position: absolute; right: 10px; bottom: 8px; font-size: 11px; color: var(--ink-3); }
  .count.over { color: var(--danger); }
  .swatch { width: 44px; height: 36px; padding: 2px; border: 1px solid var(--line-strong); border-radius: var(--r); background: var(--bg); cursor: pointer; }
  .small { font-size: 13px; }
  .status-row { display: grid; grid-template-columns: 64px 1fr; gap: 8px; width: 100%; }
  .emoji { text-align: center; font-size: 18px; }
  select.field { width: auto; }
  .actions { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; }
  .note { margin: 0; font-size: 13px; color: var(--ink-2); flex-basis: 100%; min-height: 1.4em; }
  .note.error { color: var(--danger); }

  /* Framing a picture: a square frame with a round guide, since most avatars are cut round-ish. */
  .cropper { display: flex; flex-wrap: wrap; gap: 20px; align-items: center; padding: 16px; border-bottom: 1px solid var(--line); background: var(--bg); }
  .frame { position: relative; width: var(--f); height: var(--f); overflow: hidden; border-radius: var(--r); background: var(--bg-3); cursor: grab; touch-action: none; user-select: none; }
  .frame:active { cursor: grabbing; }
  .frame img { position: absolute; left: 0; top: 0; max-width: none; transform-origin: 0 0; pointer-events: none; }
  .mask { position: absolute; inset: 0; border-radius: var(--avatar-r, 35%); box-shadow: 0 0 0 999px color-mix(in srgb, #000 62%, transparent); outline: 2px solid color-mix(in srgb, #fff 70%, transparent); pointer-events: none; }
  .crop-controls { flex: 1; min-width: 200px; display: grid; gap: 12px; }
  .crop-controls p { margin: 0; font-size: 13px; }
  .zoom { display: grid; gap: 6px; font-size: 13px; color: var(--ink-2); }
  .zoom input { accent-color: var(--lamp); }
  .row { display: flex; gap: 8px; flex-wrap: wrap; }
  .switch input { width: 18px; height: 18px; accent-color: var(--lamp); }
  .now { font-size: 14px; } .now b { font-weight: 700; } .now.off { color: var(--ink-3); text-decoration: line-through; }
  .apps { list-style: none; margin: 0; padding: 0; display: grid; gap: 2px; width: 100%; }
  .apps li { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 2px 0; }
  .apps .off { color: var(--ink-3); text-decoration: line-through; }
</style>
