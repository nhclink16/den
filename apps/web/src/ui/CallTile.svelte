<script lang="ts">
  import { call, type CallParticipant } from '../lib/call.svelte'
  import type { Share } from '../lib/call-shares'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import ParticipantVolume from './ParticipantVolume.svelte'
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
  {:else if participant.music}
    <Icon name="music" size={32} />
  {:else}
    <Avatar userId={participant.userId} size={44} />
  {/if}
  <div class="label">
    {#if screen}<Icon name={share?.surface || 'monitor'} size={14} />{/if}
    <span title={share?.label}>{#if share}{share.label} · {/if}{participant.name}{#if participant.device}<small> · {participant.device}</small>{/if}</span>
    <CallQuality {participant} {track} />
    {#if participant.muted}<span class="mute" data-testid="muted-mic" aria-label="Microphone muted"><Icon name="mic-off" size={14} /></span>{/if}
  </div>
  <div class="tile-controls">
    {#if share && participant.local}
      <button class="stop" aria-label={`Stop sharing ${share.label}`} title={`Stop sharing ${share.label}`} onclick={() => call.stopScreen(share.name)}>Stop</button>
      <details class="quality-menu"><summary aria-label="Share quality" title="Share quality"><Icon name="more" size={14} /></summary><div class="volume-panel quality-panel" popover="auto" use:disclosurePopover>
        <button onclick={(e) => { void call.screenQuality(share.name, 'Smooth'); e.currentTarget.closest('details')?.removeAttribute('open') }}>Smooth</button>
        <button onclick={(e) => { void call.screenQuality(share.name, 'Sharp'); e.currentTarget.closest('details')?.removeAttribute('open') }}>Sharp</button>
      </div></details>
    {/if}
    {#if !participant.local}
      {#if call.volume(participant.userId) === 0}<span class="mute" role="img" aria-label="Muted for you"><Icon name="sound-off" size={14} /></span>{/if}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes the disclosure) -->
      <details class="volume-menu" onkeydown={(e) => { if (e.key === 'Escape') { e.preventDefault(); e.currentTarget.open = false; e.currentTarget.querySelector('summary')?.focus() } }}>
        <summary aria-label={`${participant.name} audio options`} title={`${participant.name} audio options`}><Icon name="more" size={14} /></summary>
        <div class="volume-panel" popover="auto" use:disclosurePopover><ParticipantVolume userId={participant.userId} name={participant.name} /></div>
      </details>
    {/if}
  </div>
</div>

<style>
  .tile { container-type: inline-size; width: 216px; aspect-ratio: 16/9; flex: none; position: relative; display: grid; place-items: center; border-radius: var(--r-lg); background: var(--bg-3); overflow: hidden; transition: box-shadow 120ms; }
  .tile.screen { width: 384px; }
  .tile.speaking { box-shadow: 0 0 0 2px var(--lamp), 0 0 12px var(--lamp-glow); }
  video { width: 100%; height: 100%; min-height: 0; object-fit: cover; position: absolute; inset: 0; }
  .screen video { object-fit: contain; }
  .mirror { transform: scaleX(-1); }
  .label { position: absolute; left: 6px; bottom: 6px; max-width: calc(100% - 12px); display: flex; align-items: center; gap: 5px; padding: 4px 8px; border-radius: 6px; background: var(--overlay); font-size: 12px; color: var(--ink); }
  .label > span:first-of-type { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .tile-controls { position: absolute; top: 6px; right: 6px; display: flex; align-items: center; gap: 4px; padding: 2px 4px; border-radius: 6px; background: var(--overlay); }
  .tile-controls:empty { display: none; }
  .stop { padding: 0 6px; min-height: 24px; flex: none; font: 10px var(--mono); color: var(--ink-2); }
  .stop:hover { color: var(--ember); }
  .volume-menu { flex: none; }
  .volume-panel { position: fixed; margin: 0; padding: 0; border: 1px solid var(--line); border-radius: var(--r); background: var(--bg-2); box-shadow: 0 8px 24px var(--shadow); }
  .quality-menu { flex: none; }
  summary { cursor: pointer; list-style: none; display: grid; place-items: center; width: 24px; height: 24px; }
  summary::-webkit-details-marker { display: none; }
  .quality-panel { padding: 4px; }
  @container (max-width: 260px) { .label :global(.sent) { display: none; } }
  .quality-menu button { display: block; padding: 6px 12px; font: 11px var(--mono); }
  .quality-menu button:hover { background: var(--bg-3); }
  .mute { display: grid; flex: none; }
</style>
