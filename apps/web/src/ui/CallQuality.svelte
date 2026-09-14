<script lang="ts">
  import { LocalVideoTrack, type Track } from 'livekit-client'
  import type { CallParticipant } from '../lib/call.svelte'
  let { participant, track }: { participant: CallParticipant; track?: Track } = $props()
  let sent = $state(''), limitation = $state('')
  const bars = $derived(participant.quality === 'excellent' ? 3 : participant.quality === 'good' ? 2 : participant.quality === 'poor' ? 1 : 0)
  $effect(() => {
    const video = track
    sent = ''; limitation = ''
    if (!participant.local || !(video instanceof LocalVideoTrack)) return
    let alive = true, busy = false
    let previous = new Map<string, number>()
    async function update() {
      if (busy) return
      busy = true
      try {
        const stats = await (video as LocalVideoTrack).getSenderStats()
        if (!alive) return
        const active = stats.filter(s => (s.framesSent ?? 0) > (previous.get(s.streamId || s.rid || '') ?? 0))
        previous = new Map(stats.map(s => [s.streamId || s.rid || '', s.framesSent ?? 0]))
        const layer = active[0]
        sent = layer?.frameHeight ? `${layer.frameHeight}p · ${Math.round(layer.framesPerSecond ?? 0)}` : '—'
        limitation = layer?.qualityLimitationReason || ''
      } catch { if (alive) sent = '—' } finally { busy = false }
    }
    void update(); const timer = setInterval(update, 2000)
    return () => { alive = false; clearInterval(timer) }
  })
</script>

<span class="quality" class:poor={bars === 1} data-testid="connection-quality" aria-label={`Connection: ${participant.quality}`} title={bars === 1 ? participant.local ? 'Your connection is struggling' : 'Their connection is struggling' : `Connection: ${participant.quality}`}>
  {#each [1, 2, 3] as n}<i class:lit={bars >= n} style:height={`${n * 3}px`}></i>{/each}
</span>
{#if participant.local && track}
  <span class="sent" data-testid="sent-quality" title={limitation === 'bandwidth' ? 'Limited by your upload' : limitation === 'cpu' ? 'Limited by your CPU' : 'Sending'}>
    {#if limitation === 'bandwidth' || limitation === 'cpu'}<i class="limited"></i>{/if}{sent || '—'}
  </span>
{/if}

<style>
  .quality { display: inline-flex; align-items: end; gap: 2px; height: 10px; flex: none; }
  .quality i { width: 3px; border-radius: 1px; background: var(--ink-3); opacity: .25; }
  .quality .lit { background: var(--ink-2); opacity: 1; }
  .quality.poor .lit { background: var(--ember); }
  .sent { display: inline-flex; align-items: center; gap: 4px; white-space: nowrap; font: 10px var(--mono); color: var(--ink-2); }
  .limited { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); }
</style>
