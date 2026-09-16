<script lang="ts">
  import { onMount } from 'svelte'
  import { Track } from 'livekit-client'
  import { store, instances } from '../lib/store.svelte'
  import { call } from '../lib/call.svelte'
  import { CameraBlur, MicrophoneGain, blurAccelerated, blurSupported, defaultMicrophone, defaultCamera, deviceId, capabilities, startPreviewBlur, supports, cameraConstraints, microphoneConstraints, type CameraCapabilities } from '../lib/av'
  import type { CameraSettings, MicrophoneSettings } from '../lib/types'

  let devices = $state<MediaDeviceInfo[]>([])
  let audio = $state.raw<MediaStreamTrack | null>(null)
  // Two roles, deliberately apart. `camera` is the real device: capabilities,
  // constraints, getSettings and the device ID that keys saved settings all read it.
  // `preview` is what the element shows, which is the processed track once blur runs.
  let camera = $state.raw<MediaStreamTrack | null>(null)
  let preview = $state.raw<MediaStreamTrack | null>(null)
  let caps = $state<CameraCapabilities>({})
  let actual = $state<MediaTrackSettings>({})
  let video = $state<HTMLVideoElement>()
  let level = $state(0)
  let draftGain = $state(1)
  $effect(() => { draftGain = mic.gain })
  let requesting = $state(false)
  let busy = $state(false)
  let error = $state('')
  let status = $state('')
  let paused = $state(false)
  let context: AudioContext | null = null
  let gain: MicrophoneGain | null = null
  let timer: ReturnType<typeof setInterval> | undefined
  let generation = 0
  let disposed = false
  let ownsCamera = false
  // Blur for an owned capture. Settings gets a separate processor from the call's,
  // never a share: the owned case only happens when no camera is published, so the
  // two never segment at once, and a shared instance would be re-initialised out
  // from under the preview the moment a call camera came up.
  let ownedBlur: CameraBlur | null = null
  let ownedBlurElement: HTMLVideoElement | null = null
  let ownedRate = 0
  const blurAvailable = blurSupported()
  const blurFast = blurAccelerated()
  const fpsCap = () => ownsCamera ? ownedRate : call.blurRate
  const micId = $derived(deviceId(audio))
  const camId = $derived(deviceId(camera))
  const mic = $derived(store.voice.microphones[micId] || defaultMicrophone)
  const cam = $derived(store.voice.cameras[camId] || defaultCamera)
  const requestedHeight = $derived(cam.resolution === '720p' ? 720 : cam.resolution === '1080p' ? 1080 : 0)
  const supportedRates = $derived([24, 30, 60].filter(rate => supports(caps.frameRate, rate)))
  const livePublication = $derived(call.origin === store.origin ? call.participants.find(p => p.local)?.camera : undefined)
  const speakerSupported = 'setSinkId' in HTMLMediaElement.prototype
  const processing = [ ['echo_cancellation', 'echoCancellation', 'Echo cancellation'], ['noise_suppression', 'noiseSuppression', 'Noise suppression'], ['auto_gain_control', 'autoGainControl', 'Auto gain'] ] as const
  async function enumerate() { if (!disposed) devices = await navigator.mediaDevices.enumerateDevices() }
  function stopAudio() {
    clearInterval(timer); audio?.stop(); audio = null
    void gain?.destroy(); gain = null; void context?.close(); context = null; level = 0
  }
  function stopCamera() {
    // Only an owned capture is Den's to release here. Destroying a processor
    // borrowed from the published track would end blur for the whole call.
    if (ownsCamera) { void ownedBlur?.destroy(); camera?.stop() }
    if (ownedBlurElement) { ownedBlurElement.srcObject = null; ownedBlurElement = null }
    ownedBlur = null; ownedRate = 0; camera = null; preview = null; ownsCamera = false
  }
  /** The owned preview's own frame-rate fallback, the same ladder the call uses. */
  function strain(track: MediaStreamTrack, rate: number | null) {
    if (camera !== track) return
    if (!rate) { void call.setBlur(false, 'Background blur was turned off: this device could not keep up with it.'); return }
    ownedRate = rate
    status = `Background blur is holding the preview at ${rate} fps.`
    void track.applyConstraints(cameraConstraints(cam, track, rate)).then(() => { if (camera === track) actual = track.getSettings() }).catch(() => { /* The preview keeps its current rate. */ })
  }
  async function startAudio() {
    const current = ++generation
    stopAudio()
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: { ...microphoneConstraints(store.voice.microphones[call.prefs.microphone || 'default'] || defaultMicrophone), deviceId: call.prefs.microphone ? { exact: call.prefs.microphone } : undefined } })
      const track = stream.getAudioTracks()[0]
      if (disposed || current !== generation) { track.stop(); return }
      audio = track
      context = new AudioContext(); await context.resume()
      gain = new MicrophoneGain(id => store.voice.microphones[id] || defaultMicrophone)
      await gain.init({ kind: Track.Kind.Audio, track, audioContext: context })
      if (disposed || current !== generation) return
      const analyser = context.createAnalyser(); analyser.fftSize = 1024
      context.createMediaStreamSource(new MediaStream([gain.processedTrack!])).connect(analyser)
      const data = new Uint8Array(analyser.fftSize)
      timer = setInterval(() => {
        analyser.getByteTimeDomainData(data)
        level = Math.min(100, Math.sqrt(data.reduce((sum, n) => sum + ((n - 128) / 128) ** 2, 0) / data.length) * 400)
      }, 1000 / 30)
      await enumerate()
    } catch (err) { if (!disposed) { stopAudio(); error = message(err) } }
  }
  function message(err: unknown) { return err instanceof Error ? `${err.name === 'NotAllowedError' ? 'Allow camera and microphone access in your browser settings. ' : ''}${err.message}` : 'Could not apply device settings.' }
  async function allow() {
    requesting = true; error = ''; paused = false; cameraRequested = false
    await startAudio()
    // Camera is requested separately, so a denied camera does not disable the mic.
    cameraRequested = true
    requesting = false
  }
  let cameraRequested = $state(false)
  $effect(() => {
    const live = livePublication, selected = call.prefs.camera, requested = cameraRequested, pause = paused, blur = call.prefs.blur
    // blurActive flips only once setProcessor has finished swapping the published
    // track, which is the moment there is a processed track worth showing.
    void call.blurActive
    if (!requested || pause) { stopCamera(); return }
    let cancelled = false
    stopCamera()
    void (async () => {
      try {
        // Borrowed: call.cameraTrack already resolves past the processor to the
        // camera, and the SDK has swapped the published track for the processed one.
        const track = live ? call.cameraTrack : (await navigator.mediaDevices.getUserMedia({ video: { deviceId: selected ? { exact: selected } : undefined } })).getVideoTracks()[0]
        if (!track) return
        if (cancelled || disposed) { if (!live) track.stop(); return }
        camera = track; preview = live ? live.mediaStreamTrack : track; ownsCamera = !live; caps = capabilities(track)
        await track.applyConstraints(cameraConstraints(store.voice.cameras[deviceId(track)] || defaultCamera, track, fpsCap()))
        actual = track.getSettings(); await enumerate()
        // Owned: there is no LocalVideoTrack to run setProcessor, so Settings drives
        // the processor itself. One instance, because nothing is published here.
        if (!live && blur && blurAvailable) {
          try {
            ownedBlur = new CameraBlur(() => (store.voice.cameras[deviceId(track)] || defaultCamera).frame_rate, rate => strain(track, rate))
            ownedBlurElement = await startPreviewBlur(ownedBlur, track)
            if (cancelled || disposed) { stopCamera(); return }
            preview = ownedBlur.processedTrack ?? track
          } catch (err) {
            // Report and restore the control, leaving the plain preview running.
            void ownedBlur?.destroy(); ownedBlur = null
            if (cancelled || disposed) return
            error = `Background blur was not turned on. ${message(err)}`
            void call.setBlur(false)
          }
        }
      } catch (err) { if (!cancelled && !disposed) error = message(err) }
    })()
    return () => { cancelled = true; stopCamera() }
  })
  $effect(() => {
    const element = video, track = preview
    if (element && track) { element.srcObject = new MediaStream([track]); void element.play().catch(() => {}); return () => { element.srcObject = null } }
  })
  $effect(() => {
    const value = mic, processor = gain
    if (audio && processor) void processor.update().catch(err => { error = message(err) })
    void value
  })
  $effect(() => {
    const value = cam, track = camera
    if (track) void track.applyConstraints(cameraConstraints(value, track, fpsCap())).then(() => { if (camera === track) actual = track.getSettings() }).catch(err => { error = message(err) })
  })
  async function changeMic(patch: Partial<MicrophoneSettings>) {
    if (!audio) return
    error = ''; busy = true
    const owner = instances.active
    const next = { ...mic, ...patch }, live = call.gain.source
    try {
      await audio.applyConstraints(microphoneConstraints(next, audio))
      if (live && deviceId(live) === micId) await live.applyConstraints(microphoneConstraints(next, live))
      await owner.saveVoice({ microphones: { [micId]: next }, cameras: {} })
      status = 'Microphone settings saved to your account.'
    } catch (err) {
      error = `Microphone setting was not saved. ${message(err)}`; draftGain = mic.gain
      await gain?.update().catch(() => {})
      await call.gain.update().catch(() => {})
    } finally { busy = false }
  }
  async function changeCamera(patch: Partial<CameraSettings>) {
    if (!camera) return
    error = ''; busy = true
    const next = { ...cam, ...patch }, track = camera, owner = instances.active
    try {
      await track.applyConstraints(cameraConstraints(next, track, fpsCap()))
      actual = track.getSettings()
      await owner.saveVoice({ microphones: {}, cameras: { [camId]: next } })
      status = 'Camera settings saved to your account.'
    } catch (err) {
      error = `Camera setting was not saved. ${message(err)}`
      await track.applyConstraints(cameraConstraints(cam, track, fpsCap())).catch(() => {})
    } finally { busy = false }
  }
  function adjustGain(value: number) {
    draftGain = value; gain?.setGain(value)
    if (deviceId(call.gain.source) === micId) call.gain.setGain(value)
  }
  async function input(id: string) { await call.device('audioinput', id); await startAudio() }
  onMount(() => {
    void enumerate().then(() => {
      if (disposed) return
      if (devices.some(d => d.kind === 'audioinput' && d.label)) void startAudio()
      if (devices.some(d => d.kind === 'videoinput' && d.label)) cameraRequested = true
    }).catch(err => { error = message(err) })
    navigator.mediaDevices.addEventListener('devicechange', enumerate)
    return () => { disposed = true; ++generation; stopAudio(); stopCamera(); navigator.mediaDevices.removeEventListener('devicechange', enumerate) }
  })
</script>

<p class="muted intro">Adjust your devices here or during a call. Settings are saved per device to your account.</p>
{#if !audio || (!camera && !paused)}<button class="btn lit" disabled={requesting} onclick={allow}>{requesting ? 'Opening devices…' : 'Allow microphone and camera'}</button>{/if}
<div class="devices">
  <section aria-label="Microphone controls">
    <h3>Microphone</h3>
    <label class="device">Input device<select class="field" aria-label="Microphone" value={call.prefs.microphone} onchange={e => input(e.currentTarget.value)}><option value="">Default microphone</option>{#each devices.filter(d => d.kind === 'audioinput' && d.deviceId) as d}<option value={d.deviceId}>{d.label || 'Microphone'}</option>{/each}</select></label>
    <label class="slider"><span>Input gain <output>{Math.round(draftGain * 100)}%</output></span><input aria-label="Input gain" aria-valuetext={`${Math.round(draftGain * 100)} percent`} type="range" min="0" max="2" step="0.05" value={draftGain} oninput={e => adjustGain(+e.currentTarget.value)} disabled={!audio || busy} onchange={e => changeMic({ gain: +e.currentTarget.value })} /></label>
    <div class="meter" role="meter" aria-label="Microphone level" aria-valuenow={Math.round(level)} aria-valuemin="0" aria-valuemax="100"><span style:width={`${level}%`}></span></div>
    <p class="muted hint">Level after gain. Your test audio stays here.</p>
    {#each processing as [key, capability, label]}
      {@const available = audio?.getCapabilities?.()[capability]}
      <label class="switch"><input type="checkbox" checked={available?.includes(true) && available?.includes(false) ? mic[key] : Boolean(audio?.getSettings()[capability] ?? mic[key])} disabled={!audio || busy || !available?.includes(true) || !available?.includes(false)} onchange={e => { const el = e.currentTarget; void changeMic({ [key]: el.checked }).then(() => { el.checked = mic[key] }) }} /> {label}</label>
      {#if audio && (!available?.includes(true) || !available?.includes(false))}<p class="muted hint">{label} is managed by this device.</p>{/if}
    {/each}
  </section>
  <section aria-label="Camera controls">
    <h3>Camera</h3>
    <label class="device">Video device<select class="field" aria-label="Camera" value={call.prefs.camera} onchange={e => call.device('videoinput', e.currentTarget.value)}><option value="">Default camera</option>{#each devices.filter(d => d.kind === 'videoinput' && d.deviceId) as d}<option value={d.deviceId}>{d.label || 'Camera'}</option>{/each}</select></label>
    <div class="preview">
      {#if camera}<video bind:this={video} class:mirror={cam.mirror} autoplay muted playsinline aria-label="Camera preview"></video>{:else}<span class="muted">{paused ? 'Preview paused' : 'Camera preview'}</span>{/if}
    </div>
    <div class="preview-meta"><span class="muted">{#if camera}{actual.width} × {actual.height} · {Math.round(actual.frameRate || 0)} fps{:else}Only you can see this preview{/if}</span><button class="text-link" onclick={() => { paused = !paused; cameraRequested = true }}>{paused ? 'Resume preview' : 'Pause preview'}</button></div>
    <div class="quality">
      <label class="device">Resolution<select aria-label="Resolution" class="field" value={cam.resolution} disabled={!camera || busy} onchange={e => { const el = e.currentTarget; void changeCamera({ resolution: el.value as CameraSettings['resolution'] }).then(() => { el.value = cam.resolution }) }}><option value="auto">Auto</option>{#if requestedHeight && (!supports(caps.height, requestedHeight) || !supports(caps.width, requestedHeight * 16 / 9))}<option value={cam.resolution} disabled>{cam.resolution} unavailable</option>{/if}{#each [720, 1080] as height}{#if supports(caps.height, height) && supports(caps.width, height * 16 / 9)}<option value={`${height}p`}>{height}p</option>{/if}{/each}</select></label>
      <label class="device">Frame rate<select aria-label="Frame rate" class="field" value={cam.frame_rate} disabled={!camera || busy || !supportedRates.length} onchange={e => { const el = e.currentTarget; void changeCamera({ frame_rate: +el.value }).then(() => { el.value = String(cam.frame_rate) }) }}>{#if !supportedRates.includes(cam.frame_rate)}<option value={cam.frame_rate} disabled>{Math.round(actual.frameRate || 0)} fps · device default</option>{/if}{#each supportedRates as rate}<option value={rate}>{rate} fps</option>{/each}</select></label>
    </div>
    <label class="switch"><input type="checkbox" checked={cam.mirror} disabled={!camera || busy} onchange={e => changeCamera({ mirror: e.currentTarget.checked })} /> Mirror my preview</label>
    <label class="switch"><input type="checkbox" checked={call.prefs.blur} disabled={!blurAvailable || busy} onchange={e => { const el = e.currentTarget; void call.setBlur(el.checked).then(() => { el.checked = call.prefs.blur }) }} /> Blur my background</label>
    {#if !blurAvailable}<p class="muted hint">Background blur needs frame processing and WebGL2, which this browser does not have.</p>
    {:else if !blurFast}<p class="muted hint">Background blur runs through a slower path in this browser and may cost you frame rate.</p>{/if}
    {#if call.blurNotice}<p class="muted hint">{call.blurNotice}</p>{/if}
    {#each ['brightness', 'contrast', 'saturation'] as key}
      {@const control = key as 'brightness' | 'contrast' | 'saturation'}
      {@const range = caps[control]}
      {#if range && range.max > range.min}<label class="slider"><span class="capitalize">{control}</span><input aria-label={control} type="range" min={range.min} max={range.max} step={range.step || (range.max - range.min) / 100} value={cam[control] ?? (actual as MediaTrackSettings & Record<string, number>)[control] ?? range.min} disabled={busy} onchange={e => changeCamera({ [control]: +e.currentTarget.value })} /></label>{/if}
    {/each}
  </section>
</div>
{#if error}<p role="alert" class="error">{error}</p>{/if}
<p role="status" class="muted hint status">{status}</p>
{#if speakerSupported}<label class="device">Speaker<select class="field" value={call.prefs.speaker} onchange={e => call.device('audiooutput', e.currentTarget.value)}><option value="">Default speaker</option>{#each devices.filter(d => d.kind === 'audiooutput' && d.deviceId) as d}<option value={d.deviceId}>{d.label || 'Speaker'}</option>{/each}</select></label>{/if}

<style>
  .intro { font-size: 13px; line-height: 1.5; margin: 0 0 20px; }
  .devices { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 24px; }
  section { min-width: 0; }
  h3 { margin: 0 0 12px; }
  .device { display: grid; gap: 6px; margin: 0 0 14px; font-size: 13px; }
  select { width: 100%; min-width: 0; font: inherit; }
  .slider { display: grid; gap: 6px; font-size: 13px; margin: 16px 0 6px; }
  .slider span { display: flex; justify-content: space-between; }
  output { font-variant-numeric: tabular-nums; }
  input[type=range] { width: 100%; min-height: 28px; accent-color: var(--lamp); }
  .meter { height: 6px; border-radius: 3px; overflow: hidden; background: var(--line); }
  .meter span { display: block; height: 100%; background: var(--lamp); }
  .hint { font-size: 11px; line-height: 1.5; margin: 6px 0 12px; }
  .switch { display: flex; align-items: center; gap: 10px; min-height: 40px; font-size: 13px; }
  .switch input { accent-color: var(--lamp); width: 16px; height: 16px; }
  .preview { aspect-ratio: 16/9; display: grid; place-items: center; background: var(--bg-3); border-radius: var(--r-lg); overflow: hidden; }
  video { width: 100%; height: 100%; object-fit: contain; }
  .mirror { transform: scaleX(-1); }
  .preview-meta { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 4px; font-size: 11px; margin: 4px 0 10px; }
  .text-link { color: var(--ink-2); text-decoration: underline; min-height: 28px; font: inherit; }
  .quality { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .capitalize { text-transform: capitalize; }
  .error { font-size: 13px; color: var(--ember); }
  .status { min-height: 16px; }
  @media (max-width: 760px) { .devices { grid-template-columns: 1fr; gap: 24px; } }
</style>
