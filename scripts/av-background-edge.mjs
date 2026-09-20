import assert from 'node:assert/strict'
import { existsSync } from 'node:fs'
import { chromium } from 'playwright-core'

const base = process.env.DEN_SMOKE_URL || 'http://localhost:5178'
const channel = process.env.DEN_SMOKE_CHANNEL || 'av-verification'
const browserPath = process.env.DEN_SMOKE_BROWSER || [
  '/usr/bin/chromium',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
].find(existsSync)
assert(browserPath, 'Set DEN_SMOKE_BROWSER to Chrome or Chromium')
assert(process.env.DEN_SMOKE_PASSWORD, 'Set DEN_SMOKE_PASSWORD')

const browser = await chromium.launch({
  executablePath: browserPath,
  headless: !process.env.DISPLAY,
  args: ['--no-sandbox', '--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream', '--autoplay-policy=no-user-gesture-required'],
})

async function until(check, label, timeout = 30_000) {
  const end = Date.now() + timeout
  while (Date.now() < end) {
    if (await check()) return
    await new Promise(resolve => setTimeout(resolve, 100))
  }
  throw Error(`Timed out: ${label}`)
}

async function login(context) {
  const page = await context.newPage()
  await page.goto(base)
  await page.getByLabel('Username', { exact: true }).fill('nicholas')
  await page.getByLabel('Password', { exact: true }).fill(process.env.DEN_SMOKE_PASSWORD)
  await page.getByRole('button', { name: 'Come in', exact: true }).click()
  await page.locator('a[title="Settings"]').waitFor()
  return page
}

async function voice(page) {
  await page.locator('a[title="Settings"]').click()
  await page.getByRole('link', { name: 'Voice', exact: true }).click()
  await page.getByRole('meter').waitFor()
}

async function join(page) {
  await page.locator('nav.side').getByRole('button', { name: `Join ${channel}`, exact: true }).click()
  await until(async () => (await callState(page)).state === 'connected', 'call connected')
}

async function callState(page) {
  return page.evaluate(async () => {
    const url = performance.getEntriesByType('resource').find(entry => new URL(entry.name).pathname === '/src/lib/call.svelte.ts').name
    const { call } = await import(url)
    const track = call.room?.localParticipant.getTrackPublication('camera')?.track
    return {
      state: call.room?.state,
      cameraOn: call.cameraOn,
      error: call.error,
      background: call.cameraSettings.background,
      processor: track?.getProcessor?.()?.name ?? null,
    }
  })
}

async function saveBackground(page, id, background) {
  await page.evaluate(async ({ id, background }) => {
    const url = performance.getEntriesByType('resource').find(entry => new URL(entry.name).pathname === '/src/lib/store.svelte.ts').name
    const { instances } = await import(url)
    const current = instances.active.voice.cameras[id] || { resolution: 'auto', frame_rate: 30, mirror: true, background: 'none', brightness: null, contrast: null, saturation: null }
    await instances.active.saveVoice({ microphones: {}, cameras: { [id]: { ...current, background } } })
  }, { id, background })
}

async function savedBackground(page, id) {
  return page.evaluate(async id => {
    const url = performance.getEntriesByType('resource').find(entry => new URL(entry.name).pathname === '/src/lib/store.svelte.ts').name
    const { instances } = await import(url)
    return instances.active.voice.cameras[id]?.background
  }, id)
}

const report = {}
let selectedCamera
try {
  // A MediaPipe-only failure retries the same capture without a processor, keeps
  // the call usable, announces the fallback, and clears only this camera's effect.
  {
    const context = await browser.newContext({ permissions: ['camera', 'microphone'] })
    const page = await login(context)
    await voice(page)
    await until(() => page.getByLabel('Camera preview', { exact: true }).evaluate(video => video.videoWidth > 0), 'setup preview')
    const picker = page.getByRole('combobox', { name: 'Camera', exact: true })
    selectedCamera = await picker.locator('option').evaluateAll(options => options.map(option => option.value).find(Boolean))
    assert(selectedCamera, 'synthetic camera has a stable device ID')
    await picker.selectOption(selectedCamera)
    await page.getByRole('radio', { name: 'Blur', exact: true }).check()
    await until(() => page.getByRole('radio', { name: 'Blur', exact: true }).isChecked(), 'blur saved for failure test')
    await page.getByRole('link', { name: 'Notifications', exact: true }).click()
    await page.route('**/blur/selfie_segmenter.tflite*', route => route.abort('failed'))
    await page.reload()
    await page.locator('a[title="Settings"]').waitFor()
    await join(page)
    await page.getByRole('button', { name: 'Turn camera on', exact: true }).click()
    await until(async () => {
      const state = await callState(page)
      return state.cameraOn && state.processor === null
    }, 'plain camera fallback')
    await page.getByRole('status').filter({ hasText: 'Background blur could not start' }).waitFor()
    assert.equal(await savedBackground(page, selectedCamera), 'none')
    report.modelFailure = { plainCamera: true, visibleNotice: true, failedEffectCleared: true }
    await page.getByRole('button', { name: 'Leave call', exact: true }).click()
    await page.unroute('**/blur/selfie_segmenter.tflite*')
    await saveBackground(page, selectedCamera, 'blur')
    await context.close()
  }

  // Permission/capture failure happens before the effect exists. The plain retry
  // fails too, so it must report permission and preserve the saved blur choice.
  {
    const context = await browser.newContext({ permissions: ['camera', 'microphone'] })
    await context.addInitScript(() => {
      const getUserMedia = navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices)
      window.__videoAttempts = 0
      navigator.mediaDevices.getUserMedia = async options => {
        if (options.video) {
          window.__videoAttempts++
          throw new DOMException('Camera denied for blur preference test', 'NotAllowedError')
        }
        return getUserMedia(options)
      }
    })
    const page = await context.newPage()
    await page.goto(base)
    await page.evaluate(id => localStorage.setItem('den.voice', JSON.stringify({ camera: id })), selectedCamera)
    await page.reload()
    await page.getByLabel('Username', { exact: true }).fill('nicholas')
    await page.getByLabel('Password', { exact: true }).fill(process.env.DEN_SMOKE_PASSWORD)
    await page.getByRole('button', { name: 'Come in', exact: true }).click()
    await page.locator('a[title="Settings"]').waitFor()
    await join(page)
    await page.getByRole('button', { name: 'Turn camera on', exact: true }).click()
    await page.getByRole('alert').filter({ hasText: 'Allow camera and microphone access' }).waitFor()
    assert.equal((await callState(page)).cameraOn, false)
    assert.equal(await savedBackground(page, selectedCamera), 'blur')
    assert.equal(await page.evaluate(() => window.__videoAttempts), 2, 'blur attempt and plain retry both reached capture')
    report.permissionFailure = { cameraOff: true, preferencePreserved: true, attempts: 2 }
    await page.getByRole('button', { name: 'Leave call', exact: true }).click()
    await context.close()
  }

  // A browser without insertable video transforms keeps the saved account value,
  // presents truthful disabled controls, and publishes plain video without loading
  // the processor chunk, model, or WASM.
  {
    const context = await browser.newContext({ permissions: ['camera', 'microphone'] })
    await context.addInitScript(() => {
      Object.defineProperty(window, 'MediaStreamTrackGenerator', { value: undefined })
      Object.defineProperty(window, 'MediaStreamTrackProcessor', { value: undefined })
    })
    const page = await login(context)
    await voice(page)
    await until(() => page.getByLabel('Camera preview', { exact: true }).evaluate(video => video.videoWidth > 0), 'unsupported preview')
    const cameraId = await page.getByLabel('Camera preview', { exact: true }).evaluate(video => video.srcObject.getVideoTracks()[0].getSettings().deviceId)
    await saveBackground(page, cameraId, 'blur')
    await saveBackground(page, 'default', 'blur')
    assert(await page.getByRole('radio', { name: 'Blur', exact: true }).isDisabled())
    assert(await page.getByRole('radio', { name: 'Light blur', exact: true }).isDisabled())
    assert(await page.getByRole('radio', { name: 'None', exact: true }).isChecked())
    await page.getByText('Background blur needs modern frame processing', { exact: false }).waitFor()
    await join(page)
    await page.getByRole('button', { name: 'Turn camera on', exact: true }).click()
    await until(async () => (await callState(page)).cameraOn, 'unsupported browser plain camera')
    const state = await callState(page)
    assert.equal(state.processor, null)
    assert.equal(await page.getByRole('button', { name: /background blur/i }).count(), 0)
    assert.equal(await savedBackground(page, cameraId), 'blur')
    assert.equal(await page.evaluate(() => performance.getEntriesByType('resource').some(entry => /track-processors|selfie_segmenter|vision_wasm/.test(entry.name))), false)
    report.unsupported = { plainCamera: true, disabledWithReason: true, preferencePreserved: true, processorAssetsLoaded: false }
    await page.getByRole('button', { name: 'Leave call', exact: true }).click()
    await context.close()
  }

  console.log(`Background edge smoke passed\n${JSON.stringify(report, null, 2)}`)
} finally {
  await browser.close()
}
