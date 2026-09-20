import { sounds } from './sounds'
import { CameraBlur, MicrophoneGain, blurCaptureRate, blurSupported, defaultMicrophone, defaultCamera, deviceId, cameraConstraints, microphoneConstraints, type CameraBackground } from './av'
import { LocalVideoTrack, RemoteAudioTrack, Room, RoomEvent, Track, type Participant, type RemoteTrack, type ConnectionQuality, type LocalAudioTrack } from 'livekit-client'
import { store, instances, type Store } from './store.svelte'
import { Shares, type Share } from './call-shares'
import { HttpError } from './api'
import type { CallState, CallToken, Channel } from './types'

type Preferences = {
  mode: 'activity' | 'ptt'; pttKey: string; pttLabel: string
  microphone: string; camera: string; speaker: string
  micOn: boolean; cameraOn: boolean; sounds: boolean; musicDucking: boolean
}
const accountId = (identity: string) => identity.split(':', 1)[0]
const defaults: Preferences = { mode: 'activity', pttKey: 'Backquote', pttLabel: '`', microphone: '', camera: '', speaker: '', micOn: true, cameraOn: false, sounds: true, musicDucking: true }
function load(): Preferences {
  try { return { ...defaults, ...JSON.parse(localStorage.getItem('den.voice') || '{}') } } catch { return defaults }
}
export type CallParticipant = {
  id: string; userId: string; device: string; name: string; music: boolean; local: boolean; speaking: boolean; muted: boolean
  camera?: Track; screen?: Track; screens: Share[]; quality: ConnectionQuality
}

class Call {
  origin = $state('')
  instanceName = $state('')
  owner: Store | null = null
  get title() { return this.channel ? this.owner?.title(this.channel) || this.channel.name : '' }
  room = $state.raw<Room | null>(null)
  channel = $state<Channel | null>(null)
  participants = $state.raw<CallParticipant[]>([])
  states = $state<Map<string, string[]>>(new Map())
  prefs = $state<Preferences>(load())
  joining = $state<string | null>(null)
  expanded = $state(false)
  error = $state('')
  reconnecting = $state(false)
  audioBlocked = $state(false)
  outputMuted = $state(false)
  otherDevices = $state(0)
  held = $state(false)
  micOn = $state(false)
  cameraOn = $state(false)
  /** A blur processor is attached to the published camera. Reactive because the
   * Settings preview has to know which of the two camera tracks to show. */
  blurActive = $state(false)
  /** Why blur changed the camera or switched itself off. Not an error: the call
   * carries on either way. */
  blurNotice = $state('')
  /** The capture rate blur's own measurements settled on, or 0 for no limit. */
  blurRate = $state(0)
  backgroundBusy = $state(false)
  screenOn = $state(false)
  screenAdding = $state(false)
  shares = new Shares(() => this.room, this.refreshShares.bind(this))
  private refreshShares() { this.refresh() }
  private levels = $state<Record<string, { volume: number; muted: boolean }>>(this.loadLevels())
  private loadLevels() {
    try { const value = JSON.parse(localStorage.getItem('den.call-volume') || '{}'); return value && typeof value === 'object' ? value : {} } catch { return {} }
  }
  level(userId: string) {
    const saved = this.levels[`${this.origin}|${userId}`]
    return { volume: Number.isFinite(saved?.volume) ? Math.max(0, Math.min(1, saved.volume)) : 1, muted: saved?.muted === true }
  }
  volume(userId: string) { const level = this.level(userId); return level.muted ? 0 : level.volume }
  setVolume(userId: string, volume: number) { this.setLevel(userId, { volume: Math.max(0, Math.min(1, volume)), muted: false }) }
  muteForMe(userId: string) { this.setLevel(userId, { ...this.level(userId), muted: !this.level(userId).muted }) }
  private setLevel(userId: string, level: { volume: number; muted: boolean }) {
    this.levels = { ...this.levels, [`${this.origin}|${userId}`]: level }
    try { localStorage.setItem('den.call-volume', JSON.stringify(this.levels)) } catch { /* Session controls still work when storage is unavailable. */ }
    for (const [track, el] of this.audio) if (el.dataset.userId === userId) {
      el.volume = this.outputVolume(userId)
      if (track instanceof RemoteAudioTrack) track.setVolume(el.volume)
    }
  }
  ducked = $state(false)
  private duckTimer: ReturnType<typeof setTimeout> | undefined
  private outputVolume(userId: string) { return this.volume(userId) * (userId === `den-dj-${this.channel?.id}` && this.prefs.musicDucking && this.ducked ? Math.pow(10, -12 / 20) : 1) }
  private applyAudioLevels() {
    for (const [track, el] of this.audio) { el.volume = this.outputVolume(el.dataset.userId!); if (track instanceof RemoteAudioTrack) track.setVolume(el.volume) }
  }
  private duckMusic(speaking: boolean) {
    if (speaking) { clearTimeout(this.duckTimer); this.duckTimer = undefined; this.ducked = true; this.applyAudioLevels() }
    else if (this.ducked && !this.duckTimer) this.duckTimer = setTimeout(() => { this.ducked = false; this.duckTimer = undefined; this.applyAudioLevels() }, 700)
  }
  private generation = 0
  private audio = new Map<RemoteTrack, HTMLMediaElement>()
  private micQueue = Promise.resolve()
  private avQueue = Promise.resolve()
  gain = new MicrophoneGain((id) => (this.owner || store).voice.microphones[id] || defaultMicrophone)
  // Annotated because the type would otherwise be circular: the blur's target rate
  // reads cameraSettings, which reads cameraTrack, which reads the blur's source.
  blur: CameraBlur = new CameraBlur(() => this.cameraSettings.frame_rate, () => this.cameraSettings.background, (rate) => this.strained(rate))
  private get cameraPublication() {
    const track = this.room?.localParticipant.getTrackPublication(Track.Source.Camera)?.track
    return track instanceof LocalVideoTrack ? track : undefined
  }
  /** The camera itself, never the processed output. With blur on, the published
   * track is a generator or canvas track with no device and no capabilities, so
   * reading it here would empty the resolution and picture controls and collapse
   * every camera onto the `default` preferences key. */
  get cameraTrack() {
    const track = this.cameraPublication
    return (this.blurActive ? this.blur.source : track?.mediaStreamTrack) ?? undefined
  }
  get cameraPreferenceId() { return this.cameraTrack?.getSettings().deviceId || this.prefs.camera || 'default' }
  get cameraSettings() { return (this.owner || store).voice.cameras[this.cameraPreferenceId] || defaultCamera }
  private async applyCamera() {
    const track = this.cameraTrack
    const active = this.cameraPublication?.getProcessor() === this.blur
    const cap = active ? this.blurRate || this.blur.rate : undefined
    if (track?.readyState === 'live') await track.applyConstraints(cameraConstraints(this.cameraSettings, track, cap))
  }
  applyAV() {
    const room = this.room
    const update = this.avQueue.then(async () => {
      if (!room || this.room !== room) return
      await this.gain.update()
      await this.applyBackground(room, this.cameraSettings.background)
      await this.applyCamera()
    })
    this.avQueue = update.catch((err) => this.report(err))
    return this.avQueue
  }
  /** Blur reports it cannot hold its rate. A lower rate caps the capture instead of
   * dropping the effect; `null` means even the slowest step failed, so blur goes
   * rather than the call. */
  private strained(rate: number | null) {
    if (rate) {
      this.blurRate = rate
      this.blurNotice = `Background blur is holding your camera at ${rate} fps.`
      void this.applyAV()
    } else void this.setBackground('none', 'Background blur was turned off: this device could not keep up with it.')
  }
  private lastBackground: Exclude<CameraBackground, 'none'> = 'blur'
  async setBackground(requested: CameraBackground, notice = '') {
    const background = requested !== 'none' && !blurSupported() ? 'none' : requested
    if (background !== 'none') this.lastBackground = background
    const room = this.room, owner = this.owner || instances.active
    const id = this.cameraPreferenceId
    const previous = owner.voice.cameras[id] || defaultCamera
    const next = { ...previous, background }
    this.backgroundBusy = true; this.blurRate = 0
    const update = this.avQueue.then(async () => {
      if (!room || this.room !== room) return
      await this.applyBackground(room, background)
      await this.applyCamera()
    })
    this.avQueue = update.catch(() => {})
    try {
      await update
      await owner.saveVoice({ microphones: {}, cameras: { [id]: next } })
      this.blurNotice = requested !== background
        ? 'Background blur is unavailable in this browser, so your camera is unblurred.'
        : notice
    } catch (error) {
      const rollback = this.avQueue.then(async () => {
        if (!room || this.room !== room) return
        await this.applyBackground(room, previous.background)
        await this.applyCamera()
      })
      this.avQueue = rollback.catch(() => {})
      await rollback.catch(() => {})
      this.report(error)
      throw error
    } finally { this.backgroundBusy = false }
  }
  toggleBackground() {
    return this.setBackground(this.cameraSettings.background === 'none' ? this.lastBackground : 'none')
  }
  private async applyBackground(room: Room, background: CameraBackground) {
    const enabled = background !== 'none' && blurSupported()
    this.captureDefaults(room, background)
    const track = this.cameraPublication
    if (!track) return
    const active = track.getProcessor() === this.blur
    if (enabled && active) await this.blur.setBackground(background)
    else if (enabled) {
      try {
        // ProcessorWrapper.maxFps is fallback-only in track-processors 0.8.0.
        // Cap the real camera before modern processing starts, then keep applying
        // that cap in applyCamera as the performance ladder moves down.
        await track.mediaStreamTrack.applyConstraints(cameraConstraints(this.cameraSettings, track.mediaStreamTrack, blurCaptureRate(this.cameraSettings.frame_rate)))
        await track.setProcessor(this.blur)
      } catch (error) {
        // setProcessor stores the processor before its final sender/simulcast swaps.
        // Detach through LiveKit when that partial registration happened so a
        // stopped generated track cannot remain on the sender.
        if (track.getProcessor() === this.blur) await track.stopProcessor(false).catch(() => {})
        else await this.blur.destroy().catch(() => {})
        throw error
      }
    } else if (active) await track.stopProcessor(false)
    this.refresh()
  }
  /** The capture defaults are the single hook for the processor: `setCameraEnabled`
   * merges them into every camera it creates, including the ones the SDK recreates
   * on its own after a device change or a republish. */
  private captureDefaults(room: Room, background: CameraBackground) {
    const enabled = background !== 'none' && blurSupported()
    room.options.videoCaptureDefaults = {
      ...room.options.videoCaptureDefaults,
      frameRate: enabled ? blurCaptureRate(this.cameraSettings.frame_rate) : undefined,
      processor: enabled ? this.blur : undefined,
    }
  }
  private async enableCamera(room: Room, enabled: boolean) {
    const background = this.cameraSettings.background
    this.captureDefaults(room, background)
    try { await room.localParticipant.setCameraEnabled(enabled) } catch (err) {
      // The capability probe can pass on a machine where the segmenter still fails
      // to start, so bring the camera up unblurred instead of failing over a
      // decoration. A denied camera rethrows from the retry.
      if (!enabled || background === 'none' || !blurSupported()) throw err
      this.blurRate = 0
      this.captureDefaults(room, 'none')
      await this.blur.destroy().catch(() => { /* Never started. */ })
      try { await room.localParticipant.setCameraEnabled(enabled) } catch (retryError) {
        // The plain camera also failed, so this was permission/capture/publish,
        // not proof that the effect is broken. Preserve the user's preference.
        this.captureDefaults(room, background)
        throw retryError
      }
      this.refresh()
      this.blurNotice = 'Background blur could not start on this device, so your camera is unblurred.'
      const id = this.cameraPreferenceId, owner = this.owner || instances.active
      const current = owner.voice.cameras[id] || defaultCamera
      await owner.saveVoice({ microphones: {}, cameras: { [id]: { ...current, background: 'none' } } }).catch(saveError => this.report(saveError))
    }
  }

  save(patch: Partial<Preferences>) {
    this.prefs = { ...this.prefs, ...patch }
    localStorage.setItem('den.voice', JSON.stringify(this.prefs))
    this.applyAudioLevels()
  }
  receive(state: CallState) { this.states = new Map(this.states).set(state.channel_id, state.participant_ids) }
  snapshot(states: CallState[]) { this.states = new Map(states.map((s) => [s.channel_id, s.participant_ids])) }
  ids(id: string) { return this.states.get(id) || [] }
  report(err: unknown) {
    if (!(err instanceof HttpError)) void sounds.play('error', this.owner || store)
    if (err instanceof HttpError && err.status === 503) this.error = "Voice isn't set up on this server yet."
    else if (err instanceof Error && ['NotAllowedError', 'PermissionDeniedError'].includes(err.name)) this.error = `Allow camera and microphone access in your browser’s site settings.`
    else this.error = err instanceof Error ? err.message : 'The call could not connect. Try again.'
  }
  private refresh = () => {
    const room = this.room
    if (!room) return
    const members: Participant[] = [room.localParticipant, ...room.remoteParticipants.values()]
    const ownId = accountId(room.localParticipant.identity)
    this.otherDevices = members.filter((p) => p !== room.localParticipant && accountId(p.identity) === ownId).length
    const snapshot = (p: Participant): CallParticipant => {
      const devices = members.filter((m) => accountId(m.identity) === accountId(p.identity)).sort((a, b) => a.identity.localeCompare(b.identity))
      return {
        id: p.identity, userId: accountId(p.identity),
        device: devices.length > 1 ? p === room.localParticipant ? 'This device' : `Device ${devices.indexOf(p) + 1}` : '',
        music: p.identity === `den-dj-${this.channel?.id}`,
        name: p.identity === `den-dj-${this.channel?.id}` ? p.name || `${this.instanceName} DJ` : (this.owner || store).name(accountId(p.identity)), local: p === room.localParticipant,
        speaking: p.isSpeaking, muted: !p.isMicrophoneEnabled,
        camera: p.isCameraEnabled ? p.getTrackPublication(Track.Source.Camera)?.track : undefined,
        screen: p.getTrackPublication(Track.Source.ScreenShare)?.track,
        screens: this.shares.list(p), quality: p.connectionQuality,
      }
    }
    this.participants = [snapshot(room.localParticipant), ...[...room.remoteParticipants.values()].map(snapshot)]
    this.duckMusic(this.participants.some(p => !p.music && p.speaking))
    this.micOn = room.localParticipant.isMicrophoneEnabled
    this.cameraOn = room.localParticipant.isCameraEnabled
    this.blurActive = this.cameraPublication?.getProcessor() === this.blur
    this.screenOn = room.localParticipant.isScreenShareEnabled
  }
  private sound(join: boolean) {
    if (!this.outputMuted) void sounds.play(join ? 'call_join' : 'call_leave', this.owner || store)
  }
  async join(channel: Channel) {
    if (this.origin === store.origin && (this.channel?.id === channel.id || this.joining === channel.id)) return
    const owner = instances.active
    await this.leave()
    this.owner = owner; this.origin = owner.origin; this.instanceName = owner.settings.instance_name
    const generation = ++this.generation
    this.joining = channel.id; this.error = ''
    const prefs = this.prefs
    const camera = owner.voice.cameras[prefs.camera || 'default'] || defaultCamera
    const room = new Room({
      adaptiveStream: true, dynacast: true,
      audioCaptureDefaults: { ...microphoneConstraints(owner.voice.microphones[prefs.microphone || 'default'] || defaultMicrophone), deviceId: prefs.microphone || undefined },
      videoCaptureDefaults: { deviceId: prefs.camera || undefined, processor: camera.background !== 'none' && blurSupported() ? this.blur : undefined },
      audioOutput: { deviceId: prefs.speaker || 'default' },
    })
    try {
      const { url, token } = await owner.api.post<CallToken>(`/calls/${channel.id}/token`)
      if (generation !== this.generation) return
      await room.connect(url, token, { autoSubscribe: false })
      if (generation !== this.generation) { await room.disconnect(); return }
      this.room = room; this.channel = channel
      for (const event of [RoomEvent.ParticipantConnected, RoomEvent.ParticipantDisconnected, RoomEvent.TrackSubscribed,
        RoomEvent.TrackUnsubscribed, RoomEvent.TrackMuted, RoomEvent.TrackUnmuted, RoomEvent.LocalTrackPublished,
        RoomEvent.LocalTrackUnpublished, RoomEvent.ActiveSpeakersChanged, RoomEvent.TrackPublished, RoomEvent.TrackUnpublished,
        RoomEvent.ParticipantNameChanged, RoomEvent.ParticipantAttributesChanged, RoomEvent.ConnectionQualityChanged]) room.on(event, this.refresh)
      room.on(RoomEvent.ParticipantConnected, p => { if (accountId(p.identity) !== owner.me?.id && !this.outputMuted) void sounds.play('someone_joined', owner) })
      room.on(RoomEvent.ParticipantDisconnected, p => { if (accountId(p.identity) !== owner.me?.id && !this.outputMuted) void sounds.play('someone_left', owner) })
      const shareSound = (pub: { source: Track.Source }) => { if (pub.source === Track.Source.ScreenShare && !this.outputMuted) void sounds.play('screen_share_started', owner) }
      room.on(RoomEvent.TrackPublished, shareSound)
      room.on(RoomEvent.LocalTrackPublished, shareSound)
      const attachAudio = (track: RemoteTrack, participant: Participant) => {
        if (track.kind !== Track.Kind.Audio || this.outputMuted || this.audio.has(track)) return
        // Hear a phone's shared media on the desktop, but never echo our own mic.
        if (accountId(participant.identity) === accountId(room.localParticipant.identity) && track.source === Track.Source.Microphone) return
        const el = track.attach()
        el.dataset.userId = accountId(participant.identity)
        el.volume = this.outputVolume(el.dataset.userId)
        if (track instanceof RemoteAudioTrack) track.setVolume(el.volume)
        this.audio.set(track, el); document.body.append(el)
        el.play().catch(() => { if (!this.outputMuted) this.audioBlocked = true })
      }
      room.on(RoomEvent.TrackPublished, () => this.subscriptions())
      room.on(RoomEvent.TrackSubscribed, (track, _publication, participant) => attachAudio(track, participant))
      room.on(RoomEvent.TrackUnsubscribed, (track) => {
        track.detach().forEach((el) => el.remove()); this.audio.get(track)?.remove(); this.audio.delete(track)
      })
      room.on(RoomEvent.AudioPlaybackStatusChanged, () => { this.audioBlocked = !this.outputMuted && !room.canPlaybackAudio })
      room.on(RoomEvent.Reconnecting, () => { this.reconnecting = true; this.setHeld(false) })
      room.on(RoomEvent.Reconnected, () => { this.reconnecting = false; void this.applyAV(); this.refresh() })
      room.on(RoomEvent.Disconnected, () => {
        if (this.room === room) { const quiet = this.outputMuted; this.clear(); if (!quiet) this.sound(false) }
      })
      this.refresh()
      if (this.otherDevices > 0) {
        this.outputMuted = true
        this.save({ micOn: false })
      }
      this.subscriptions()
      if (!this.outputMuted) await room.startAudio()
      await this.enableMicrophone(room, prefs.mode === 'activity' && this.prefs.micOn)
      if (generation !== this.generation) { await room.disconnect(); return }
      if (prefs.cameraOn) await this.enableCamera(room, true)
      await this.applyAV()
      this.refresh(); this.sound(true)
    } catch (err) {
      await room.disconnect()
      if (generation === this.generation) { this.clear(); this.report(err) }
    } finally {
      if (generation === this.generation) this.joining = null
    }
  }
  private clear() {
    clearTimeout(this.duckTimer); this.duckTimer = undefined; this.ducked = false
    for (const [track, el] of this.audio) { track.detach(); el.remove() }
    void this.gain.destroy()
    // LiveKit owns a published processor and destroys it with the track. Destroying
    // the same instance here before room.disconnect can stop the sender early.
    this.blurActive = false; this.blurNotice = ''; this.blurRate = 0; this.backgroundBusy = false
    this.shares.clear(); this.audio.clear(); this.room = null; this.channel = null; this.participants = []
    this.held = false; this.expanded = false; this.reconnecting = false; this.audioBlocked = false
    this.micOn = false; this.cameraOn = false; this.screenOn = false; this.screenAdding = false
    this.outputMuted = false; this.otherDevices = 0
  }
  async leave() {
    ++this.generation; this.joining = null
    const room = this.room, quiet = this.outputMuted
    this.clear()
    if (room) { await room.disconnect(); if (!quiet) this.sound(false) }
  }
  async startAudio() {
    try { await this.room?.startAudio(); this.audioBlocked = false } catch (err) { this.report(err) }
  }
  private subscriptions() {
    const room = this.room
    if (!room) return
    for (const participant of room.remoteParticipants.values()) {
      for (const publication of participant.trackPublications.values()) {
        const ownMic = accountId(participant.identity) === accountId(room.localParticipant.identity) && publication.source === Track.Source.Microphone
        publication.setSubscribed(publication.kind !== Track.Kind.Audio || (!this.outputMuted && !ownMic))
      }
    }
  }
  async toggleOutput() {
    this.outputMuted = !this.outputMuted
    this.subscriptions()
    if (this.outputMuted) {
      for (const [track, el] of this.audio) { track.detach(); el.remove() }
      this.audio.clear(); this.audioBlocked = false
    } else await this.startAudio()
  }
  private async enableMicrophone(room: Room, enabled: boolean) {
    if (enabled && !room.localParticipant.getTrackPublication(Track.Source.Microphone)) {
      const [track] = await room.localParticipant.createTracks({ audio: true })
      try {
        await (track as LocalAudioTrack).setProcessor(this.gain)
        if (this.room !== room) { track.stop(); return }
        await room.localParticipant.publishTrack(track)
      } catch (err) { track.stop(); throw err }
    }
    await room.localParticipant.setMicrophoneEnabled(enabled)
  }
  private applyMic() {
    const room = this.room
    this.micQueue = this.micQueue.then(async () => {
      if (!room || this.room !== room) return
      await this.enableMicrophone(room, this.prefs.mode === 'ptt' ? this.held : this.prefs.micOn)
      this.refresh()
    }).catch((err) => this.report(err))
    return this.micQueue
  }
  toggleMic() { this.save({ micOn: !this.prefs.micOn }); return this.applyMic() }
  setHeld(held: boolean) {
    if (this.held === held) return
    this.held = held; void this.applyMic()
  }
  setMode(mode: Preferences['mode']) { this.held = false; this.save({ mode }); void this.applyMic() }
  async toggleCamera() {
    const room = this.room
    if (!room) return
    const enabled = !room.localParticipant.isCameraEnabled
    try {
      // track-processors 0.8.0 can wedge its MediaStreamTrackProcessor when the
      // source is muted and resumed in place. Detach before mute; applyAV installs
      // a fresh pipeline after LiveKit has brought the camera back.
      const track = this.cameraPublication
      if (!enabled && track?.getProcessor() === this.blur) await track.stopProcessor(false)
      await this.enableCamera(room, enabled)
      if (enabled) await this.applyAV()
      this.refresh(); this.save({ cameraOn: this.cameraOn })
    } catch (err) { this.report(err) }
  }
  async addScreen() {
    if (this.screenAdding) return
    const generation = this.generation
    this.screenAdding = true
    try { await this.shares.add(); this.refresh() } catch (err) {
      if (!(err instanceof Error && ['NotAllowedError', 'AbortError'].includes(err.name))) this.report(err)
    } finally { if (this.generation === generation) this.screenAdding = false }
  }
  async stopScreen(name?: string) {
    try { await this.shares.stop(name); this.refresh() } catch (err) { this.report(err) }
  }
  toggleScreen() { return this.screenOn ? this.stopScreen() : this.addScreen() }
  async screenQuality(name: string, mode: 'Smooth' | 'Sharp') {
    try { await this.shares.quality(name, mode); this.refresh() } catch (err) { this.report(err) }
  }
  async device(kind: MediaDeviceKind, id: string) {
    // A new camera restarts the track, which restarts the processor on it. Its rate
    // ladder starts over, so the cap the old camera earned must not linger.
    if (kind === 'videoinput') { this.blurRate = 0; this.blurNotice = '' }
    try {
      if (this.room && kind === 'audioinput') {
        const preferences = (this.owner || store).voice.microphones[id || 'default'] || defaultMicrophone
        const options = { ...microphoneConstraints(preferences), deviceId: id || 'default' }
        const track = this.room.localParticipant.getTrackPublication(Track.Source.Microphone)?.audioTrack
        // Device changes reacquire capture. Start with that device's processing
        // constraints, since browsers can reject changing them after capture.
        if (track) await track.restartTrack(options)
        this.room.options.audioCaptureDefaults = { ...this.room.options.audioCaptureDefaults, ...options }
      } else if (this.room) await this.room.switchActiveDevice(kind, id || 'default')
      await this.applyAV()
      this.refresh()
      this.save(kind === 'audioinput' ? { microphone: id } : kind === 'videoinput' ? { camera: id } : { speaker: id })
    } catch (err) { this.report(err) }
  }
  keydown(e: KeyboardEvent) {
    if (!this.room || this.joining || e.repeat || e.ctrlKey || e.metaKey || e.altKey || e.defaultPrevented) return
    const target = e.target as HTMLElement
    if (target.closest('[data-terminal-focus="true"],.tl-container,input,textarea,select,[contenteditable="true"],[role="dialog"]')) return
    if (this.prefs.mode === 'ptt' && e.code === this.prefs.pttKey) { e.preventDefault(); this.setHeld(true); return }
    const key = e.key.toLowerCase()
    if (key === 'm' && this.prefs.mode === 'activity') { e.preventDefault(); void this.toggleMic() }
    if (key === 'v') { e.preventDefault(); void this.toggleCamera() }
    if (key === 's') { e.preventDefault(); void this.toggleScreen() }
  }
  keyup(e: KeyboardEvent) { if (e.code === this.prefs.pttKey) this.setHeld(false) }
}
export const call = new Call()
