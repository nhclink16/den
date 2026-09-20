import assert from 'node:assert/strict'
import { existsSync } from 'node:fs'
import { chromium } from 'playwright-core'

const base = process.env.DEN_SMOKE_URL || 'http://127.0.0.1:5188'
const browserPath = process.env.DEN_SMOKE_BROWSER || [
  '/usr/bin/chromium',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
].find(existsSync)
assert(browserPath, 'Set DEN_SMOKE_BROWSER to Chrome or Chromium')

const browser = await chromium.launch({
  executablePath: browserPath,
  headless: !process.env.DISPLAY,
  args: [
    '--no-sandbox',
    '--use-fake-ui-for-media-stream',
    '--use-fake-device-for-media-stream',
    '--autoplay-policy=no-user-gesture-required',
    ...(process.env.DEN_SMOKE_VULKAN === '1' ? ['--use-angle=vulkan', '--enable-gpu', '--ignore-gpu-blocklist'] : []),
  ],
})
const report = {}

async function pageWithCamera(init) {
  const context = await browser.newContext({ permissions: ['camera'] })
  if (init) await context.addInitScript(init)
  const page = await context.newPage()
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto(base)
  return { context, page, errors }
}

try {
  {
    const { context, page, errors } = await pageWithCamera()
    report.modern = await page.evaluate(async () => {
      const av = await import('/src/lib/av.ts?smoke=modern')
      const source = (await navigator.mediaDevices.getUserMedia({ video: true })).getVideoTracks()[0]
      let background = 'light_blur'
      const rates = []
      const blur = new av.CameraBlur(() => 30, () => background, rate => rates.push(rate))
      const element = await av.startPreviewBlur(blur, source)
      const output = blur.processedTrack
      const video = document.createElement('video')
      video.muted = true
      video.srcObject = new MediaStream([output])
      document.body.append(video)
      await video.play()
      const deadline = performance.now() + 15_000
      while (!video.videoWidth && performance.now() < deadline) await new Promise(resolve => setTimeout(resolve, 100))
      const assets = performance.getEntriesByType('resource').map(entry => entry.name).filter(name => /(?:\/blur\/|jsdelivr|storage\.googleapis)/.test(name))
      const result = {
        supported: av.blurSupported(),
        lightRadius: av.backgroundBlurRadius('light_blur'),
        fullRadius: av.backgroundBlurRadius('blur'),
        initialRadius: blur.inner.transformer.options.blurRadius,
        dimensions: [video.videoWidth, video.videoHeight],
        outputState: output?.readyState,
        assets,
        rates,
      }
      background = 'blur'
      await blur.setBackground(background)
      result.switchedRadius = blur.inner.transformer.options.blurRadius
      await blur.destroy()
      element.srcObject = null
      source.stop()
      return { ...result, cleaned: { source: source.readyState, output: output?.readyState } }
    })
    assert.equal(report.modern.supported, true)
    assert.deepEqual(report.modern.dimensions, [640, 480])
    assert.equal(report.modern.outputState, 'live')
    assert(report.modern.lightRadius < report.modern.fullRadius)
    assert.equal(report.modern.initialRadius, report.modern.lightRadius)
    assert.equal(report.modern.switchedRadius, report.modern.fullRadius)
    assert(report.modern.assets.some(url => new URL(url).href.endsWith('/blur/selfie_segmenter.tflite?v=191ac952')))
    assert(report.modern.assets.some(url => new URL(url).pathname.endsWith('/blur/wasm-0.10.14/vision_wasm_internal.wasm')))
    assert(report.modern.assets.every(url => new URL(url).origin === new URL(base).origin), `blur fetched an external asset: ${report.modern.assets.join(', ')}`)
    assert.deepEqual(report.modern.cleaned, { source: 'ended', output: 'ended' })
    assert.deepEqual(errors, [])
    await context.close()
  }

  {
    const { context, page, errors } = await pageWithCamera()
    report.strain = await page.evaluate(async () => {
      const av = await import('/src/lib/av.ts?smoke=strain')
      const rates = []
      const blur = new av.CameraBlur(() => 60, () => 'blur', rate => rates.push(rate))
      for (const duration of [40, 50, 70]) {
        for (let frame = 0; frame < 60; frame++) blur.measure(duration)
      }
      return { initialCaps: [av.blurCaptureRate(60), av.blurCaptureRate(24)], rates, finalRate: blur.rate }
    })
    assert.deepEqual(report.strain, { initialCaps: [30, 24], rates: [24, 15, null], finalRate: 15 })
    assert.deepEqual(errors, [])
    await context.close()
  }

  {
    const { context, page, errors } = await pageWithCamera()
    report.stuckTeardown = await page.evaluate(async () => {
      const av = await import('/src/lib/av.ts?smoke=stuck-teardown')
      const source = (await navigator.mediaDevices.getUserMedia({ video: true })).getVideoTracks()[0]
      const blur = new av.CameraBlur(() => 30, () => 'blur', () => {})
      const element = await av.startPreviewBlur(blur, source)
      const output = blur.processedTrack
      const input = blur.input
      Object.defineProperty(blur.inner.processor, 'writableControl', {
        configurable: true,
        value: { close: () => new Promise(() => {}) },
      })
      const started = performance.now()
      await blur.destroy()
      const result = {
        elapsed: performance.now() - started,
        source: source.readyState,
        input: input?.readyState,
        output: output?.readyState,
        canvases: document.querySelectorAll('canvas[data-livekit-processor]').length,
      }
      element.srcObject = null
      source.stop()
      return result
    })
    assert(report.stuckTeardown.elapsed < 1_500, `stuck teardown took ${report.stuckTeardown.elapsed} ms`)
    assert.equal(report.stuckTeardown.source, 'live')
    assert.equal(report.stuckTeardown.input, 'ended')
    assert.equal(report.stuckTeardown.output, 'ended')
    assert.equal(report.stuckTeardown.canvases, 0)
    assert.deepEqual(errors, [])
    await context.close()
  }

  {
    const { context, page, errors } = await pageWithCamera()
    await page.route('**/blur/selfie_segmenter.tflite*', async route => {
      await new Promise(resolve => setTimeout(resolve, 1_200))
      await route.continue()
    })
    report.cancelled = await page.evaluate(async () => {
      const av = await import('/src/lib/av.ts?smoke=cancelled')
      const source = (await navigator.mediaDevices.getUserMedia({ video: true })).getVideoTracks()[0]
      const blur = new av.CameraBlur(() => 30, () => 'blur', () => {})
      const starting = av.startPreviewBlur(blur, source)
      await new Promise(resolve => setTimeout(resolve, 50))
      await blur.destroy()
      const element = await starting
      await new Promise(resolve => setTimeout(resolve, 100))
      const result = {
        source: source.readyState,
        output: blur.processedTrack?.readyState ?? null,
        canvases: document.querySelectorAll('canvas[data-livekit-processor]').length,
      }
      element.srcObject = null
      source.stop()
      return result
    })
    assert.deepEqual(report.cancelled, { source: 'live', output: null, canvases: 0 })
    assert.deepEqual(errors, [])
    await context.close()
  }

  {
    const { context, page, errors } = await pageWithCamera()
    await page.route('**/blur/selfie_segmenter.tflite*', route => route.abort('failed'))
    report.failedModel = await page.evaluate(async () => {
      const av = await import('/src/lib/av.ts?smoke=failed-model')
      const source = (await navigator.mediaDevices.getUserMedia({ video: true })).getVideoTracks()[0]
      const blur = new av.CameraBlur(() => 30, () => 'blur', () => {})
      let failure = ''
      try { await av.startPreviewBlur(blur, source) } catch (error) { failure = error instanceof Error ? error.message : String(error) }
      const result = {
        failure,
        source: source.readyState,
        output: blur.processedTrack?.readyState ?? null,
        canvases: document.querySelectorAll('canvas[data-livekit-processor]').length,
      }
      source.stop()
      return result
    })
    assert(report.failedModel.failure)
    assert.equal(report.failedModel.source, 'live')
    assert.equal(report.failedModel.output, null)
    assert.equal(report.failedModel.canvases, 0)
    assert.deepEqual(errors, [])
    await context.close()
  }

  {
    const { context, page, errors } = await pageWithCamera(() => {
      Object.defineProperty(window, 'MediaStreamTrackProcessor', { value: undefined })
      Object.defineProperty(window, 'MediaStreamTrackGenerator', { value: undefined })
    })
    report.unsupported = await page.evaluate(async () => {
      const av = await import('/src/lib/av.ts?smoke=unsupported')
      return { supported: av.blurSupported(), accelerated: av.blurAccelerated() }
    })
    assert.deepEqual(report.unsupported, { supported: false, accelerated: false })
    assert.deepEqual(errors, [])
    await context.close()
  }

  console.log(`Background blur smoke passed\n${JSON.stringify(report, null, 2)}`)
} finally {
  await browser.close()
}
