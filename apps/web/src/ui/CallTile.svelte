<script lang="ts">
  import { call, type CallParticipant } from '../lib/call.svelte'
  import type { Share } from '../lib/call-shares'
  import CallQuality from './CallQuality.svelte'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  let { participant, screen = false, share }: { participant: CallParticipant; screen?: boolean; share?: Share } = $props()
  let video = $state<HTMLVideoElement>()
  const track = $derived(screen ? share ? share.track : participant.screen : participant.camera)
  $effect(() => {
    const current = track, element = video
    if (current && element) {
      current.attach(element)
      return () => { current.detach(element); element.srcObject = null }
    }
  })
</script>

<div class="tile" class:screen class:speaking={participant.speaking} data-testid={screen ? 'screen-tile' : 'call-tile'} data-share-name={share?.name} data-local={participant.local} data-user-id={participant.userId} data-connection-id={participant.id}>
  {#if track}
    <video bind:this={video} class:mirror={participant.local && !screen && call.cameraSettings.mirror} autoplay playsinline muted aria-label={`${participant.name} ${screen ? 'screen' : 'camera'}`}></video>
  {:else}
    <Avatar userId={participant.userId} size={44} />
  {/if}
  <span class="label">
    {#if screen}<Icon name="screen" size={14} />{/if}
    <span title={share?.label}>{#if share}{share.label} · {/if}{participant.name}{#if participant.device}<small> · {participant.device}</small>{/if}</span>
    <CallQuality {participant} {track} />
    {#if share && participant.local}
      <button class="stop" aria-label={`Stop ${share.name}`} onclick={() => call.stopScreen(share.name)}>stop</button>
      <details class="quality-menu"><summary aria-label="Share quality" title="Share quality"><Icon name="more" size={14} /></summary><div>
        <button onclick={(e) => { void call.screenQuality(share.name, 'Smooth'); e.currentTarget.closest('details')?.removeAttribute('open') }}>Smooth</button>
        <button onclick={(e) => { void call.screenQuality(share.name, 'Sharp'); e.currentTarget.closest('details')?.removeAttribute('open') }}>Sharp</button>
      </div></details>
    {/if}
    {#if participant.muted}<span class="mute" data-testid="muted-mic" aria-label="Microphone muted"><Icon name="mic-off" size={14} /></span>{/if}
  </span>
</div>

<style>
  .tile { width: 216px; aspect-ratio: 16/9; flex: none; position: relative; display: grid; place-items: center; border-radius: var(--r-lg); background: var(--bg-3); overflow: hidden; transition: box-shadow 120ms; }
  .tile.screen { width: 384px; }
  .tile.speaking { box-shadow: 0 0 0 2px var(--lamp), 0 0 12px var(--lamp-glow); }
  video { width: 100%; height: 100%; min-height: 0; object-fit: cover; position: absolute; inset: 0; }
  .screen video { object-fit: contain; }
  .mirror { transform: scaleX(-1); }
  .label { position: absolute; left: 6px; bottom: 6px; max-width: calc(100% - 12px); display: flex; align-items: center; gap: 5px; padding: 4px 8px; border-radius: 6px; background: var(--overlay); font-size: 12px; color: var(--ink); }
  .label > span:first-of-type { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .stop { flex: none; font: 10px var(--mono); color: var(--ink-2); }
  .stop:hover { color: var(--ember); }
  .quality-menu { flex: none; }
  summary { cursor: pointer; list-style: none; display: grid; place-items: center; width: 20px; height: 20px; }
  summary::-webkit-details-marker { display: none; }
  .quality-menu > div { position: absolute; right: 0; bottom: 32px; padding: 4px; border: 1px solid var(--line); border-radius: 6px; background: var(--bg-2); z-index: 4; }
  .quality-menu button { display: block; padding: 6px 12px; font: 11px var(--mono); }
  .quality-menu button:hover { background: var(--bg-3); }
  .mute { display: grid; flex: none; }
</style>
