<script lang="ts">
  import DictationSettings from './DictationSettings.svelte'
  import { native } from '../lib/native'
  import VoiceDevices from './VoiceDevices.svelte'
  import { store } from '../lib/store.svelte'
  import { call } from '../lib/call.svelte'
  let capturing = $state(false)
  function capture(e: KeyboardEvent) {
    if (e.key === 'Tab') { capturing = false; return }
    e.preventDefault(); e.stopPropagation()
    if (e.key === 'Escape') { capturing = false; return }
    if (['Control', 'Meta', 'Alt', 'Shift'].includes(e.key)) return
    call.setHeld(false); call.save({ pttKey: e.code, pttLabel: e.key === ' ' ? 'Space' : e.key }); capturing = false
  }
</script>

<h2 class="display">Voice</h2>
<fieldset>
  <legend class="eyebrow">Input mode</legend>
  <label class="switch"><input type="radio" name="voice-mode" checked={call.prefs.mode === 'activity'} onchange={() => call.setMode('activity')} /> Voice activity</label>
  <label class="switch"><input type="radio" name="voice-mode" checked={call.prefs.mode === 'ptt'} onchange={() => call.setMode('ptt')} /> Push to talk</label>
</fieldset>
{#if call.prefs.mode === 'ptt'}
  <label class="device">Push-to-talk key<input class="field mono" readonly value={capturing ? 'Press a key' : call.prefs.pttLabel} onfocus={() => (capturing = true)} onblur={() => (capturing = false)} onkeydown={capture} /></label>
  <p class="muted small">{native ? 'Hold the key in any app while in a call, or hold the talk button.' : `Hold the key while ${store.settings.instance_name} has focus, or hold the talk button.`}</p>
{/if}
<VoiceDevices />
<label class="switch"><input type="checkbox" checked={call.prefs.cameraOn} onchange={(e) => call.save({ cameraOn: e.currentTarget.checked })} /> Join with camera on</label>
<p class="muted">Choose call tones in <a href="/settings/sounds">Sounds settings</a>.</p>

<DictationSettings />

<style>

  fieldset { border: 0; padding: 0; margin: 16px 0; }
  .switch { display: flex; align-items: center; gap: 10px; padding: 8px 0; }
  .switch input { accent-color: var(--lamp); width: 16px; height: 16px; }
  .device { display: grid; gap: 6px; margin: 18px 0 10px; }
  .small { font-size: 13px; }
</style>
