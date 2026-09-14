import { env, pipeline, type AutomaticSpeechRecognitionPipeline } from '@huggingface/transformers'
import { modelCache, model, revision, markReady } from './dictation-cache'

env.allowLocalModels = false
// Code/WASM ships with Den. Only model files are fetched, and never audio.
env.backends.onnx.wasm!.numThreads = 1
env.backends.onnx.wasm!.proxy = false
env.useBrowserCache = false
env.useCustomCache = true
env.customCache = modelCache
let transcriber: AutomaticSpeechRecognitionPipeline | undefined
let backend: 'webgpu' | 'wasm' = 'wasm'
self.onmessage = async ({ data }) => {
  try {
    if (data.type === 'load') {
      env.backends.onnx.wasm!.wasmPaths = { mjs: data.mjs }
      env.backends.onnx.wasm!.wasmBinary = data.wasm
      const options = { dtype: 'q8' as const, revision, progress_callback: (p: { status: string; progress?: number }) => {
        if (p.status === 'progress') self.postMessage({ type: 'progress', progress: p.progress })
      } }
      const gpu = (navigator as Navigator & { gpu?: { requestAdapter(): Promise<unknown> } }).gpu
      backend = data.device !== 'wasm' && await gpu?.requestAdapter().catch(() => null) ? 'webgpu' : 'wasm'
      transcriber = await pipeline<'automatic-speech-recognition'>('automatic-speech-recognition', model, { ...options, device: backend })
      await markReady()
      self.postMessage({ type: 'ready', backend })
    } else if (data.type === 'audio' && transcriber) {
      const result = await transcriber(data.audio, { return_timestamps: false, max_new_tokens: 96 })
      self.postMessage({ type: 'text', text: Array.isArray(result) ? result[0]?.text || '' : result.text })
    }
  } catch (error) { self.postMessage({ type: 'error', message: error instanceof Error ? error.message : String(error) }) }
}
