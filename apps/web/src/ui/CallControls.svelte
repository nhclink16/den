<script lang="ts">
  import MusicQueueButton from './MusicQueueButton.svelte'
  import { desktop } from '../lib/desktop.svelte'
  import { call } from '../lib/call.svelte'
  import { blurSupported } from '../lib/av'
  import { callLayouts } from '../lib/call-layout.svelte'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import { shell } from '../lib/shell-call'
  import Icon from './Icon.svelte'
  // `bar` floats under the open call with every button. `dock` is the same set,
  // smaller, for the call bar above the sidebar, whose Leave sits on the bar's top
  // row instead (CallPill). `pill` is what you need while you chat: your mic, your
  // sound when it's off, and Leave.
  let { variant = 'dock' }: { variant?: 'bar' | 'dock' | 'pill' } = $props()
  const pill = $derived(variant === 'pill')
  // A dock icon cannot carry a reason, so hide it where blur cannot run. Settings
  // shows the disabled control and says why.
  const blurAvailable = blurSupported()
  // Phones have no screen to share: the button would only ever show an error.
  const shareAvailable = !!navigator.mediaDevices?.getDisplayMedia && !shell
  const shares = $derived(call.participants.find(p => p.local)?.screens || [])
  const icon = $derived(variant === 'bar' ? 22 : 18)
</script>

<div class="controls v-{variant}">
  {#if pill && call.outputMuted}<button class="off" aria-label="Play sound on this device" aria-pressed={false} title="Sound on this device" onclick={() => call.toggleOutput()}><Icon name="sound-off" size={icon} /></button>{/if}
  {#if call.prefs.mode === 'ptt'}
    <button class="ptt" class:held={call.held} aria-label="Push to talk" aria-pressed={call.held} title={`Hold ${call.prefs.pttLabel} to talk`}
      onpointerdown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); call.setHeld(true) }}
      onpointerup={() => call.setHeld(false)} onpointercancel={() => call.setHeld(false)} onlostpointercapture={() => call.setHeld(false)}
      onkeydown={(e) => { if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); call.setHeld(true) } }}
      onkeyup={(e) => { if (e.key === ' ' || e.key === 'Enter') call.setHeld(false) }} onblur={() => call.setHeld(false)}
    >{#if variant !== 'bar'}<Icon name="mic" size={icon} />{:else}<span class="ptt-icon"><Icon name="mic" size={icon} /></span><span class="ptt-label">hold {call.prefs.pttLabel}{#if desktop.global}<small> global</small>{/if} to talk</span>{/if}</button>
  {:else}
    <button class:off={!call.micOn} aria-label={call.micOn ? 'Mute microphone' : 'Unmute microphone'} aria-pressed={call.micOn} title="Microphone (M)" disabled={!!call.joining} onclick={() => call.toggleMic()}><Icon name={call.micOn ? 'mic' : 'mic-off'} size={icon} /></button>
  {/if}
  {#if !pill}
  <button class:off={call.outputMuted} aria-label={call.outputMuted ? 'Play sound on this device' : 'Mute sound on this device'} aria-pressed={!call.outputMuted} title="Sound on this device" onclick={() => call.toggleOutput()}><Icon name={call.outputMuted ? 'sound-off' : 'sound'} size={icon} /></button>
  <!-- Den Shell runs calls voice-only for now. -->
  {#if !shell}<button class:off={!call.cameraOn} aria-label={call.cameraOn ? 'Turn camera off' : 'Turn camera on'} aria-pressed={call.cameraOn} title="Camera (V)" disabled={!!call.joining || (call.backgroundBusy && !call.cameraOn)} onclick={() => call.toggleCamera()}><Icon name={call.cameraOn ? 'camera' : 'camera-off'} size={icon} /></button>{/if}
  {#if call.cameraOn && blurAvailable}<button class:on={call.cameraSettings.background !== 'none'} aria-label={call.cameraSettings.background !== 'none' ? 'Turn background blur off' : 'Blur my background'} aria-pressed={call.cameraSettings.background !== 'none'} title="Background blur" disabled={call.backgroundBusy} onclick={() => call.toggleBackground()}><Icon name="blur" size={icon} /></button>{/if}
  {#if shareAvailable}<button class:on={call.screenOn} aria-label={call.screenOn ? 'Stop sharing screen' : 'Share screen'} aria-pressed={call.screenOn} title="Screen share (S)" disabled={!!call.joining} onclick={() => call.toggleScreen()}><Icon name="screen" size={icon} /></button>{/if}
  {#if call.screenOn}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes the disclosure) -->
    <details class="overflow shares" onkeydown={(e) => { if (e.key === 'Escape') { e.preventDefault(); e.currentTarget.open = false; e.currentTarget.querySelector('summary')?.focus() } }}>
      <summary aria-label="Your shares" title="Your shares"><Icon name="more" size={icon} /></summary>
      <div popover="auto" use:disclosurePopover>
        <!-- Lives in this menu rather than beside it, so a sidebar-wide dock keeps one row. -->
        <button disabled={call.screenAdding || shares.length === 3} aria-label="Share another" title={shares.length === 3 ? 'Up to 3 at once' : 'Share another'} onclick={(e) => { e.currentTarget.closest('details')?.removeAttribute('open'); void call.addScreen() }}><Icon name="plus" size={14} /><span>{shares.length === 3 ? 'Up to 3 at once' : 'Share another'}</span></button>
        {#each shares as share (share.name)}
          <button aria-label={`Stop sharing ${share.label}`} title={`Stop sharing ${share.label}`} onclick={() => call.stopScreen(share.name)}><Icon name={share.surface} size={14} /><span>{share.label}</span><span class="stop-label">Stop</span></button>
        {/each}
      </div>
    </details>
  {/if}
  {#if variant === 'bar'}<details class="overflow options"><summary aria-label="Call options" title="Call options"><Icon name="more" size={icon} /></summary><div><button onclick={(e) => { callLayouts.reset(call.channel?.id); e.currentTarget.closest('details')?.removeAttribute('open') }}>Reset layout</button></div></details>{/if}
  <MusicQueueButton />
  {/if}
  {#if variant !== 'dock'}<button class="leave" aria-label="Leave call" title="Leave call" onclick={() => call.leave()}><Icon name="leave" size={icon} /></button>{/if}
</div>

<style>
  /* Round buttons, like FaceTime. Each size is --cb so the Music button, which
     lives in its own component, matches. Sharp corners square them off. */
  .controls { --cb: 34px; display: flex; align-items: center; gap: 6px; flex: none; }
  .v-bar { --cb: 48px; gap: 10px; }
  .v-pill { --cb: 40px; gap: 8px; }
  @media (pointer: coarse) { .controls { --cb: 44px; gap: 8px; } .v-bar { --cb: 48px; } }
  /* A phone's bar fits seven buttons on one row: mic, sound, camera, blur, options, music, Leave. */
  @media (max-width: 600px) { .v-bar { --cb: 44px; gap: 6px; } .v-bar > .leave { margin-inline-start: 8px; } }
  /* Only this row's own buttons and disclosures: the Music menu's popover has
     summaries of its own that must keep their text layout. */
  .controls > button, .controls > :global(details > summary) {
    display: grid; place-items: center; flex: none; width: var(--cb); height: var(--cb); border-radius: var(--r-pill);
    color: var(--ink); background: color-mix(in srgb, var(--ink) 11%, transparent); cursor: pointer;
    transition: background-color var(--t-fast), color var(--t-fast), filter var(--t-fast);
  }
  /* Plain buttons only: a solid state (off, on, held, Leave) keeps its fill under the pointer. */
  @media (hover: hover) { .controls > button:not(.off, .on, .held, .leave):hover:not(:disabled), .controls > :global(details > summary:not(.live):hover) { background: color-mix(in srgb, var(--ink) 18%, transparent); } }
  .controls > button:disabled { opacity: .5; cursor: default; }
  /* Off is a state you need to see at a glance: the button turns solid, like
     FaceTime's, rather than dimming. On (blur, a share) takes the accent. */
  .controls > button.off, .controls > button.off:hover { background: var(--ink); color: var(--bg); }
  .controls > button.on, .controls > button.on:hover { background: var(--lamp); color: var(--on-lamp); }
  /* Leave is red and sits apart from the toggles, so it is never fumbled mid-call. */
  .controls > .leave { margin-inline-start: 10px; background: var(--danger); color: var(--bg); }
  @media (hover: hover) { .controls > .leave:hover { background: var(--danger); filter: brightness(1.1); } }
  .v-bar > .leave { margin-inline-start: 14px; }
  .v-pill > .leave { margin-inline-start: 0; }
  /* The open call's bar and the sidebar's row stay one row each. In a narrow call
     (a small phone, or a window with both side panels open) or with every toggle
     showing in the sidebar, the round buttons shrink together rather than wrap. */
  .v-bar, .v-dock { min-width: 0; }
  .v-bar > button:not(.ptt), .v-dock > button, .v-bar > :global(details), .v-dock > :global(details) { flex: 0 1 auto; width: var(--cb); min-width: 28px; }
  .v-bar > button:not(.ptt), .v-dock > button { height: auto; aspect-ratio: 1; }
  .v-bar > :global(details > summary), .v-dock > :global(details > summary) { width: 100%; height: auto; aspect-ratio: 1; }
  .v-bar > .shares, .v-dock > .shares { width: calc(var(--cb) * .6); min-width: 17px; }
  /* Push to talk's words don't fit a narrow open call: it becomes a mic like the rest. */
  .ptt-icon { display: none; }
  @container callview (max-width: 480px) {
    .controls.v-bar > .ptt { flex: 0 1 auto; width: var(--cb); min-width: 28px; height: auto; aspect-ratio: 1; padding: 0; }
    .v-bar > .ptt > .ptt-label { display: none; }
    .v-bar > .ptt > .ptt-icon { display: grid; }
  }
  .v-bar > .shares > summary, .v-dock > .shares > summary { width: 100%; aspect-ratio: 3 / 5; }
  .controls > .ptt { width: auto; padding: 0 14px; white-space: nowrap; font: 600 var(--fs-xs) var(--body); touch-action: none; }
  .v-pill > .ptt, .v-dock > .ptt { width: var(--cb); padding: 0; }
  .controls > .ptt.held { color: var(--on-lamp); background: var(--lamp); }
  /* A Jam is playing: the Music button lights like any other thing that is on. */
  .controls > :global(details > summary.live) { color: var(--on-lamp); background: var(--lamp); }
  .overflow { position: relative; flex: none; }
  .overflow > summary { list-style: none; }
  .overflow > summary::-webkit-details-marker { display: none; }
  .options > div { position: absolute; bottom: calc(var(--cb) + 14px); right: 0; padding: 4px; border-radius: var(--r-lg); border: 1px solid var(--line); background: var(--bg-2); box-shadow: var(--lift); }
  .options > div button { width: max-content; padding: 8px 12px; border-radius: var(--r); font: var(--fs-sm) var(--body); }
  @media (hover: hover) { .options > div button:hover { background: var(--hover); } }
  /* Your shares hide behind a smaller button beside Share. */
  .controls > .shares > summary { width: calc(var(--cb) * .6); }
  .shares > div { position: fixed; inset: auto; margin: 0; z-index: 20; width: min(280px, calc(100vw - 24px)); padding: 4px; border-radius: var(--r-lg); border: 1px solid var(--line); background: var(--bg-2); box-shadow: var(--lift); }
  .shares > div button { display: flex; align-items: center; gap: 8px; width: 100%; min-height: 40px; padding: 0 12px; border-radius: var(--r); text-align: left; font: var(--fs-sm) var(--body); }
  @media (hover: hover) { .shares > div button:hover { background: var(--hover); } }
  .shares button > span:first-of-type { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .stop-label { color: var(--ember); }
</style>
