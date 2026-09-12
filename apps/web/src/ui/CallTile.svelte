<script lang="ts">
  import { type CallParticipant } from '../lib/call.svelte'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  let { participant, screen = false }: { participant: CallParticipant; screen?: boolean } = $props()
  let video = $state<HTMLVideoElement>()
  const track = $derived(screen ? participant.screen : participant.camera)
  $effect(() => {
    const current = track, element = video
    if (current && element) {
      current.attach(element)
      return () => { current.detach(element); element.srcObject = null }
    }
  })
</script>

<div class="tile" class:screen class:speaking={participant.speaking} data-testid={screen ? 'screen-tile' : 'call-tile'} data-user-id={participant.id}>
  {#if track}
    <video bind:this={video} class:mirror={participant.local && !screen} autoplay playsinline muted aria-label={`${participant.name} ${screen ? 'screen' : 'camera'}`}></video>
  {:else}
    <Avatar userId={participant.id} size={44} />
  {/if}
  <span class="label">
    {#if screen}<Icon name="screen" size={14} />{/if}
    <span>{participant.name}</span>
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
  .label { position: absolute; left: 6px; bottom: 6px; max-width: calc(100% - 12px); display: flex; align-items: center; gap: 5px; padding: 4px 8px; border-radius: 6px; background: rgba(0,0,0,.45); font-size: 12px; color: var(--ink); }
  .label > span:first-of-type { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .mute { display: grid; flex: none; }
</style>
