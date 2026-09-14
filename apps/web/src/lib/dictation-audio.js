// AudioWorklet sends mono 16 kHz PCM only to the local transcription worker.
class DictationAudio extends AudioWorkletProcessor {
  constructor() {
    super()
    this.port.onmessage = () => { this.port.postMessage({ flush: true, audio: this.buffer.slice(0, this.length) }); this.length = 0 }
  }
  buffer = new Float32Array(3200)
  length = 0
  phase = 0
  sum = 0
  count = 0
  /** @param {Float32Array[][]} inputs */
  process(inputs) {
    for (const value of inputs[0]?.[0] || []) {
      this.sum += value; this.count++; this.phase += 16000
      if (this.phase >= sampleRate) {
        this.phase -= sampleRate
        this.buffer[this.length++] = this.sum / this.count
        this.sum = 0; this.count = 0
        if (this.length === this.buffer.length) {
          this.port.postMessage(this.buffer, [this.buffer.buffer])
          this.buffer = new Float32Array(3200); this.length = 0
        }
      }
    }
    return true
  }
}
registerProcessor('den-dictation', DictationAudio)
