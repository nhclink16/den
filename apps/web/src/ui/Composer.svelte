<script lang="ts">
  import { store } from '../lib/store.svelte'
  import type { Channel, Message, Upload } from '../lib/types'
  import { upload as send, type Progress } from '../lib/upload'
  import { bytes } from '../lib/time'
  import Icon from './Icon.svelte'

  let { channel, replyTo = $bindable(null), dropped = $bindable([]) }: { channel: Channel; replyTo: Message | null; dropped: File[] } = $props()

  type Pending = { id: number; file: File; progress: Progress; done?: Upload; error?: string; abort: AbortController }
  let seq = 0
  let text = $state('')
  let pending = $state.raw<Pending[]>([])
  let busy = $state(false)
  let ta: HTMLTextAreaElement
  let fileInput: HTMLInputElement
  const MAX = 1024 ** 3

  const placeholder = $derived(channel.kind === 'dm' ? `Message ${store.title(channel)}` : `Say something in #${channel.name}`)
  const uploading = $derived(pending.some((p) => !p.done && !p.error))

  $effect(() => { if (dropped.length) { add(dropped); dropped = [] } })
  $effect(() => { if (replyTo) ta?.focus() })

  // Pending entries are replaced, never mutated, so the keyed list re-renders.
  function patch(id: number, part: Partial<Pending>) { pending = pending.map((x) => (x.id === id ? { ...x, ...part } : x)) }

  function add(files: File[]) {
    for (const file of files) {
      const p: Pending = { id: ++seq, file, progress: { sent: 0, total: file.size }, abort: new AbortController() }
      if (file.size > MAX) { pending = [...pending, { ...p, error: 'Over the 1 GB limit' }]; continue }
      pending = [...pending, p]
      send(channel.id, file, (progress) => patch(p.id, { progress }), p.abort.signal)
        .then((done) => patch(p.id, { done }))
        .catch((e) => patch(p.id, { error: e.name === 'AbortError' ? 'Cancelled' : e.message || 'Upload failed' }))
    }
    ta?.focus()
  }

  function removePending(p: Pending) { p.abort.abort(); pending = pending.filter((x) => x.id !== p.id) }

  function onPaste(e: ClipboardEvent) {
    const files = [...(e.clipboardData?.files || [])]
    if (files.length) { e.preventDefault(); add(files) }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); submit() }
    if (e.key === 'Escape' && replyTo) replyTo = null
    if (e.key === 'ArrowUp' && !text) {
      const mine = [...(store.messages.get(channel.id) || [])].reverse().find((m) => m.author_id === store.me?.id)
      if (mine) document.getElementById(`m-${mine.id}`)?.querySelector<HTMLButtonElement>('button[title="Edit"]')?.click()
    }
  }

  function grow() { ta.style.height = 'auto'; ta.style.height = Math.min(ta.scrollHeight, 220) + 'px'; if (text.trim()) store.sendTyping(channel.id) }

  async function submit() {
    const content = text.trim()
    const ready = pending.filter((p) => p.done).map((p) => p.done!.id)
    if ((!content && !ready.length) || uploading || busy) return
    busy = true
    try {
      await store.send(channel.id, content, { reply_to: replyTo?.id, upload_ids: ready })
      text = ''; replyTo = null
      pending = pending.filter((p) => !p.done || !ready.includes(p.done.id))
      requestAnimationFrame(grow)
    } finally {
      busy = false
    }
  }
</script>

<div class="composer">
  {#if replyTo}
    <div class="reply-bar">
      <Icon name="reply" size={13} />
      <span>Replying to <b>{store.name(replyTo.author_id)}</b></span>
      <span class="snippet faint">{replyTo.content.slice(0, 80)}</span>
      <button class="x" onclick={() => (replyTo = null)} aria-label="Cancel reply"><Icon name="x" size={14} /></button>
    </div>
  {/if}

  {#if pending.length}
    <div class="pending">
      {#each pending as p (p.id)}
        {@const pct = p.progress.total ? Math.round((p.progress.sent / p.progress.total) * 100) : 0}
        <div class="chip" class:err={!!p.error} class:done={!!p.done}>
          <span class="ring" style="--p:{p.done ? 100 : pct}"></span>
          <span class="fname">{p.file.name}</span>
          <span class="faint mono">{p.error || (p.done ? bytes(p.file.size) : `${pct}%`)}</span>
          <button class="x" onclick={() => removePending(p)} aria-label="Remove"><Icon name="x" size={13} /></button>
        </div>
      {/each}
    </div>
  {/if}

  <div class="box" class:uploading>
    <button class="attach" title="Attach a file" onclick={() => fileInput.click()}><Icon name="clip" size={18} /></button>
    <input class="sr-only" type="file" multiple bind:this={fileInput} onchange={(e) => { add([...(e.currentTarget.files || [])]); e.currentTarget.value = '' }} tabindex="-1" />
    <textarea bind:this={ta} bind:value={text} {placeholder} rows="1" oninput={grow} onkeydown={onKey} onpaste={onPaste} aria-label={placeholder}></textarea>
    <button class="sendbtn" class:ready={text.trim() || pending.some((p) => p.done)} onclick={submit} disabled={uploading || busy} title="Send (Enter)"><Icon name="send" size={16} /></button>
  </div>
</div>

<style>
  .composer { padding: 0 16px 14px; }
  .box {
    display: flex; align-items: flex-end; gap: 6px; padding: 6px 6px 6px 8px;
    background: var(--bg-3); border: 1px solid var(--line); border-radius: var(--r-lg);
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .box:focus-within { border-color: var(--lamp); box-shadow: 0 0 0 3px var(--lamp-glow); }
  textarea {
    flex: 1; resize: none; background: none; border: 0; outline: 0; padding: 8px 4px;
    max-height: 220px; line-height: 1.4; color: var(--ink);
  }
  textarea::placeholder { color: var(--ink-3); }
  .attach, .sendbtn { display: grid; padding: 9px; border-radius: 8px; color: var(--ink-3); flex: none; }
  .attach:hover { color: var(--ink); background: var(--bg-2); }
  .sendbtn.ready { color: var(--lamp); }
  .sendbtn:disabled { opacity: 0.4; }
  .reply-bar, .pending {
    display: flex; align-items: center; gap: 8px; padding: 6px 12px; margin: 0 8px;
    background: var(--bg-2); border: 1px solid var(--line); border-bottom: 0;
    border-radius: var(--r-lg) var(--r-lg) 0 0; font-size: 13px;
  }
  .pending { flex-wrap: wrap; gap: 6px; }
  .snippet { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .x { display: grid; padding: 3px; border-radius: 4px; color: var(--ink-3); }
  .x:hover { color: var(--ink); background: var(--bg-3); }
  .chip {
    display: inline-flex; align-items: center; gap: 8px; padding: 5px 8px 5px 6px;
    background: var(--bg-3); border-radius: 999px; max-width: 100%;
  }
  .chip.err { color: var(--ember); }
  .ring {
    width: 14px; height: 14px; border-radius: 50%; flex: none;
    background: conic-gradient(var(--lamp) calc(var(--p) * 1%), var(--line) 0);
    mask: radial-gradient(circle, transparent 45%, #000 50%);
  }
  .chip.done .ring { background: var(--lamp); mask: none; }
  .fname { max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
