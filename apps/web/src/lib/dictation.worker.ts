import { env, WhisperTokenizer, WhisperProcessor, AutoFeatureExtractor, WhisperForConditionalGeneration, AutomaticSpeechRecognitionPipeline } from '@huggingface/transformers'
import { modelCache, model, revision, markReady } from './dictation-cache'

env.allowLocalModels = false
// Code/WASM ships with Den. Only model files are fetched, and never audio.
env.backends.onnx.wasm!.numThreads = 1
env.backends.onnx.wasm!.proxy = false
env.useBrowserCache = false
env.useCustomCache = true
env.customCache = modelCache
// v4 tokenizer discovery ignores revision and its HEAD request bypasses the cache.
// This pinned Whisper export has exactly these two tokenizer files.
async function tokenizerJSON(file: string) {
  const url = `https://huggingface.co/${model}/resolve/${revision}/${file}`
  let response = await modelCache.match(url)
  if (!response) {
    response = await fetch(url)
    if (!response.ok) throw new Error('Voice model download failed. Try again.')
    await modelCache.put(url, response.clone())
  }
  return response.json()
}
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
      // ORT's q8 MatMulNBits rewrite fails this export; unoptimized graphs pass GPU and WASM speech checks.
      // Load the known components directly: pipeline() performs uncached HEAD discovery against main.
      const [tokenizer, feature_extractor, weights] = await Promise.all([
        Promise.all(['tokenizer.json', 'tokenizer_config.json'].map(tokenizerJSON)).then(([json, config]) => new WhisperTokenizer(json, config)), AutoFeatureExtractor.from_pretrained(model, options),
        WhisperForConditionalGeneration.from_pretrained(model, { ...options, device: backend, session_options: { graphOptimizationLevel: 'disabled' } }),
      ])
      const processor = new WhisperProcessor({}, { tokenizer, feature_extractor }, '')
      transcriber = new AutomaticSpeechRecognitionPipeline({ task: 'automatic-speech-recognition', tokenizer, processor, model: weights })
      await markReady()
      self.postMessage({ type: 'ready', backend })
    } else if (data.type === 'audio' && transcriber) {
      const result = await transcriber(data.audio, { return_timestamps: false, max_new_tokens: 96 })
      self.postMessage({ type: 'text', text: Array.isArray(result) ? result[0]?.text || '' : result.text })
    }
  } catch (error) { self.postMessage({ type: 'error', message: error instanceof Error ? error.message : String(error) }) }
}
