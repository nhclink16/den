<script lang="ts">
  import { call, type CallParticipant } from '../lib/call.svelte'
  import type { Share } from '../lib/call-shares'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import ParticipantVolume from './ParticipantVolume.svelte'
  import CallQuality from './CallQuality.svelte'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import { shell } from '../lib/shell-call'
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
    <span class="face"><Avatar userId={participant.userId} size={72} instance={call.owner ?? undefined} presence={false} /></span>
    {#if participant.unseen}<span class="unseen">{participant.unseen === 'screen' ? 'Sharing their screen' : 'Camera on'}<span class="soon"> · video coming soon on iPhone</span></span>{/if}
  {/if}
  <div class="label">
    {#if screen}<Icon name={share?.surface || 'monitor'} size={14} />{:else if participant.speaking}<span class="talk-bars" aria-hidden="true"><i></i><i></i><i></i></span>{/if}
    <span title={share?.label}>{#if share}{share.label}{' · '}{/if}{participant.name}{#if participant.device}<small> · {participant.device}</small>{/if}</span>
    <CallQuality {participant} {track} />
    {#if participant.muted}<span class="mute" data-testid="muted-mic" aria-label="Microphone muted"><Icon name="mic-off" size={14} /></span>{/if}
  </div>
  <!-- Only when it has something in it: an empty one drew a stray dot of glass in the corner. -->
  {#if (share && participant.local) || (!participant.local && !shell)}
  <div class="tile-controls">
    {#if share && participant.local}
      <button class="stop" aria-label={`Stop sharing ${share.label}`} title={`Stop sharing ${share.label}`} onclick={() => call.stopScreen(share.name)}>Stop</button>
      <details class="quality-menu"><summary aria-label="Share quality" title="Share quality"><Icon name="more" size={14} /></summary><div class="volume-panel quality-panel" popover="auto" use:disclosurePopover>
        <button onclick={(e) => { void call.screenQuality(share.name, 'Smooth'); e.currentTarget.closest('details')?.removeAttribute('open') }}>Smooth</button>
        <button onclick={(e) => { void call.screenQuality(share.name, 'Sharp'); e.currentTarget.closest('details')?.removeAttribute('open') }}>Sharp</button>
      </div></details>
    {/if}
    {#if !participant.local && !shell}
      {#if call.volume(participant.userId) === 0}<span class="mute" role="img" aria-label="Muted for you"><Icon name="sound-off" size={14} /></span>{/if}
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes the disclosure) -->
      <details class="volume-menu" onkeydown={(e) => { if (e.key === 'Escape') { e.preventDefault(); e.currentTarget.open = false; e.currentTarget.querySelector('summary')?.focus() } }}>
        <summary aria-label={`${participant.name} audio options`} title={`${participant.name} audio options`}><Icon name="more" size={14} /></summary>
        <div class="volume-panel" popover="auto" use:disclosurePopover><ParticipantVolume userId={participant.userId} name={participant.name} /></div>
      </details>
    {/if}
  </div>
  {/if}
</div>

<style>
  .tile { container-type: inline-size; width: 216px; aspect-ratio: 16/9; flex: none; position: relative; display: grid; place-items: center; border-radius: var(--r-tile); background: var(--bg-3); overflow: hidden; }
  .tile.screen { width: 384px; }
  /* Who's talking: a ring drawn inside the tile, so neighbours and the strip's
     edge never clip it. */
  .tile::after { content: ''; position: absolute; inset: 0; z-index: 2; border-radius: inherit; pointer-events: none; box-shadow: inset 0 0 0 3px transparent; transition: box-shadow var(--t); }
  .tile.speaking::after { box-shadow: inset 0 0 0 3px var(--lamp); }
  video { width: 100%; height: 100%; min-height: 0; object-fit: cover; position: absolute; inset: 0; }
  .screen video { object-fit: contain; }
  .mirror { transform: scaleX(-1); }
  .face { display: flex; margin-bottom: 14px; }
  @container (max-width: 300px) { .face { transform: scale(.66); margin-bottom: 0; } }
  @container (max-width: 160px) { .face { transform: scale(.5); } }
  /* Clear of the layout pin and the corner controls. A small tile says only what is on. */
  .unseen { position: absolute; top: 10px; inset-inline: 44px; text-align: center; font-size: var(--fs-xs); color: var(--ink-2); }
  @container (max-width: 300px) { .soon { display: none; } }
  /* The name, on frosted glass so it reads on any camera or share. */
  .label { position: absolute; z-index: 3; left: 8px; bottom: 8px; max-width: calc(100% - 16px); display: flex; align-items: center; gap: 6px; min-height: 26px; padding: 3px 10px; border-radius: var(--r-pill); background: var(--glass); backdrop-filter: blur(10px); font-size: var(--fs-sm); font-weight: 600; color: var(--ink); }
  @container (max-width: 260px) { .label { left: 6px; bottom: 6px; min-height: 22px; padding: 2px 8px; font-size: var(--fs-xs); gap: 4px; } .label :global(.sent) { display: none; } }
  .label > span:not(.talk-bars):first-of-type, .label > span.talk-bars + span { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .label small { font-weight: 400; color: var(--ink-2); }
  .tile-controls { position: absolute; z-index: 3; top: 8px; right: 8px; display: flex; align-items: center; gap: 4px; padding: 2px; border-radius: var(--r-pill); background: var(--glass); backdrop-filter: blur(10px); }
  .stop { padding: 0 10px; min-height: 26px; flex: none; border-radius: var(--r-pill); font: 600 var(--fs-xs) var(--body); color: var(--ink); }
  @media (hover: hover) { .stop:hover { color: var(--ember); } }
  .volume-menu { flex: none; }
  .volume-panel { position: fixed; margin: 0; padding: 0; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-2); box-shadow: var(--lift); }
  .quality-menu { flex: none; }
  summary { cursor: pointer; list-style: none; display: grid; place-items: center; width: 26px; height: 26px; border-radius: var(--r-pill); }
  @media (hover: hover) { summary:hover { background: color-mix(in srgb, var(--ink) 12%, transparent); } }
  summary::-webkit-details-marker { display: none; }
  .quality-panel { padding: 4px; }
  .quality-menu button { display: block; width: 100%; padding: 8px 12px; border-radius: var(--r); text-align: left; font: var(--fs-sm) var(--body); }
  @media (hover: hover) { .quality-menu button:hover { background: var(--hover); } }
  /* Muted is the call state people most need to read at a glance. */
  .label .mute { display: grid; flex: none; color: var(--danger); }
  .tile-controls .mute { display: grid; flex: none; padding-inline: 4px; color: var(--danger); }
</style>
