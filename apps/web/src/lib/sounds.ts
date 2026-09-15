import { fetchBytes } from './api'
import type { SoundEvent, SoundState, SoundPack } from './types'

export const soundEvents: { id: SoundEvent; name: string; description: string }[] = [
  { id: 'message', name: 'Message', description: 'A message arrives in a room you follow.' },
  { id: 'mention', name: 'Mention', description: 'Someone mentions you outside the room you are viewing.' },
  { id: 'dm', name: 'Direct message', description: 'A direct message arrives outside the conversation you are viewing.' },
  { id: 'call_join', name: 'You joined', description: 'You connect to a call.' },
  { id: 'call_leave', name: 'You left', description: 'You leave or disconnect from a call.' },
  { id: 'someone_joined', name: 'Someone joined', description: 'Another person joins your call.' },
  { id: 'someone_left', name: 'Someone left', description: 'Another person leaves your call.' },
  { id: 'screen_share_started', name: 'Screen share', description: 'A screen share starts in your call.' },
  { id: 'terminal_bell', name: 'Terminal bell', description: 'An open live terminal rings its bell.' },
  { id: 'upload_complete', name: 'Upload complete', description: 'Your file finishes uploading.' },
  { id: 'error', name: 'Error', description: 'A request or call action fails.' },
]
type Source = { origin: string; sounds: SoundState | null }
let context: AudioContext | undefined
const cache = new Map<string, Promise<AudioBuffer>>()
const active = new Set<AudioBufferSourceNode>()
let sequence = 0
function ctx() { return context ||= new AudioContext() }
export const sounds = {
  unlock() { void ctx().resume().catch(() => {}) },
  stop() { sequence++; for (const node of active) { try { node.stop() } catch { /* already ended */ } } },
  async url(origin: string, url: string, volume = .7) {
    const run = sequence, audio = ctx(); await audio.resume()
    if (!volume) return
    const key = `${origin}:${url}`
    if (!cache.has(key)) {
      const bytes = url.startsWith('/sounds/') ? fetch(url).then(r => { if (!r.ok) throw Error('Sound unavailable'); return r.arrayBuffer() }) : fetchBytes(origin, url)
      cache.set(key, bytes.then(data => audio.decodeAudioData(data)).catch(err => { cache.delete(key); throw err }))
      if (cache.size > 64) cache.delete(cache.keys().next().value!)
    }
    const buffer = await cache.get(key)!
    if (run !== sequence) return
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) for (const node of active) { try { node.stop() } catch { /* ended */ } }
    const node = audio.createBufferSource(), gain = audio.createGain()
    node.buffer = buffer; gain.gain.value = volume; node.connect(gain); gain.connect(audio.destination)
    active.add(node)
    await new Promise<void>(resolve => { node.onended = () => { active.delete(node); node.disconnect(); gain.disconnect(); resolve() }; node.start() })
  },
  async play(event: SoundEvent, source: Source) {
    const state = source.sounds, resolved = state?.resolved[event]
    if (!state || !resolved?.url) return
    const volume = state.preferences.master_volume * (state.preferences.volumes[event] ?? 100) / 10000
    try { await this.url(source.origin, resolved.url, volume) } catch { /* Permissions or unavailable media must not interrupt the action. */ }
  },
  async test(source: Source, onEvent: (event: SoundEvent | null) => void, pack?: SoundPack, uploadId?: string) {
    this.stop(); this.unlock(); const run = sequence
    try {
      const events = pack ? soundEvents.filter(e => e.id in pack.sounds) : soundEvents
      for (const [i,event] of events.entries()) {
        if (run !== sequence) break
        onEvent(event.id)
        if (pack) {
          const ref = pack.sounds[event.id]
          if (ref?.type === 'builtin') await this.url(source.origin, `/sounds/${ref.name}.wav`)
          else if (ref?.type === 'upload' && uploadId) await this.url(source.origin, `/uploads/${uploadId}/sounds/${ref.id}`)
        } else await this.play(event.id, source)
        if (i < events.length - 1 && run === sequence) await new Promise(r => setTimeout(r, 1000))
      }
    } finally { onEvent(null) }
  },
}
// Unlock with the first real gesture, without fetching any audio.
window.addEventListener('pointerdown', () => sounds.unlock(), { once: true })
window.addEventListener('keydown', () => sounds.unlock(), { once: true })
