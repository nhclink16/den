import { LocalVideoTrack, Track, type Room, type Participant, type LocalTrack } from 'livekit-client'

export type Share = { name: string; label: string; track?: Track }
// LiveKit 2.15's first-share default is 1080p/15 at 2.5 Mbps. Additional
// captures get 750 kbps/15; Smooth lifts only the frame-rate cap to 30.
export class Shares {
  private groups = new Map<string, LocalTrack[]>()
  private pending = false
  private epoch = 0
  private next = 1
  constructor(private room: () => Room | null, private changed: () => void) {}
  list(p: Participant): Share[] {
    return [...p.videoTrackPublications.values()].filter(t => t.source === Track.Source.ScreenShare).map(t => ({
      name: t.trackName, track: t.track,
      label: (p.attributes[`den.share.${t.trackName}`] || t.trackName || 'Screen').slice(0, 28),
    }))
  }
  clear() { ++this.epoch; this.groups.clear(); this.pending = false; this.next = 1 }
  async add() {
    const room = this.room(), epoch = this.epoch
    if (!room || this.pending || this.groups.size >= 3) return
    this.pending = true
    let tracks: LocalTrack[] = [], name = ''
    try {
      const extra = this.groups.size > 0
      // Call capture before awaiting anything else: the picker needs user activation.
      tracks = await room.localParticipant.createScreenTracks({ audio: true, ...(extra ? { resolution: { width: 1920, height: 1080, frameRate: 15 } } : {}) })
      if (this.room() !== room || epoch !== this.epoch) { tracks.forEach(t => t.stop()); return }
      const n = this.next++
      name = `screen-${n}`; this.groups.set(name, tracks)
      const video = tracks.find(t => t.kind === Track.Kind.Video)!
      video.mediaStreamTrack.addEventListener('ended', () => { if (this.room() === room && epoch === this.epoch) void this.stop(name).catch(() => {}) }, { once: true })
      await room.localParticipant.setAttributes({ [`den.share.${name}`]: video.mediaStreamTrack.label.slice(0, 28) || `Screen ${n}` })
      for (const track of tracks) {
        if (this.room() !== room || epoch !== this.epoch || !this.groups.has(name) || video.mediaStreamTrack.readyState === 'ended') { tracks.forEach(t => t.stop()); return }
        await room.localParticipant.publishTrack(track, {
          name: track.kind === Track.Kind.Video ? name : `${name}-audio`, stream: name,
          degradationPreference: 'balanced',
          ...(extra && track.kind === Track.Kind.Video ? { screenShareEncoding: { maxBitrate: 750_000, maxFramerate: 15 } } : {}),
        })
      }
      this.changed()
    } catch (error) {
      if (this.room() === room && epoch === this.epoch && name) await this.stop(name)
      tracks.forEach(t => t.stop())
      throw error
    } finally { if (epoch === this.epoch) this.pending = false }
  }
  async stop(name?: string) {
    const room = this.room()
    if (!room) return
    for (const [key, tracks] of [...this.groups]) {
      if (name && key !== name) continue
      this.groups.delete(key)
      await Promise.all(tracks.map(t => room.localParticipant.unpublishTrack(t)))
      await room.localParticipant.setAttributes({ [`den.share.${key}`]: '' })
    }
    this.changed()
  }
  async quality(name: string, mode: 'Smooth' | 'Sharp') {
    const room = this.room(), video = this.groups.get(name)?.find(t => t instanceof LocalVideoTrack)
    if (!room || !(video instanceof LocalVideoTrack)) return
    const pub = room.localParticipant.getTrackPublicationByName(name)
    const preference = mode === 'Smooth' ? 'maintain-framerate' : 'maintain-resolution'
    if (pub?.options) {
      pub.options.degradationPreference = preference
      pub.options.screenShareEncoding = { ...pub.options.screenShareEncoding, maxBitrate: pub.options.screenShareEncoding?.maxBitrate ?? 2_500_000, maxFramerate: mode === 'Smooth' ? 30 : 15 }
    }
    await video.mediaStreamTrack.applyConstraints({ frameRate: mode === 'Smooth' ? 30 : 15 })
    // Update this sender in place: no republish, new tile, or lost paired audio.
    const sender = video.sender
    if (sender) {
      const params = sender.getParameters(); params.degradationPreference = preference
      for (const encoding of params.encodings) encoding.maxFramerate = mode === 'Smooth' ? 30 : 15
      await sender.setParameters(params)
    }
    await video.setDegradationPreference(preference)
  }
}
