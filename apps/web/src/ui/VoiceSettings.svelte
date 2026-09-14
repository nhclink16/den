<script lang="ts">
  import DictationSettings from './DictationSettings.svelte'
  import { native } from '../lib/native'
  import { onMount } from 'svelte'
  import { store } from '../lib/store.svelte'
  import { call } from '../lib/call.svelte'
  let devices = $state<MediaDeviceInfo[]>([])
  let permitted = $state(false)
  let requesting = $state(false)
  let level = $state(0)
  let capturing = $state(false)
  let stream: MediaStream | null = null
  let context: AudioContext | null = null
  let timer: ReturnType<typeof setInterval> | undefined
  let disposed = false
  let previewGeneration = 0
  const speakerSupported = 'setSinkId' in HTMLMediaElement.prototype
  async function enumerate() { devices = await navigator.mediaDevices.enumerateDevices() }
  function stopMeter() {
    clearInterval(timer); stream?.getTracks().forEach((t) => t.stop()); stream = null
    void context?.close(); context = null; level = 0
  }
  async function meter() {
    const generation = ++previewGeneration
    stopMeter()
    try {
      const next = await navigator.mediaDevices.getUserMedia({ audio: { deviceId: call.prefs.microphone || undefined, echoCancellation: true, noiseSuppression: true, autoGainControl: true } })
      if (disposed || generation !== previewGeneration) { next.getTracks().forEach((t) => t.stop()); return }
      stream = next; context = new AudioContext(); await context.resume()
      const analyser = context.createAnalyser(); analyser.fftSize = 256
      context.createMediaStreamSource(stream).connect(analyser)
      const data = new Uint8Array(analyser.fftSize)
      timer = setInterval(() => {
        analyser.getByteTimeDomainData(data)
        const rms = Math.sqrt(data.reduce((sum, n) => sum + ((n - 128) / 128) ** 2, 0) / data.length)
        level = Math.min(100, rms * 400)
      }, 1000 / 30)
    } catch (err) { if (!disposed) call.report(err) }
  }
  async function allow() {
    requesting = true
    try {
      const media = await navigator.mediaDevices.getUserMedia({ audio: true, video: true })
      media.getTracks().forEach((t) => t.stop())
      if (disposed) return
      permitted = true; await enumerate(); await meter()
    } catch (err) { call.report(err) } finally { requesting = false }
  }
  async function input(id: string) { await call.device('audioinput', id); if (permitted) await meter() }
  onMount(() => {
    void enumerate().then(async () => {
      // Device labels are available after permission has already been granted.
      if (!disposed && devices.some((d) => d.kind === 'audioinput' && d.label) && devices.some((d) => d.kind === 'videoinput' && d.label)) {
        permitted = true; await meter()
      }
    }).catch((err) => call.report(err))
    navigator.mediaDevices.addEventListener('devicechange', enumerate)
    return () => { disposed = true; ++previewGeneration; stopMeter(); navigator.mediaDevices.removeEventListener('devicechange', enumerate) }
  })
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
{#if !permitted}<button class="btn lit" disabled={requesting} onclick={allow}>Allow microphone and camera</button>{/if}
<label class="device">Microphone<select class="field" value={call.prefs.microphone} onchange={(e) => input(e.currentTarget.value)}><option value="">Default microphone</option>{#each devices.filter((d) => d.kind === 'audioinput' && d.deviceId) as d}<option value={d.deviceId}>{d.label || 'Microphone'}</option>{/each}</select></label>
<div class="meter" role="meter" aria-label="Microphone level" aria-valuenow={Math.round(level)} aria-valuemin="0" aria-valuemax="100"><span style:width={`${level}%`}></span></div>
<label class="device">Camera<select class="field" value={call.prefs.camera} onchange={(e) => call.device('videoinput', e.currentTarget.value)}><option value="">Default camera</option>{#each devices.filter((d) => d.kind === 'videoinput' && d.deviceId) as d}<option value={d.deviceId}>{d.label || 'Camera'}</option>{/each}</select></label>
{#if speakerSupported}<label class="device">Speaker<select class="field" value={call.prefs.speaker} onchange={(e) => call.device('audiooutput', e.currentTarget.value)}><option value="">Default speaker</option>{#each devices.filter((d) => d.kind === 'audiooutput' && d.deviceId) as d}<option value={d.deviceId}>{d.label || 'Speaker'}</option>{/each}</select></label>{/if}
<label class="switch"><input type="checkbox" checked={call.prefs.cameraOn} onchange={(e) => call.save({ cameraOn: e.currentTarget.checked })} /> Join with camera on</label>
<label class="switch"><input type="checkbox" checked={call.prefs.sounds} onchange={(e) => call.save({ sounds: e.currentTarget.checked })} /> Play join and leave sounds</label>

<DictationSettings />

<style>
  h2 { font-size: 26px; margin: 0 0 6px; }
  fieldset { border: 0; padding: 0; margin: 16px 0; }
  .switch { display: flex; align-items: center; gap: 10px; padding: 8px 0; }
  .switch input { accent-color: var(--lamp); width: 16px; height: 16px; }
  .device { display: grid; gap: 6px; margin: 18px 0 10px; }
  select { font: inherit; }
  .meter { height: 6px; border-radius: 3px; overflow: hidden; background: var(--line); }
  .meter span { display: block; height: 100%; background: var(--lamp); }
  .small { font-size: 13px; }
</style>
