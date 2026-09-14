<script lang="ts">
  import { onMount } from 'svelte'
  import { call } from '../lib/call.svelte'
  import Icon from './Icon.svelte'
  import type { Dictation, DictationEvent } from '../lib/dictation'
  import type { DictationDraft } from '../lib/dictation-draft'
  let { textarea, text, update, listening = $bindable(false), channelId }: { textarea: () => HTMLTextAreaElement; text: () => string; update: (value: string, caret: number) => void; listening?: boolean; channelId: string } = $props()
  let supported = $state(false), phase = $state('idle'), error = $state(''), progress = $state(0)
  let button = $state<HTMLButtonElement>(), dialog: HTMLDialogElement
  let engine: Dictation | undefined, draft: DictationDraft | undefined, lastApplied = ''
  let modules: [typeof import('../lib/dictation'), typeof import('../lib/dictation-draft')] | undefined
  let generation = 0, caret = 0
  const denied = 'Den needs the microphone for dictation. Allow it in Settings.'
  // A second capture could change a live call's input route. Hide during calls.
  const available = $derived(supported && !call.channel && !call.joining)
  function reset() { ++generation; engine?.dispose(); engine = undefined; phase = 'idle'; listening = false }
  function stop() {
    listening = false
    if (phase === 'listening') { phase = 'stopping'; engine?.stop() }
    else { reset(); dialog?.close() }
  }
  $effect(() => { void channelId; return reset })
  $effect(() => { if (!available && phase !== 'idle') reset() })
  function receive(event: DictationEvent) {
    if (event.type === 'loading') { phase = 'loading'; progress = Math.round(event.progress || 0) }
    if (event.type === 'listening') {
      phase = 'listening'; listening = true
      const punctuation = localStorage.getItem('den.dictation.punctuation') !== 'false'
      draft = new modules![1].DictationDraft(text(), Math.min(caret, text().length), punctuation)
      lastApplied = text(); textarea().focus()
    }
    if (event.type === 'text' && draft) {
      if (text() !== lastApplied) { reset(); return } // Preserve typing, sends and edits during final inference.
      const value = draft.accept(event.text, event.final); lastApplied = value.text; update(value.text, value.caret)
    }
    if (event.type === 'stopped') { phase = 'idle'; listening = false }
    if (event.type === 'error') { error = event.message; phase = 'idle'; listening = false }
  }
  async function start() {
    dialog.close(); phase = 'loading'; const current = ++generation
    engine = new modules![0].Dictation(event => { if (generation === current) receive(event) })
    await engine.start(call.prefs.microphone)
  }
  async function toggle() {
    if (phase !== 'idle') { stop(); return }
    if (error === denied) {
      const permission = await navigator.permissions?.query({ name: 'microphone' as PermissionName }).catch(() => null)
      if (!permission || permission.state === 'denied') return
    }
    error = ''; caret = textarea().selectionStart
    phase = 'checking'; const current = ++generation
    try {
      modules = await Promise.all([import('../lib/dictation'), import('../lib/dictation-draft')])
      const cached = await modules[0].hasModel()
      if (current !== generation) return
      if (cached) await start()
      else { phase = 'prompt'; dialog.showModal() }
    } catch { if (current === generation) { error = 'Dictation is unavailable in this browser.'; reset() } }
  }
  onMount(() => {
    supported = !!(globalThis.isSecureContext && typeof navigator.mediaDevices?.getUserMedia === 'function' && 'Worker' in globalThis && 'WebAssembly' in globalThis && 'AudioWorkletNode' in globalThis && 'indexedDB' in globalThis)
    const outside = (event: PointerEvent) => { if (phase === 'listening' && !button?.contains(event.target as Node)) stop() }
    const key = (event: KeyboardEvent) => {
      if (phase !== 'idle' && phase !== 'prompt' && ['Escape', 'Enter'].includes(event.key)) { event.preventDefault(); event.stopImmediatePropagation(); stop() }
    }
    const hidden = () => { if (document.hidden && phase !== 'idle') stop() }
    document.addEventListener('pointerdown', outside, true); document.addEventListener('keydown', key, true); document.addEventListener('visibilitychange', hidden)
    return () => { reset(); document.removeEventListener('pointerdown', outside, true); document.removeEventListener('keydown', key, true); document.removeEventListener('visibilitychange', hidden) }
  })
</script>

{#if available}
  <button bind:this={button} class="dictate" class:listening class:working={phase !== 'idle'} title={error === denied ? denied : phase === 'idle' ? 'Dictate' : 'Stop dictating'} aria-label={phase === 'idle' ? 'Dictate' : 'Stop dictating'} aria-pressed={listening} onclick={toggle}><Icon name="mic" size={18} /></button>
{/if}
{#if phase === 'loading'}<span class="notice mono" role="status">Preparing dictation{progress ? ` · ${progress}%` : ''}</span>{/if}
{#if error}<span class="notice error" role="alert">{error}</span>{/if}
<dialog aria-label="Download voice model" bind:this={dialog} oncancel={() => reset()} onclose={() => { if (phase === 'prompt') reset() }}>
  <p>Dictation runs on this device. Download the voice model (40 MB) once?</p>
  <div class="actions"><button class="btn quiet" onclick={() => { dialog.close(); reset() }}>Not now</button><button class="btn lit" onclick={start}>Download</button></div>
</dialog>

<style>
  .dictate { display: grid; padding: 9px; border-radius: 8px; color: var(--ink-3); flex: none; position: relative; }
  @media (hover: hover) { .dictate:hover { color: var(--ink); background: var(--bg-2); } }
  .dictate.listening { color: var(--lamp); background: var(--lamp-glow); }
  .dictate.listening::after { content: ''; position: absolute; inset: 0; border: 1px solid var(--lamp); border-radius: inherit; pointer-events: none; }
  @media (prefers-reduced-motion: no-preference) {
    .dictate.listening::after { animation: listen 1.2s ease infinite; }
    @keyframes listen { from { opacity: .5; transform: scale(1); } to { opacity: 0; transform: scale(1.2); } }
  }
  .notice { position: absolute; bottom: 100%; left: 20px; right: 20px; padding: 6px 8px; border-radius: 6px; background: var(--bg-2); font-size: 12px; }
  .error { color: var(--ember); }
  dialog { max-width: min(360px, calc(100vw - 32px)); border: 1px solid var(--line); border-radius: var(--r-lg); padding: 20px; color: var(--ink); background: var(--bg-2); overscroll-behavior: contain; }
  dialog::backdrop { background: #0006; }
  dialog p { margin: 0 0 20px; }
  .actions { display: flex; justify-content: flex-end; gap: 8px; }
</style>
