<script lang="ts">
  import type { Upload } from '../lib/types'
  import { fileUrl } from '../lib/upload'
  import { bytes } from '../lib/time'
  import Icon from './Icon.svelte'

  let { upload }: { upload: Upload } = $props()
  const url = $derived(fileUrl(upload.id))
  const kind = $derived(upload.content_type.startsWith('video/') ? 'video' : upload.content_type.startsWith('image/') ? 'image' : upload.content_type.startsWith('audio/') ? 'audio' : 'file')
</script>

{#if kind === 'video'}
  <!-- svelte-ignore a11y_media_has_caption -->
  <video class="media" src={url} controls preload="metadata" playsinline></video>
{:else if kind === 'image'}
  <a href={url} target="_blank" rel="noopener"><img class="media" src={upload.thumbnail_url || url} alt={upload.filename} loading="lazy" /></a>
{:else if kind === 'audio'}
  <audio class="audio" src={url} controls preload="metadata"></audio>
{:else}
  <a class="file" href={url} download={upload.filename}>
    <Icon name="file" size={20} />
    <span class="fname">{upload.filename}</span>
    <span class="faint mono">{bytes(upload.size)}</span>
  </a>
{/if}

<style>
  .media { max-width: min(560px, 100%); max-height: 420px; border-radius: var(--r); background: var(--bg); display: block; }
  img.media { background: transparent; object-fit: contain; }
  .audio { width: min(420px, 100%); }
  .file {
    display: inline-flex; align-items: center; gap: 10px; padding: 10px 14px;
    background: var(--bg-3); border: 1px solid var(--line); border-radius: var(--r); color: var(--ink);
  }
  .file:hover { text-decoration: none; border-color: var(--ink-3); }
  .fname { max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
