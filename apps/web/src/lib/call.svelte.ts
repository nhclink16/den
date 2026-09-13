import { Room, RoomEvent, Track, type Participant, type RemoteTrack } from 'livekit-client'
import { api, HttpError } from './api'
import type { CallState, CallToken, Channel } from './types'

type Preferences = {
  mode: 'activity' | 'ptt'; pttKey: string; pttLabel: string
  microphone: string; camera: string; speaker: string
  micOn: boolean; cameraOn: boolean; sounds: boolean
}
const accountId = (identity: string) => identity.split(':', 1)[0]
const defaults: Preferences = { mode: 'activity', pttKey: 'Backquote', pttLabel: '`', microphone: '', camera: '', speaker: '', micOn: true, cameraOn: false, sounds: true }
function load(): Preferences {
  try { return { ...defaults, ...JSON.parse(localStorage.getItem('den.voice') || '{}') } } catch { return defaults }
}
export type CallParticipant = {
  id: string; userId: string; device: string; name: string; local: boolean; speaking: boolean; muted: boolean
  camera?: Track; screen?: Track
}

class Call {
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
  screenOn = $state(false)
  private generation = 0
  private audio = new Map<RemoteTrack, HTMLMediaElement>()
  private micQueue = Promise.resolve()

  save(patch: Partial<Preferences>) {
    this.prefs = { ...this.prefs, ...patch }
    localStorage.setItem('den.voice', JSON.stringify(this.prefs))
  }
  receive(state: CallState) { this.states = new Map(this.states).set(state.channel_id, state.participant_ids) }
  snapshot(states: CallState[]) { this.states = new Map(states.map((s) => [s.channel_id, s.participant_ids])) }
  ids(id: string) { return this.states.get(id) || [] }
  report(err: unknown) {
    if (err instanceof HttpError && err.status === 503) this.error = "Voice isn't set up on this server yet."
    else if (err instanceof Error && ['NotAllowedError', 'PermissionDeniedError'].includes(err.name)) this.error = "Den needs your microphone. Allow it in the browser's site settings."
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
        name: p.name || p.identity, local: p === room.localParticipant,
        speaking: p.isSpeaking, muted: !p.isMicrophoneEnabled,
        camera: p.isCameraEnabled ? p.getTrackPublication(Track.Source.Camera)?.track : undefined,
        screen: p.getTrackPublication(Track.Source.ScreenShare)?.track,
      }
    }
    this.participants = [snapshot(room.localParticipant), ...[...room.remoteParticipants.values()].map(snapshot)]
    this.micOn = room.localParticipant.isMicrophoneEnabled
    this.cameraOn = room.localParticipant.isCameraEnabled
    this.screenOn = room.localParticipant.isScreenShareEnabled
  }
  private sound(join: boolean) {
    if (!this.prefs.sounds || this.outputMuted) return
    try {
      const ctx = new AudioContext()
      void ctx.resume()
      for (const [i, frequency] of (join ? [440, 554] : [554, 440]).entries()) {
        const osc = ctx.createOscillator(), gain = ctx.createGain(), at = ctx.currentTime + i * 0.08
        osc.frequency.value = frequency; osc.type = 'sine'
        gain.gain.setValueAtTime(0, at); gain.gain.linearRampToValueAtTime(0.045, at + 0.008)
        gain.gain.linearRampToValueAtTime(0, at + 0.08)
        osc.connect(gain); gain.connect(ctx.destination); osc.start(at); osc.stop(at + 0.08)
      }
      setTimeout(() => void ctx.close(), 250)
    } catch { /* sounds are optional */ }
  }
  async join(channel: Channel) {
    if (this.channel?.id === channel.id || this.joining === channel.id) return
    await this.leave()
    const generation = ++this.generation
    this.joining = channel.id; this.error = ''
    const prefs = this.prefs
    const room = new Room({
      adaptiveStream: true, dynacast: true,
      audioCaptureDefaults: { echoCancellation: true, noiseSuppression: true, autoGainControl: true, deviceId: prefs.microphone || undefined },
      videoCaptureDefaults: { deviceId: prefs.camera || undefined },
      audioOutput: { deviceId: prefs.speaker || 'default' },
    })
    try {
      const { url, token } = await api.post<CallToken>(`/calls/${channel.id}/token`)
      if (generation !== this.generation) return
      await room.connect(url, token, { autoSubscribe: false })
      if (generation !== this.generation) { await room.disconnect(); return }
      this.room = room; this.channel = channel
      for (const event of [RoomEvent.ParticipantConnected, RoomEvent.ParticipantDisconnected, RoomEvent.TrackSubscribed,
        RoomEvent.TrackUnsubscribed, RoomEvent.TrackMuted, RoomEvent.TrackUnmuted, RoomEvent.LocalTrackPublished,
        RoomEvent.LocalTrackUnpublished, RoomEvent.ActiveSpeakersChanged, RoomEvent.TrackPublished, RoomEvent.TrackUnpublished,
        RoomEvent.ParticipantNameChanged]) room.on(event, this.refresh)
      const attachAudio = (track: RemoteTrack, participant: Participant) => {
        if (track.kind !== Track.Kind.Audio || this.outputMuted || this.audio.has(track)) return
        // Hear a phone's shared media on the desktop, but never echo our own mic.
        if (accountId(participant.identity) === accountId(room.localParticipant.identity) && track.source === Track.Source.Microphone) return
        const el = track.attach()
        this.audio.set(track, el); document.body.append(el)
        el.play().catch(() => { if (!this.outputMuted) this.audioBlocked = true })
      }
      room.on(RoomEvent.TrackPublished, () => this.subscriptions())
      room.on(RoomEvent.TrackSubscribed, (track, _publication, participant) => attachAudio(track, participant))
      room.on(RoomEvent.TrackUnsubscribed, (track) => {
        track.detach().forEach((el) => el.remove()); this.audio.delete(track)
      })
      room.on(RoomEvent.AudioPlaybackStatusChanged, () => { this.audioBlocked = !this.outputMuted && !room.canPlaybackAudio })
      room.on(RoomEvent.Reconnecting, () => { this.reconnecting = true; this.setHeld(false) })
      room.on(RoomEvent.Reconnected, () => { this.reconnecting = false; this.refresh() })
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
      await room.localParticipant.setMicrophoneEnabled(prefs.mode === 'activity' && this.prefs.micOn)
      if (generation !== this.generation) { await room.disconnect(); return }
      if (prefs.cameraOn) await room.localParticipant.setCameraEnabled(true)
      this.refresh(); this.sound(true)
    } catch (err) {
      await room.disconnect()
      if (generation === this.generation) { this.clear(); this.report(err) }
    } finally {
      if (generation === this.generation) this.joining = null
    }
  }
  private clear() {
    for (const [track, el] of this.audio) { track.detach(); el.remove() }
    this.audio.clear(); this.room = null; this.channel = null; this.participants = []
    this.held = false; this.expanded = false; this.reconnecting = false; this.audioBlocked = false
    this.micOn = false; this.cameraOn = false; this.screenOn = false
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
  private applyMic() {
    const room = this.room
    this.micQueue = this.micQueue.then(async () => {
      if (!room || this.room !== room) return
      await room.localParticipant.setMicrophoneEnabled(this.prefs.mode === 'ptt' ? this.held : this.prefs.micOn)
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
    try { await room.localParticipant.setCameraEnabled(!room.localParticipant.isCameraEnabled); this.refresh(); this.save({ cameraOn: this.cameraOn }) } catch (err) { this.report(err) }
  }
  async toggleScreen() {
    const room = this.room
    if (!room) return
    try { await room.localParticipant.setScreenShareEnabled(!room.localParticipant.isScreenShareEnabled, { audio: true }); this.refresh() } catch (err) {
      if (!(err instanceof Error && ['NotAllowedError', 'AbortError'].includes(err.name))) this.report(err)
    }
  }
  async device(kind: MediaDeviceKind, id: string) {
    try {
      if (this.room) await this.room.switchActiveDevice(kind, id || 'default')
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
