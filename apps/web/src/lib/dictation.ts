import worklet from './dictation-audio.js?url&no-inline'
import workerUrl from './dictation.worker.ts?worker&url'
import wasmUrl from '../../node_modules/onnxruntime-web/dist/ort-wasm-simd-threaded.jsep.wasm?url'
import wasmModule from '../../node_modules/onnxruntime-web/dist/ort-wasm-simd-threaded.jsep.mjs?url'
import { modelCache, hasModel, removeModel } from './dictation-cache'
export { hasModel }
export async function remove() { await removeModel() }
export type DictationEvent = { type: 'loading' | 'listening' | 'stopped'; progress?: number } | { type: 'text'; text: string; final: boolean } | { type: 'error'; message: string }

export class Dictation {
  private urls: string[] = []
  private worker: Worker | undefined
  private stream: MediaStream | undefined
  private context: AudioContext | undefined
  private capture: AudioWorkletNode | undefined
  private audio = new Float32Array(0)
  private boundary = 0
  private lastSent = 0
  private busy = false
  private ending = false
  private disposed = false
  private timer: ReturnType<typeof setInterval> | undefined
  private inflight: { end: number; final: boolean } | undefined
  backend = ''
  constructor(private emit: (event: DictationEvent) => void) {}
  async start(microphone: string) {
    try {
      this.emit({ type: 'loading' })
      // Request permission in response to the tap; never hold a mic while downloading.
      this.stream = await navigator.mediaDevices.getUserMedia({ audio: { deviceId: microphone || undefined, echoCancellation: true, noiseSuppression: true, autoGainControl: true } })
      if (this.disposed) { this.release(); return }
      this.stream.getTracks().forEach(t => t.stop()); this.stream = undefined
      try { await this.load('auto') }
      catch {
        // A failed ONNX initialization poisons its module-level promise. Retry in a fresh worker.
        if (this.disposed) return
        this.worker?.terminate(); await this.load('wasm')
      }
      if (this.disposed) return
      this.worker!.onmessage = ({ data }) => {
        if (this.disposed) return
        if (data.type === 'error') { this.fail(new Error(data.message)); return }
        if (data.type !== 'text' || !this.inflight) return
        const { end, final } = this.inflight
        this.emit({ type: 'text', text: data.text, final })
        if (final) {
          const keepFrom = Math.max(0, end - 8000)
          this.audio = this.audio.slice(keepFrom); this.boundary = end - keepFrom; this.lastSent -= keepFrom
        }
        this.busy = false
        if (this.ending) this.pump()
      }
      this.worker!.onerror = () => this.fail(new Error('Dictation stopped. Your text is still here.'))
      this.stream = await navigator.mediaDevices.getUserMedia({ audio: { deviceId: microphone || undefined, echoCancellation: true, noiseSuppression: true, autoGainControl: true } })
      if (this.disposed) { this.release(); return }
      this.context = new AudioContext()
      await this.context.audioWorklet.addModule(await this.assetUrl(worklet, 'text/javascript'))
      if (this.disposed) return
      await this.context.resume()
      this.capture = new AudioWorkletNode(this.context, 'den-dictation')
      this.capture.port.onmessage = ({ data }: MessageEvent<Float32Array | { flush: true; audio: Float32Array }>) => {
        if (this.disposed) return
        const flushing = !(data instanceof Float32Array)
        if (this.ending && !flushing) return
        const samples = flushing ? data.audio : data
        const next = new Float32Array(this.audio.length + samples.length); next.set(this.audio); next.set(samples, this.audio.length); this.audio = next
        if (flushing) { this.release(); this.pump() }
      }
      this.context.createMediaStreamSource(this.stream).connect(this.capture)
      this.capture.connect(this.context.destination) // Processor emits silence, never microphone echo.
      this.emit({ type: 'listening' })
      this.timer = setInterval(() => this.pump(), 2000)
    } catch (error) { if (!this.disposed) this.fail(error) }
  }
  private async asset(url: string) {
    const key = 'runtime:' + url
    let response = await modelCache.match(key)
    if (!response) {
      response = await fetch(url)
      if (!response.ok) throw new Error('Dictation download failed')
      await modelCache.put(key, response.clone())
    }
    return response.arrayBuffer()
  }
  private async assetUrl(url: string, type: string) {
    // Bundled runtime modules resolve their fallback URLs relative to their original asset.
    const source = new TextDecoder().decode(await this.asset(url)).replaceAll('import.meta.url', JSON.stringify(new URL(url, location.href).href))
    const blob = URL.createObjectURL(new Blob([source], { type }))
    this.urls.push(blob); return blob
  }
  private async load(device: string) {
    const [source, mjs, wasm] = await Promise.all([this.assetUrl(workerUrl, 'text/javascript'), this.assetUrl(wasmModule, 'text/javascript'), this.asset(wasmUrl)])
    if (this.disposed) return
    this.worker = new Worker(source, { type: 'module' })
    await new Promise<void>((resolve, reject) => {
      this.worker!.onerror = () => reject(new Error('Dictation could not start on this device.'))
      this.worker!.onmessage = ({ data }) => {
        if (data.type === 'ready') { this.backend = data.backend; resolve() }
        else if (data.type === 'error') reject(new Error(data.message))
        else if (data.type === 'progress') this.emit({ type: 'loading', progress: data.progress })
      }
      this.worker!.postMessage({ type: 'load', device, mjs, wasm }, [wasm])
    })
  }
  private pump() {
    if (this.busy || this.disposed) return
    if (this.audio.length <= this.lastSent || this.audio.length <= this.boundary) {
      if (this.ending) { this.emit({ type: 'stopped' }); this.dispose() }
      return
    }
    const end = Math.min(this.audio.length, this.boundary + 128000)
    const audio = this.audio.slice(Math.max(0, this.boundary - 8000), end)
    this.inflight = { end, final: this.ending || end - this.boundary >= 128000 }
    this.lastSent = end; this.busy = true
    // Silence should not become Whisper's guessed subtitles.
    if (audio.reduce((s, n) => s + n * n, 0) / audio.length < 0.00001) {
      this.worker!.onmessage!({ data: { type: 'text', text: '' } } as MessageEvent)
    } else this.worker!.postMessage({ type: 'audio', audio }, [audio.buffer])
  }
  stop() {
    this.ending = true; clearInterval(this.timer)
    this.stream?.getTracks().forEach(t => t.stop())
    if (this.capture) this.capture.port.postMessage('flush')
    else { this.emit({ type: 'stopped' }); this.dispose() }
  }
  dispose() { this.disposed = true; this.release(); this.worker?.terminate(); this.worker = undefined; this.urls.forEach(url => URL.revokeObjectURL(url)); this.urls = []; this.audio = new Float32Array(0) }
  private release() { clearInterval(this.timer); this.capture?.disconnect(); this.stream?.getTracks().forEach(t => t.stop()); void this.context?.close(); this.context = undefined; this.stream = undefined }
  private fail(error: unknown) {
    const denied = error instanceof DOMException && ['NotAllowedError', 'PermissionDeniedError'].includes(error.name)
    this.emit({ type: 'error', message: denied ? 'Den needs the microphone for dictation. Allow it in Settings.' : 'Dictation could not start. Try again on this device.' })
    this.dispose()
  }
}
