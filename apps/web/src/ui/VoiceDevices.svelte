<script lang="ts">
  import { onMount, type Snippet } from 'svelte'
  import { Track } from 'livekit-client'
  import { store, instances } from '../lib/store.svelte'
  import { call } from '../lib/call.svelte'
  import { CameraBlur, MicrophoneGain, blurCaptureRate, blurSupported, defaultMicrophone, defaultCamera, deviceId, capabilities, startPreviewBlur, supports, cameraConstraints, microphoneConstraints, type CameraBackground, type CameraCapabilities } from '../lib/av'
  import type { CameraSettings, MicrophoneSettings } from '../lib/types'
  import SettingRow from './SettingRow.svelte'

  /** The Talking group, drawn after the permission ask so that ask comes first. */
  let { talking }: { talking?: Snippet } = $props()
  // A phone has no use for the computer's tuning: gain, processing, resolution and frame rate.
  const touch = matchMedia('(pointer: coarse)').matches

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
  const fpsCap = () => ownsCamera ? ownedRate || ownedBlur?.rate || 0 : call.blurRate
  const micId = $derived(deviceId(audio))
  const camId = $derived(deviceId(camera))
  const mic = $derived(store.voice.microphones[micId] || defaultMicrophone)
  const cam = $derived(store.voice.cameras[camId] || defaultCamera)
  const requestedHeight = $derived(cam.resolution === '720p' ? 720 : cam.resolution === '1080p' ? 1080 : 0)
  const supportedRates = $derived([24, 30, 60].filter(rate => supports(caps.frameRate, rate)))
  const livePublication = $derived(call.origin === store.origin ? call.participants.find(p => p.local)?.camera : undefined)
  const speakerSupported = 'setSinkId' in HTMLMediaElement.prototype
  // A picker with nothing to pick (just the default) isn't shown.
  const listed = (kind: MediaDeviceKind) => devices.filter(d => d.kind === kind && d.deviceId && d.deviceId !== 'default')
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
    if (!rate) {
      call.blurNotice = 'Background blur was turned off: this device could not keep up with it.'
      void changeCamera({ background: 'none' })
      return
    }
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
    const live = livePublication, selected = call.prefs.camera, requested = cameraRequested, pause = paused
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
      } catch (err) { if (!cancelled && !disposed) error = message(err) }
    })()
    return () => { cancelled = true; stopCamera() }
  })
  $effect(() => {
    const track = camera, owned = ownsCamera, background = cam.background
    if (!track || !owned) return
    preview = track; ownedRate = 0
    if (background === 'none' || !blurAvailable) return
    let cancelled = false, element: HTMLVideoElement | null = null
    const processor = new CameraBlur(
      () => (store.voice.cameras[deviceId(track)] || defaultCamera).frame_rate,
      () => (store.voice.cameras[deviceId(track)] || defaultCamera).background,
      rate => { if (ownedBlur === processor) strain(track, rate) },
    )
    ownedBlur = processor
    void (async () => {
      try {
        await track.applyConstraints(cameraConstraints(cam, track, blurCaptureRate(cam.frame_rate)))
        if (camera === track) actual = track.getSettings()
        element = await startPreviewBlur(processor, track)
        if (cancelled || disposed || camera !== track || ownedBlur !== processor) {
          element.srcObject = null; await processor.destroy(); return
        }
        ownedBlurElement = element
        preview = processor.processedTrack ?? track
      } catch (err) {
        await processor.destroy().catch(() => {})
        if (cancelled || disposed || camera !== track || ownedBlur !== processor) return
        ownedBlur = null
        error = `Background blur was not turned on. ${message(err)}`
        call.blurNotice = 'Background blur could not start on this device, so your camera is unblurred.'
        void changeCamera({ background: 'none' })
      }
    })()
    return () => {
      cancelled = true
      if (element) element.srcObject = null
      if (ownedBlur === processor) { ownedBlur = null; ownedBlurElement = null }
      if (camera === track) preview = track
      void processor.destroy()
    }
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
  async function changeBackground(background: CameraBackground) {
    if (!camera) return
    if (!livePublication) { await changeCamera({ background }); return }
    error = ''; busy = true
    try {
      await call.setBackground(background)
      status = 'Camera settings saved to your account.'
    } catch (err) { error = `Camera setting was not saved. ${message(err)}` }
    finally { busy = false }
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

{#if !audio || (!camera && !paused)}
  <!-- The ask comes first: everything below needs it. -->
  <div class="slist allow">
    <button class="action-row" disabled={requesting} onclick={allow}>{requesting ? 'Opening devices…' : 'Allow Microphone and Camera'}</button>
  </div>
  <p class="list-foot">To test and adjust them here. Only you hear and see the test.</p>
{/if}
{@render talking?.()}
{#if listed('audioinput').length > 1 || !touch}
<h3 class="list-head">Microphone</h3>
<section class="slist" aria-label="Microphone controls">
  {#if listed('audioinput').length > 1 || !touch}
  <SettingRow find="microphone" label="Input device">
    {#snippet control()}<select class="field" aria-label="Microphone" value={call.prefs.microphone} onchange={e => input(e.currentTarget.value)}><option value="">Default microphone</option>{#each devices.filter(d => d.kind === 'audioinput' && d.deviceId) as d}<option value={d.deviceId}>{d.label || 'Microphone'}</option>{/each}</select>{/snippet}
  </SettingRow>
  {/if}
  {#if !touch}
  <SettingRow find="gain" label="Input gain" hint="The bar shows your level after gain. Your test audio stays here." wide>
    {#snippet control()}
      <div class="gain">
        <span class="slider"><input aria-label="Input gain" aria-valuetext={`${Math.round(draftGain * 100)} percent`} type="range" min="0" max="2" step="0.05" value={draftGain} oninput={e => adjustGain(+e.currentTarget.value)} disabled={!audio || busy} onchange={e => changeMic({ gain: +e.currentTarget.value })} /><output>{Math.round(draftGain * 100)}%</output></span>
        <div class="meter" role="meter" aria-label="Microphone level" aria-valuenow={Math.round(level)} aria-valuemin="0" aria-valuemax="100"><span style:width={`${level}%`}></span></div>
      </div>
    {/snippet}
  </SettingRow>
  {#each processing as [key, capability, label], i}
    {@const available = audio?.getCapabilities?.()[capability]}
    <SettingRow find={i === 0 ? 'processing' : undefined} {label} hint={audio && (!available?.includes(true) || !available?.includes(false)) ? `${label} is managed by this device.` : undefined}>
      {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label={label} checked={available?.includes(true) && available?.includes(false) ? mic[key] : Boolean(audio?.getSettings()[capability] ?? mic[key])} disabled={!audio || busy || !available?.includes(true) || !available?.includes(false)} onchange={e => { const el = e.currentTarget; void changeMic({ [key]: el.checked }).then(() => { el.checked = mic[key] }) }} />{/snippet}
    </SettingRow>
  {/each}
  {/if}
</section>
{/if}

<h3 class="list-head">Camera</h3>
<section class="slist" aria-label="Camera controls">
  {#if listed('videoinput').length > 1 || !touch}
  <SettingRow find="camera" label="Video device">
    {#snippet control()}<select class="field" aria-label="Camera" value={call.prefs.camera} onchange={e => call.device('videoinput', e.currentTarget.value)}><option value="">Default camera</option>{#each devices.filter(d => d.kind === 'videoinput' && d.deviceId) as d}<option value={d.deviceId}>{d.label || 'Camera'}</option>{/each}</select>{/snippet}
  </SettingRow>
  {/if}
  <!-- The preview shows once there is a camera to show, not as an empty box before. -->
  {#if camera || paused}
  <div class="pad">
    <div class="preview">
      {#if camera}<video bind:this={video} class:mirror={cam.mirror} autoplay muted playsinline aria-label="Camera preview"></video>{:else}<span class="muted">Preview paused</span>{/if}
    </div>
    <div class="preview-meta"><span class="muted">{#if camera && !touch}{actual.width} × {actual.height} · {Math.round(actual.frameRate || 0)} fps{:else}Only you can see this preview{/if}</span><button class="text-link" onclick={() => { paused = !paused; cameraRequested = true }}>{paused ? 'Resume preview' : 'Pause preview'}</button></div>
  </div>
  {/if}
  {#if !touch}
  <SettingRow find="quality" label="Resolution and frame rate">
    {#snippet control()}
      <select aria-label="Resolution" class="field" value={cam.resolution} disabled={!camera || busy} onchange={e => { const el = e.currentTarget; void changeCamera({ resolution: el.value as CameraSettings['resolution'] }).then(() => { el.value = cam.resolution }) }}><option value="auto">Auto</option>{#if requestedHeight && (!supports(caps.height, requestedHeight) || !supports(caps.width, requestedHeight * 16 / 9))}<option value={cam.resolution} disabled>{cam.resolution} unavailable</option>{/if}{#each [720, 1080] as height}{#if supports(caps.height, height) && supports(caps.width, height * 16 / 9)}<option value={`${height}p`}>{height}p</option>{/if}{/each}</select>
      <select aria-label="Frame rate" class="field" value={cam.frame_rate} disabled={!camera || busy || !supportedRates.length} onchange={e => { const el = e.currentTarget; void changeCamera({ frame_rate: +el.value }).then(() => { el.value = String(cam.frame_rate) }) }}>{#if !supportedRates.includes(cam.frame_rate)}<option value={cam.frame_rate} disabled>{Math.round(actual.frameRate || 0)} fps · device default</option>{/if}{#each supportedRates as rate}<option value={rate}>{rate} fps</option>{/each}</select>
    {/snippet}
  </SettingRow>
  {/if}
  <SettingRow find="mirror" label="Mirror my preview" hint={camera ? 'Only how you see yourself. Others see you the right way round.' : 'Allow the camera above to change this.'}>
    {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="Mirror my preview" checked={cam.mirror} disabled={!camera || busy} onchange={e => changeCamera({ mirror: e.currentTarget.checked })} />{/snippet}
  </SettingRow>
  {#if blurAvailable}
  <SettingRow find="background" label="Background" hint={call.blurNotice || undefined}>
    {#snippet control()}
      <fieldset class="segments" disabled={!camera || busy} aria-label="Camera background">
        {#each [['none', 'None'], ['blur', 'Blur'], ['light_blur', 'Light blur']] as [value, label]}
          <label><input type="radio" name="camera-background" value={value} checked={(blurAvailable ? cam.background : 'none') === value} disabled={value !== 'none' && !blurAvailable} onchange={() => { void changeBackground(value as CameraBackground) }} /> {label}</label>
        {/each}
      </fieldset>
    {/snippet}
  </SettingRow>
  {/if}
  {#each ['brightness', 'contrast', 'saturation'] as key}
    {@const prop = key as 'brightness' | 'contrast' | 'saturation'}
    {@const range = caps[prop]}
    {#if range && range.max > range.min}
      <SettingRow label={prop[0]!.toUpperCase() + prop.slice(1)}>
        {#snippet control()}<input aria-label={prop} type="range" min={range.min} max={range.max} step={range.step || (range.max - range.min) / 100} value={cam[prop] ?? (actual as MediaTrackSettings & Record<string, number>)[prop] ?? range.min} disabled={busy} onchange={e => changeCamera({ [prop]: +e.currentTarget.value })} />{/snippet}
      </SettingRow>
    {/if}
  {/each}
</section>
{#if error}<p role="alert" class="error">{error}</p>{/if}
<p role="status" class="hint status">{status}</p>
{#if speakerSupported && (listed('audiooutput').length > 1 || !touch)}
  <h3 class="list-head">Speaker</h3>
  <div class="slist">
    <SettingRow find="speaker" label="Output device">
      {#snippet control()}<select class="field" aria-label="Speaker" value={call.prefs.speaker} onchange={e => call.device('audiooutput', e.currentTarget.value)}><option value="">Default speaker</option>{#each devices.filter(d => d.kind === 'audiooutput' && d.deviceId) as d}<option value={d.deviceId}>{d.label || 'Speaker'}</option>{/each}</select>{/snippet}
    </SettingRow>
  </div>
{/if}

<style>
  .allow { margin-top: 18px; }
  select.field { width: auto; min-width: 0; max-width: 100%; }
  .gain { display: grid; gap: 8px; width: 100%; }
  .slider { display: flex; align-items: center; gap: 10px; }
  output { font-size: var(--fs-xs); color: var(--ink-2); min-width: 38px; text-align: end; font-variant-numeric: tabular-nums; }
  input[type=range] { flex: 1; min-width: 0; min-height: 28px; accent-color: var(--lamp); }
  .meter { height: 6px; border-radius: 3px; overflow: hidden; background: var(--line); }
  .meter span { display: block; height: 100%; background: var(--lamp); }
  .pad { padding: 4px 16px 12px; }
  .preview { aspect-ratio: 16/9; display: grid; place-items: center; background: var(--bg-3); border-radius: var(--r); overflow: hidden; }
  video { width: 100%; height: 100%; object-fit: contain; }
  .mirror { transform: scaleX(-1); }
  .preview-meta { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 4px; font-size: var(--fs-xs); margin-top: 4px; }
  .text-link { color: var(--ink-2); text-decoration: underline; min-height: 28px; font: inherit; }
  fieldset.segments { margin: 0; border: 0; }
  .hint { font-size: var(--fs-xs); line-height: 1.5; margin: 6px 4px 0; color: var(--ink-3); }
  .error { font-size: var(--fs-sm); color: var(--danger); }
  .status { min-height: 16px; }
</style>
