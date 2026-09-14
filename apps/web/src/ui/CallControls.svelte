<script lang="ts">
  import { desktop } from '../lib/desktop.svelte'
  import { call } from '../lib/call.svelte'
  import { callLayouts } from '../lib/call-layout.svelte'
  import Icon from './Icon.svelte'
  let { large = false }: { large?: boolean } = $props()
</script>

<div class="controls" class:large>
  {#if call.prefs.mode === 'ptt'}
    <button class="ptt" class:held={call.held} aria-label="Push to talk" aria-pressed={call.held}
      onpointerdown={(e) => { e.currentTarget.setPointerCapture(e.pointerId); call.setHeld(true) }}
      onpointerup={() => call.setHeld(false)} onpointercancel={() => call.setHeld(false)} onlostpointercapture={() => call.setHeld(false)}
      onkeydown={(e) => { if (e.key === ' ' || e.key === 'Enter') { e.preventDefault(); call.setHeld(true) } }}
      onkeyup={(e) => { if (e.key === ' ' || e.key === 'Enter') call.setHeld(false) }} onblur={() => call.setHeld(false)}
    >hold {call.prefs.pttLabel}{#if desktop.global}<small class="mono"> global</small>{/if} to talk</button>
  {:else}
    <button class:muted={!call.micOn} aria-label={call.micOn ? 'Mute microphone' : 'Unmute microphone'} aria-pressed={call.micOn} title="Microphone (M)" disabled={!!call.joining} onclick={() => call.toggleMic()}><Icon name={call.micOn ? 'mic' : 'mic-off'} /></button>
  {/if}
  <button class:muted={call.outputMuted} aria-label={call.outputMuted ? 'Play sound on this device' : 'Mute sound on this device'} aria-pressed={!call.outputMuted} title="Sound on this device" onclick={() => call.toggleOutput()}><Icon name={call.outputMuted ? 'sound-off' : 'sound'} /></button>
  <button class:muted={!call.cameraOn} aria-label={call.cameraOn ? 'Turn camera off' : 'Turn camera on'} aria-pressed={call.cameraOn} title="Camera (V)" disabled={!!call.joining} onclick={() => call.toggleCamera()}><Icon name={call.cameraOn ? 'camera' : 'camera-off'} /></button>
  <button class:sharing={call.screenOn} aria-label={call.screenOn ? 'Stop sharing screen' : 'Share screen'} aria-pressed={call.screenOn} title="Screen share (S)" disabled={!!call.joining} onclick={() => call.toggleScreen()}><Icon name="screen" /></button>
  {#if call.screenOn}<button class="another" disabled={call.participants.find(p => p.local)?.screens.length === 3} aria-label="Share another" title={call.participants.find(p => p.local)?.screens.length === 3 ? 'Up to 3 at once' : 'Share another'} onclick={() => call.addScreen()}><Icon name="plus" size={12} /></button>{/if}
  {#if large}<details class="overflow"><summary aria-label="Call options" title="Call options"><Icon name="more" /></summary><div><button onclick={(e) => { callLayouts.reset(call.channel?.id); e.currentTarget.closest('details')?.removeAttribute('open') }}>Reset layout</button></div></details>{/if}
  <button class="leave" aria-label="Leave call" title="Leave call" onclick={() => call.leave()}><Icon name="leave" /></button>
</div>

<style>
  .overflow { position: relative; }
  summary { width: 40px; height: 40px; display: grid; place-items: center; cursor: pointer; list-style: none; }
  summary::-webkit-details-marker { display: none; }
  .overflow > div { position: absolute; bottom: 46px; right: 0; padding: 4px; border-radius: 6px; border: 1px solid var(--line); background: var(--bg-2); }
  .overflow > div button { width: max-content; padding: 0 12px; font: 11px var(--mono); }
  .controls .another { width: 20px; height: 20px; margin-right: 2px; color: var(--lamp); border: 1px solid var(--line); }
  .controls { display: flex; align-items: center; gap: 0; flex: none; }
  button { display: grid; place-items: center; width: 28px; height: 28px; border-radius: 6px; color: var(--ink); }
  button:hover { background: var(--bg-3); }
  button:disabled { opacity: .5; cursor: default; }
  .muted { color: var(--ink-3); }
  .sharing { color: var(--lamp); }
  .leave { color: var(--ink-2); }
  .leave:hover { color: var(--ember); }
  .ptt { width: auto; padding: 0 6px; white-space: nowrap; font: 11px var(--mono); color: var(--ink-2); background: var(--bg); touch-action: none; }
  .ptt.held { color: var(--lamp); background: var(--lamp-glow); }
  .large { gap: 4px; }
  .large button { width: 40px; height: 40px; }
  .large .ptt { width: auto; padding: 0 10px; }
</style>
