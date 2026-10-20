<script lang="ts">
  import { mediaUrl, native } from '../lib/native'
  import { modal } from '../lib/modal'
  import type { Upload } from '../lib/types'
  import { fileUrl } from '../lib/upload'
  import { bytes } from '../lib/time'
  import SoundCard from './SoundCard.svelte'
  import Icon from './Icon.svelte'

  let { upload, author = 'a member', onmediaready }: { upload: Upload; author?: string; onmediaready?: () => void } = $props()
  const url = $derived(fileUrl(upload.id))
  const kind = $derived(upload.content_type.startsWith('video/') ? 'video' : upload.content_type.startsWith('image/') ? 'image' : upload.content_type.startsWith('audio/') ? 'audio' : 'file')
  // The frame takes the picture's own shape once it loads, no wider than the thumbnail
  // and no taller than 420px; until then a 4:3 box holds its place.
  let shape = $state<{ ratio: number; width: number } | undefined>()
  function loaded(e: Event) {
    const img = e.currentTarget as HTMLImageElement
    if (img.naturalWidth && img.naturalHeight) shape = { ratio: img.naturalWidth / img.naturalHeight, width: img.naturalWidth }
    onmediaready?.()
  }

  // A picture opens over Den, never in place of it: a phone shell has no back button.
  // Modified clicks still open the file in a new tab.
  let viewing = $state(false)
  function view(e: MouseEvent) {
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return
    e.preventDefault()
    viewing = true
  }
  // Swipe down to close, as in Photos. Any drag is not also a tap outside the picture.
  let start: { x: number; y: number } | undefined
  let drop = $state(0)
  let swiped = false
  function grab(e: PointerEvent) { if (e.isPrimary) { start = { x: e.clientX, y: e.clientY }; drop = 0; swiped = false } }
  function drag(e: PointerEvent) {
    if (!start) return
    drop = Math.max(0, e.clientY - start.y)
    if (Math.hypot(e.clientX - start.x, e.clientY - start.y) > 10) swiped = true
  }
  function release() {
    if (!start) return
    start = undefined
    if (drop > 80) viewing = false
    drop = 0
  }
</script>

{#if upload.filename.endsWith('.den-sounds.zip') || ['audio/ogg', 'audio/mpeg', 'audio/wav'].includes(upload.content_type)}
  <SoundCard {upload} {author} />
{:else if kind === 'video'}
  <!-- iOS paints no first frame for preload="metadata" until a seek; the fragment is that seek. -->
  <!-- svelte-ignore a11y_media_has_caption -->
  <video class="media" src="{url}#t=0.1" controls preload="metadata" playsinline onloadedmetadata={onmediaready}></video>
{:else if kind === 'image'}
  <a class="image" href={url} target="_blank" rel="noopener" onclick={view} style:aspect-ratio={shape?.ratio} style:inline-size={shape && `min(512px, 100%, ${shape.width}px, ${420 * shape.ratio}px)`}><img class="media" src={upload.thumbnail_url ? mediaUrl(upload.thumbnail_url) : url} alt={upload.filename} loading="lazy" onload={loaded} /></a>
  {#if viewing}
    <!-- The explicit role keeps call shortcuts (push-to-talk, V for camera) out of the viewer. -->
    <!-- svelte-ignore a11y_no_redundant_roles -->
    <dialog class="viewer" role="dialog" use:modal aria-label={upload.filename} style:background-color="rgb(0 0 0 / {0.96 - drop / 500})" oncancel={(e) => { e.preventDefault(); viewing = false }}
      onclick={(e) => { if (!swiped && !(e.target as Element).closest('img, a, button')) viewing = false }}
      onpointermove={drag} onpointerup={release} onpointercancel={release}>
      <div class="bar">
        <span class="name">{upload.filename}</span>
        <!-- The desktop apps can't open a den-media: file in a window; the viewer already shows it full size. -->
        {#if !native}<a class="act" href={url} target="_blank" rel="noopener"><Icon name="popout" />Open original</a>{/if}
        <button class="act" aria-label="Close" data-initial-focus onclick={() => (viewing = false)}><Icon name="x" size={20} /></button>
      </div>
      <div class="stage" role="presentation" onpointerdown={grab}>
        <img src={url} alt={upload.filename} style:transform={drop ? `translateY(${drop}px)` : null} />
      </div>
    </dialog>
  {/if}
{:else if kind === 'audio'}
  <audio class="audio" src={url} controls preload="metadata"></audio>
{:else}
  <a class="file" href={url} download={upload.filename}>
    <Icon name="file" size={20} />
    <span class="fname">{upload.filename}</span>
    <span class="faint">{bytes(upload.size)}</span>
  </a>
{/if}

<style>
  .media { max-width: min(560px, 100%); max-height: 420px; border-radius: var(--r); background: var(--bg); display: block; }
  /* align-self: iOS 26 WebKit kept stretching the frame to the first 4:3 line height
     of the attachments row after the shape changed. */
  .image { display: block; align-self: flex-start; inline-size: min(512px, 100%); aspect-ratio: 4 / 3; border-radius: var(--r); background: var(--bg-3); overflow: hidden; }
  .image img.media { width: 100%; height: 100%; max-width: none; max-height: none; background: transparent; object-fit: contain; }
  .audio { width: min(420px, 100%); }
  .file {
    display: inline-flex; align-items: center; gap: 10px; padding: 10px 14px;
    background: var(--bg-3); border: 1px solid var(--line); border-radius: var(--r); color: var(--ink);
  }
  @media (hover: hover) { .file:hover { text-decoration: none; border-color: var(--ink-3); } }
  .fname { max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  /* A photo viewer is dark in every theme, like Photos. */
  .viewer {
    inline-size: 100%; block-size: 100%; max-inline-size: none; max-block-size: none; margin: 0; padding: 0; border: 0;
    color: #fff;
    /* It sits inside a message, which turns these off on touch screens; here Save Image belongs. */
    -webkit-touch-callout: default; -webkit-user-select: auto; user-select: auto;
  }
  .viewer[open] { display: flex; flex-direction: column; }
  .viewer::backdrop { background: transparent; }
  .bar {
    display: flex; align-items: center; gap: 4px;
    /* Clear the notch, and the desktop app's native window controls (--wco-*). */
    padding: max(8px, env(safe-area-inset-top)) max(8px, env(safe-area-inset-right), var(--wco-right, 0px)) 8px max(16px, env(safe-area-inset-left), calc(var(--wco-left, 0px) + 16px));
  }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-sm); color: rgb(255 255 255 / .75); }
  .act {
    display: inline-flex; align-items: center; gap: 6px; min-block-size: 44px; min-inline-size: 44px; justify-content: center; padding: 0 12px;
    border-radius: var(--r); color: #fff; font-size: var(--fs-sm); white-space: nowrap;
  }
  @media (hover: hover) { .act:hover { text-decoration: none; } }
  @media (hover: hover) { .act:hover { background: rgb(255 255 255 / .12); } }
  .stage {
    flex: 1; min-block-size: 0; display: grid; grid-template: minmax(0, 1fr) / minmax(0, 1fr); place-items: center; padding: 0 8px max(8px, env(safe-area-inset-bottom));
    touch-action: none; overscroll-behavior: contain;
  }
  .stage img { max-inline-size: 100%; max-block-size: 100%; object-fit: contain; }
</style>
