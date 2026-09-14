<script lang="ts">
  import { mediaUrl } from '../../lib/native'
  import { onMount } from 'svelte'
  import { api } from '../../lib/api'
  import { store } from '../../lib/store.svelte'
  import { terminals, load, catalog, capability } from './state.svelte'
  import { subscribe } from './stream'
  import type { ObjectSummary, TerminalState, LiveObject } from '../../lib/types'
  let { object, compact = false }: { object: ObjectSummary; compact?: boolean } = $props()
  let host: HTMLDivElement
  let sessionId = $state('')
  let error = $state('')
  let direct = $state(false)
  let focused = $state(false)
  let hint = $state(false)
  let mounted = $state(false)
  let shareTo = $state('')
  let time = $state(0)
  let duration = $state(0)
  let playing = $state(false)
  let replayLoaded = ''
  let replay: [number, Uint8Array][] = []
  let replayAt = 0
  let elapsed = 0
  let editor: Awaited<ReturnType<typeof import('./renderer').mount>> | undefined
  let stream: ReturnType<typeof subscribe> | undefined
  let attach = () => {}
  const t = $derived(terminals.sessions[sessionId])
  const owner = $derived(!!t && t.owner_id === store.me?.id)
  const canView = $derived(!!t && capability(t))
  const canControl = $derived(!!t && capability(t, true))
  const controller = $derived(!!t && !t.ended_at && canControl && t.active_controller_id === store.me?.id)
  const name = (id: string) => store.user(id)?.username || 'someone'
  $effect(() => { void object.version; load(object).then(value => { if (value) sessionId = value.id }).catch(e => error = e.message) })
  $effect(() => {
    if (mounted && t && editor && !controller && (editor.term.cols !== t.cols || editor.term.rows !== t.rows)) {editor.term.resize(t.cols, t.rows); editor.fitScreen()}
    if (mounted && t && !t.ended_at) {if (canView && !stream) attach(); else if (!canView && stream) {stream.destroy(); stream = undefined; editor?.term.reset()}}
    if (mounted && canView && t?.recording_upload_id && replayLoaded !== t.recording_upload_id) void recording(t)
  })
  async function act(path: string, body = {}) { try { await api.post(path, body); await catalog(); error = '' } catch (e) { error = (e as Error).message } }
  async function request(control: boolean) {
    if (!t) return
    if (control && canControl) { await act(`/sessions/${t.id}/request-control`); return }
    try {
      const o = await api.post<LiveObject>(`/hosts/${t.host_id}/requests`, { capability: control ? 'terminal_control' : 'terminal_view', duration_minutes: 60 })
      await store.resync(); await store.loadLatest(o.channel_id); error = 'Request sent to the owner.'
    } catch (e) { error = (e as Error).message }
  }
  async function share() {
    if (!t || !shareTo) return
    await act(`/sessions/${t.id}/share`, { channel_id: shareTo }); await store.loadLatest(shareTo); shareTo = ''
  }
  async function revokeControl() {
    if (!t?.active_controller_id) return
    try {await catalog(); for (const g of terminals.grants.filter(g => g.host_id === t.host_id && g.grantee_id === t.active_controller_id && g.capability === 'terminal_control')) await api.del(`/grants/${g.id}`); await catalog()} catch (e) {error = (e as Error).message}
  }
  async function end() { if (!t) return; try { await api.del(`/sessions/${t.id}`) } catch (e) { error = (e as Error).message } }
  async function recording(state: TerminalState) {
    if (!canView || !editor) return
    replayLoaded = state.recording_upload_id!; stream?.destroy(); stream = undefined; direct = false
    try {
      const response = await fetch(mediaUrl(`/uploads/${state.recording_upload_id}/file`))
      if (!response.ok) throw new Error('Recording access denied.')
      replay = (await response.text()).trim().split('\n').filter(Boolean).map(line => { const [ms, bytes] = JSON.parse(line); return [ms, Uint8Array.from(atob(bytes), c => c.charCodeAt(0))] })
      duration = replay.at(-1)?.[0] || 1; seek(0); playing = true
    } catch (e) { error = (e as Error).message }
  }
  function replayBytes(bytes: Uint8Array) {
    // Recordings prefix each output with its recorded terminal dimensions (CSI 8).
    const header = new TextDecoder().decode(bytes.subarray(0, 24)).match(/^\x1b\[8;(\d+);(\d+)t/)
    if (header) {editor?.term.resize(+header[2], +header[1]); editor?.fitScreen(); bytes = bytes.subarray(header[0].length)}
    editor?.term.write(bytes)
  }
  function seek(value: number) {
    playing = false
    time = value; elapsed = value; replayAt = 0
    // RIS resets Ghostty without freeing the buffer still referenced by its renderer.
    editor?.term.write('\x1bc'); editor?.term.renderer?.clear()
    while (replayAt < replay.length && replay[replayAt][0] <= value) replayBytes(replay[replayAt++][1])
    editor?.redraw()
  }
  function focus() {
    focused = true
    if (!t?.ended_at && !sessionStorage.getItem(`den.terminal.hint.${sessionId}`)) {hint = true; sessionStorage.setItem(`den.terminal.hint.${sessionId}`, '1'); setTimeout(() => hint = false, 5000)}
  }
  onMount(() => {
    let dead = false
    let observer: ResizeObserver | undefined
    let lastEscape = 0
    let previewTimer: ReturnType<typeof setTimeout> | undefined
    let ended = false
    async function start() {
      const value = await load(object); await catalog(); if (!value || dead) return
      sessionId = value.id
      const renderer = await import('./renderer'); if (dead) return
      editor = await renderer.mount(host, value.cols, value.rows, text => { if (controller) stream?.input(text) }); if (dead) {editor.destroy(); return}
      editor.term.attachCustomKeyEventHandler(e => {
        if (e.type === 'keydown' && e.key === 'Escape') {
          const now = performance.now()
          if (now - lastEscape < 400) { e.preventDefault(); (document.activeElement as HTMLElement)?.blur(); focused = false; lastEscape = 0; return true }
          lastEscape = now
        }
        // ghostty-web 0.4.0 returns true when the custom handler consumes a key.
        return false
      })
      const bytes = (bytes: Uint8Array) => editor?.term.write(bytes, () => {
        if (!previewTimer) previewTimer = setTimeout(() => {previewTimer = undefined; if (editor) {terminals.previews[sessionId] = renderer.screen(editor.term); editor.redraw()}}, 100)
      })
      attach = () => {stream = subscribe(sessionId, { resize: (cols, rows) => {editor?.term.resize(cols, rows); if (compact || !controller) editor?.fitScreen()}, bytes, reset: () => editor?.term.reset(), path: value => direct = value })}
      const fit = () => { if (editor && controller && !compact) { editor.term.options.fontSize = 14; editor.fit.fit(); stream?.resize(editor.term.cols, editor.term.rows) } else editor?.fitScreen() }
      observer = new ResizeObserver(fit); observer.observe(host); mounted = true; fit()
      if (!value.ended_at && capability(value)) {attach(); fit()}
      if (value.ended_at) await recording(value)
    }
    start().catch(e => error = e.message)
    const timer = setInterval(() => {
      if (playing && editor && replay.length) {
        elapsed = Math.min(duration, elapsed + 16); time = elapsed
        while (replayAt < replay.length && replay[replayAt][0] <= time) replayBytes(replay[replayAt++][1])
        editor.redraw()
        if (time >= duration) playing = false
      }
      if (t?.ended_at && !ended) { ended = true; stream?.destroy(); stream = undefined }
    }, 16)
    return () => {dead = true; observer?.disconnect(); clearInterval(timer); clearTimeout(previewTimer); stream?.destroy(); editor?.destroy()}
  })
</script>
<div class="terminal-view" class:compact data-terminal-focus={focused}>
  {#if t && t.active_controller_id && t.active_controller_id !== t.owner_id && !t.ended_at}
    <div class="control-banner" data-testid="terminal-control-banner"><span>{name(t.active_controller_id)} has control</span>{#if owner}<button onclick={() => act(`/sessions/${t.id}/controller`, { user_id: t.owner_id })}>Take back</button><button onclick={revokeControl}>Revoke control</button>{/if}</div>
  {/if}
  {#if owner && t?.control_request_ids.length}
    <div class="control-prompt">{name(t.control_request_ids[0])} wants control · <button onclick={() => act(`/sessions/${t.id}/controller`, { user_id: t.control_request_ids[0] })}>Give</button> · <button onclick={() => act(`/sessions/${t.id}/controller`, { user_id: t.active_controller_id })}>Not now</button></div>
  {/if}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="terminal-screen" bind:this={host} role="group" aria-label={`Terminal on ${object.name}`} onfocusin={focus} onfocusout={() => focused = false} onkeydown={e => e.stopPropagation()} onkeyup={e => e.stopPropagation()} data-testid={compact ? 'terminal-tile-editor' : 'terminal-editor'}></div>
  {#if !canView && t}<div class="access-needed"><p>Ask {name(t.owner_id)} to view this terminal.</p><button class="btn lit" onclick={() => request(false)}>Request access</button><button class="btn quiet" onclick={() => request(true)}>Request control</button></div>{/if}
  {#if hint}<span class="hint">Esc Esc to leave</span>{/if}
  {#if t && !t.ended_at && canView}<div class="view-status">{#if direct}<span class="direct">direct</span>{/if}{#if !controller}<span>View only</span><button onclick={() => request(true)}>Request control</button>{/if}</div>{/if}
  {#if t?.ended_at}<div class="replay" data-testid="terminal-replay"><span>Session ended</span><button aria-label={playing ? 'Pause replay' : 'Play replay'} onclick={() => {if (time >= duration) seek(0); playing = !playing}}>{playing ? 'Pause' : 'Play'}</button><input aria-label="Replay position" type="range" min="0" max={duration || 1} value={time} oninput={e => seek(+e.currentTarget.value)} /><span>{Math.floor(time / 1000)}s</span>{#if t.recording_capped}<span>Recording stopped at 64 MiB</span>{/if}</div>{/if}
  {#if owner && t && !compact && !t.ended_at}<div class="actions"><label>Post card <select aria-label="Post terminal card to" bind:value={shareTo}><option value="">Choose a room</option>{#each store.textChannels.filter(c => c.kind === 'text') as c}<option value={c.id}>#{c.name}</option>{/each}</select></label><button class="btn quiet" disabled={!shareTo} onclick={share}>Post</button><button class="btn quiet" onclick={end}>End session</button></div>{/if}
  {#if error}<div class="status" role="status">{error}<button aria-label="Dismiss terminal message" onclick={() => error = ''}>×</button></div>{/if}
</div>
<style>
  .terminal-view { position: absolute; inset: 0; display: flex; flex-direction: column; background: var(--bg); overflow: hidden; padding-top: 44px; }
  .terminal-screen { flex: 1; min-height: 0; min-width: 0; overflow: auto; position: relative; margin: 0 10px 24px; }
  .terminal-screen :global(canvas) { display: block; }
  .control-banner, .control-prompt { display: flex; align-items: center; gap: 8px; min-height: 28px; padding: 0 10px; font-size: 12px; background: var(--bg-2); border-left: 2px solid var(--lamp); flex-shrink: 0; }
  .control-banner span { flex: 1; } .control-banner button, .control-prompt button { color: var(--lamp); padding: 4px 6px; }
  .view-status { position: absolute; bottom: 8px; right: 12px; display: flex; align-items: center; gap: 10px; font: 10px var(--mono); color: var(--ink-2); } .view-status button { color: var(--ink); }
  .hint { position: absolute; bottom: 8px; left: 12px; font: 10px var(--mono); color: var(--ink-2); }
  .actions { display: flex; align-items: center; gap: 8px; padding: 4px 10px 28px; font-size: 11px; } .actions .btn { padding: 3px 6px; min-height: 24px; }
  select { font-size: 12px; max-width: 145px; padding: 2px; margin-left: 6px; }
  .access-needed { position: absolute; inset: 44px 0 0; display: flex; flex-wrap: wrap; align-content: center; justify-content: center; gap: 8px; background: var(--bg); font-size: 13px; } .access-needed p { width: 100%; text-align: center; }
  .status { position: absolute; top: 44px; left: 12px; max-width: calc(100% - 24px); border: 1px solid var(--line); padding: 8px 10px; background: var(--bg-2); font-size: 12px; } .status button { margin-left: 8px; }
  .replay { display: flex; align-items: center; gap: 8px; padding: 4px 10px 12px; font: 11px var(--mono); flex-wrap: wrap; } .replay input { flex: 1; min-width: 60px; }
  .compact { padding-top: 28px; } .compact .terminal-screen { margin: 0 4px 22px; }
</style>
