<script lang="ts">
  import DictationSettings from './DictationSettings.svelte'
  import { native } from '../lib/native'
  import { shell } from '../lib/shell-call'
  import VoiceDevices from './VoiceDevices.svelte'
  import { store } from '../lib/store.svelte'
  import { call } from '../lib/call.svelte'
  import { router } from '../lib/router.svelte'
  import SettingRow from './SettingRow.svelte'
  import Icon from './Icon.svelte'
  let capturing = $state(false)
  function capture(e: KeyboardEvent) {
    if (e.key === 'Tab') { capturing = false; return }
    e.preventDefault(); e.stopPropagation()
    if (e.key === 'Escape') { capturing = false; return }
    if (['Control', 'Meta', 'Alt', 'Shift'].includes(e.key)) return
    call.setHeld(false); call.save({ pttKey: e.code, pttLabel: e.key === ' ' ? 'Space' : e.key }); capturing = false
  }
</script>

{#snippet talking()}
  <h3 class="list-head">Talking</h3>
  <div class="slist">
    <SettingRow find="input-mode" label="Input mode" hint={call.prefs.mode !== 'ptt' ? undefined : shell ? 'Hold the talk button in a call.' : native ? 'Hold the key in any app while in a call, or hold the talk button.' : `Hold the key while ${store.settings.instance_name} has focus, or hold the talk button.`}>
      {#snippet control()}
        <div class="segments" role="radiogroup" aria-label="Input mode">
          <label><input type="radio" name="voice-mode" checked={call.prefs.mode === 'activity'} onchange={() => call.setMode('activity')} /> Voice activity</label>
          <label><input type="radio" name="voice-mode" checked={call.prefs.mode === 'ptt'} onchange={() => call.setMode('ptt')} /> Push to talk</label>
        </div>
      {/snippet}
    </SettingRow>
    {#if call.prefs.mode === 'ptt' && !shell}
      <SettingRow label="Push-to-talk key" hint="Click the box, then press the key you want.">
        {#snippet control()}<input class="field key" aria-label="Push-to-talk key" readonly value={capturing ? 'Press a key' : call.prefs.pttLabel} onfocus={() => (capturing = true)} onblur={() => (capturing = false)} onkeydown={capture} />{/snippet}
      </SettingRow>
    {/if}
    {#if !shell}
      <SettingRow find="camera-on" label="Join with camera on">
        {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="Join with camera on" checked={call.prefs.cameraOn} onchange={(e) => call.save({ cameraOn: e.currentTarget.checked })} />{/snippet}
      </SettingRow>
    {/if}
    <a class="nav-row" href="/settings/sounds" onclick={(e) => { e.preventDefault(); router.go('/settings/sounds') }}><span class="grow">Call tones</span><span class="val">In Sounds</span><span class="chev"><Icon name="chevron" size={14} /></span></a>
  </div>
{/snippet}

<h2 class="display">Voice</h2>
<!-- The iPhone app's calls run natively and voice only, with iOS's own microphone: the browser's device settings don't reach them. -->
{#if shell}{@render talking()}{:else}<VoiceDevices {talking} />{/if}

<DictationSettings />

<style>
  .key { width: 140px; text-align: center; cursor: pointer; }
</style>
