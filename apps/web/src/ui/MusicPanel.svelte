<script lang="ts">
  import { call } from '../lib/call.svelte'
  import type { Store } from '../lib/store.svelte'
  import type { MusicQueue } from '../lib/types'
  import MusicComposer from './MusicComposer.svelte'
  import ParticipantVolume from './ParticipantVolume.svelte'
  import Icon from './Icon.svelte'
  let { roomId, owner }: { roomId: string; owner: Store } = $props()
  let clock = $state(Date.now()), busy = $state(false), error = $state(''), dragged = $state('')
  const q = $derived(owner.music.get(roomId))
  const current = $derived(q?.queue.find(t => ['loading', 'playing', 'paused'].includes(t.state)))
  const pending = $derived(q?.queue.filter(t => t.id !== current?.id) || [])
  const position = $derived(Math.min(current?.duration || Infinity, (q?.position_seconds || 0) + (current?.state === 'playing' && !q?.paused ? Math.max(0, clock - (owner.musicReceivedAt.get(roomId) || clock)) / 1000 : 0)))
  const time = (seconds: number) => `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, '0')}`
  $effect(() => { void owner.loadMusic(roomId).catch(e => error = e.message) })
  $effect(() => { const id = setInterval(() => clock = Date.now(), 250); return () => clearInterval(id) })
  async function change(path: string, body: unknown = {}, method: 'post' | 'put' | 'del' = 'post') {
    if (busy) return
    busy = true; error = ''
    try {
      if (method === 'del') { await owner.api.del(`/rooms/${roomId}/music${path}`); await owner.loadMusic(roomId) }
      else owner.receiveMusic(await owner.api[method]<MusicQueue>(`/rooms/${roomId}/music${path}`, body))
    } catch (e) { error = e instanceof Error ? e.message : 'Could not update the queue.' }
    finally { busy = false }
  }
  function move(id: string, target: number) {
    const ids = pending.map(t => t.id), at = ids.indexOf(id)
    if (at < 0 || target < 0 || target >= ids.length) return
    ids.splice(at, 1); ids.splice(target, 0, id)
    void change('/queue/order', { ids }, 'put')
  }
</script>
<section class="music-panel" aria-label="Music queue" data-testid="music-panel">
  <header><Icon name="music" size={20} /><div><h2>{owner.settings.instance_name} DJ</h2><p>One queue for everyone in {owner.channel(roomId)?.name || 'the call'}.</p></div></header>
  {#if q?.paused_for_jam}<p class="for-jam" role="status">Queue paused for the Jam. It picks up where it left off when the Jam ends.</p>{/if}
  <div class="now">
    <div class="art">{#if current?.thumbnail}<img src={current.thumbnail} alt="" referrerpolicy="no-referrer" />{:else}<Icon name="music" size={32} />{/if}</div>
    <div class="now-copy"><span class="eyebrow">{current?.state === 'loading' ? 'Loading' : q?.paused_for_jam ? 'Paused for the Jam' : q?.paused ? 'Paused' : 'Now playing'}</span><h3>{current?.title || 'Pick the first track'}</h3><p>{current ? `Added by ${owner.name(current.added_by)}` : 'Paste a YouTube link below.'}</p></div>
  </div>
  {#if current}
    <label class="seek">Playback position<input aria-label="Playback position" type="range" min="0" max={Math.max(0, (current.duration || 1) - 1)} step="1" value={position} disabled={busy || !current.duration || current.state === 'loading'} onchange={e => change('/seek', { position_seconds: +e.currentTarget.value })} /></label>
    <div class="times"><span>{time(position)}</span><span>{current.duration ? time(current.duration) : '—'}</span></div>
  {/if}
  <div class="playback"><button class="btn" disabled={busy} onclick={() => change('/pause', { paused: !q?.paused })}>{q?.paused ? 'Play' : 'Pause'}</button><button class="btn" disabled={busy || !q?.queue.some(t => t.state !== 'failed')} onclick={() => change('/skip')}>Skip track</button></div>
  <div class="queue-head"><h3>Up next</h3><span>{pending.length}</span></div>
  <ol aria-label="Queued tracks">
    {#each pending as track, i (track.id)}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions (Native buttons provide the same reorder actions without dragging.) -->
      <li draggable={!busy} ondragstart={e => { dragged = track.id; e.dataTransfer?.setData('text/plain', track.id) }} ondragend={() => dragged = ''} ondragover={e => e.preventDefault()} ondrop={e => { e.preventDefault(); move(dragged, i); dragged = '' }} data-track-id={track.id}>
        <span class="position" aria-hidden="true">{i + 1}</span><div class="track"><span class:failed={track.state === 'failed'}>{track.title}</span><small>{owner.name(track.added_by)}{track.duration ? ` · ${time(track.duration)}` : ''}</small></div>
        <div class="row-actions"><button disabled={busy || i === 0} aria-label={`Move ${track.title} up`} onclick={() => move(track.id, i - 1)}>↑</button><button disabled={busy || i === pending.length - 1} aria-label={`Move ${track.title} down`} onclick={() => move(track.id, i + 1)}>↓</button><button disabled={busy} aria-label={`Remove ${track.title}`} onclick={() => change(`/queue/${track.id}`, {}, 'del')}><Icon name="x" size={14} /></button></div>
      </li>
    {/each}
  </ol>
  {#if !pending.length}<p class="empty">The next song is up to you.</p>{/if}
  <MusicComposer {roomId} {owner} />
  <div class="listener"><details><summary>Your listening volume</summary>{#if q}<ParticipantVolume userId={q.participant_id} name={`${owner.settings.instance_name} DJ`} />{/if}</details><label class="duck"><input type="checkbox" checked={call.prefs.musicDucking} onchange={e => call.save({ musicDucking: e.currentTarget.checked })} /> Lower music while people talk</label></div>
  <p class="error" role="status">{error}</p>
</section>
<style>
  .music-panel { width: min(380px, calc(100vw - 24px)); max-height: min(720px, calc(100dvh - 32px)); overflow: auto; overscroll-behavior: contain; padding: 18px; color: var(--ink); }
  header { display: flex; align-items: center; gap: 10px; margin-bottom: 18px; }
  h2 { font-size: 18px; margin: 0; } h3 { font-size: 14px; margin: 0; } p { margin: 4px 0 0; font-size: 12px; color: var(--ink-2); }
  .for-jam { margin: -4px 0 14px; padding: 8px 10px; border-radius: var(--r); background: color-mix(in srgb, var(--lamp) 12%, var(--bg-3)); color: var(--ink); font-size: 12px; }
  .now { display: flex; align-items: center; gap: 12px; } .art { width: 68px; height: 68px; flex: none; display: grid; place-items: center; background: var(--bg-3); border-radius: var(--r); overflow: hidden; color: var(--lamp); }
  img { width: 100%; height: 100%; object-fit: cover; } .now-copy { min-width: 0; } .now-copy h3 { margin-top: 4px; overflow-wrap: anywhere; } .eyebrow { font-size: 10px; }
  .seek { display: block; margin-top: 14px; font-size: 0; } .seek input { width: 100%; min-height: 24px; accent-color: var(--lamp); }
  .times { display: flex; justify-content: space-between; font: 11px var(--mono); color: var(--ink-2); font-variant-numeric: tabular-nums; }
  .playback { display: flex; gap: 8px; margin: 12px 0 20px; } .playback button { min-height: 40px; flex: 1; justify-content: center; }
  .queue-head { display: flex; justify-content: space-between; align-items: center; color: var(--ink-2); font-size: 12px; }
  ol { padding: 0; margin: 8px 0 16px; list-style: none; } li { display: flex; align-items: center; gap: 8px; padding: 9px 0; border-bottom: 1px solid var(--line); }
  .position { width: 14px; flex: none; font: 11px var(--mono); color: var(--ink-3); } .track { min-width: 0; flex: 1; display: grid; gap: 3px; } .track > span { font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } small { font-size: 11px; color: var(--ink-2); }
  .row-actions { display: flex; gap: 2px; } .row-actions button { display: grid; place-items: center; width: 26px; height: 32px; border-radius: var(--r); color: var(--ink-2); } button:disabled { opacity: .4; } .failed, .error { color: var(--danger); }
  .empty { margin-bottom: 16px; } .listener { border-top: 1px solid var(--line); margin-top: 16px; padding-top: 12px; } summary { cursor: pointer; min-height: 28px; font-size: 12px; } .listener :global(.volume-control) { width: 100%; }
  .duck { display: flex; align-items: center; gap: 8px; min-height: 40px; font-size: 12px; color: var(--ink-2); } .duck input { accent-color: var(--lamp); } .error:empty { display: none; }
  @media (hover: hover) { .row-actions button:hover { background: var(--bg-3); color: var(--ink); } }
</style>
