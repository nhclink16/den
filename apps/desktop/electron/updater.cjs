const fs = require('node:fs')
const { Provider, resolveFiles } = require('electron-updater/out/providers/Provider')
// Same Minisign key as Tauri v0.2.x. Keep the private key in the existing CI secret.
// Releases must use legacy (non-prehashed) signatures: Electron's BoringSSL has no
// BLAKE2b, so a prehashed signature fails with "Digest method not supported".
const publicKey = 'RWQSGQ8mWZtpT8uMqfsU4tVijnXz1Vv/MBPgU0E8LPWi+tYk3OtmXI90'
const releaseRoot = 'https://github.com/nhclink16/den/releases/'
async function verifyManifest(bytes, signature, key = publicKey) {
  const { MinisignVerifier } = await import('@kaito-tokyo/minisign-verify')
  const verifier = await MinisignVerifier.create(key)
  if (!(await verifier.verify([bytes], signature)).ok) throw Error('Update signature verification failed')
  const info = JSON.parse(bytes.toString('utf8'))
  if (!/^\d+\.\d+\.\d+$/.test(info.version) || !Array.isArray(info.files) || !info.files.length) throw Error('Invalid update manifest')
  for (const file of info.files) {
    if (!/^[\w.-]+$/.test(file.url) || !/^[A-Za-z0-9+/]{86}==$/.test(file.sha512) || !Number.isSafeInteger(file.size) || file.size <= 0) throw Error('Invalid update file')
  }
  return info
}
class SignedProvider extends Provider {
  constructor(_options, _updater, runtime) { super({ ...runtime, isUseMultipleRangeRequest: false }) }
  async getLatestVersion() {
    const suffix = process.platform === 'darwin' ? 'darwin-universal' : `${process.platform}-${process.arch}`
    const url = `${releaseRoot}latest/download/electron-${suffix}.json`
    const response = await fetch(url, { signal: AbortSignal.timeout(30000) })
    if (!response.ok) throw Error('No signed Electron update manifest is published yet')
    // A release racing these requests fails signature verification and retries next check.
    const signature = await fetch(url + '.sig', { signal: AbortSignal.timeout(30000) })
    if (!signature.ok) throw Error('Missing update signature')
    return verifyManifest(Buffer.from(await response.arrayBuffer()), Buffer.from((await signature.text()).trim(), 'base64').toString('utf8'))
  }
  resolveFiles(info) { return resolveFiles(info, new URL(`${releaseRoot}download/v${info.version}/`)) }
}
// Updates run unattended, so keep a small log to diagnose the next silent failure.
function fileLog(file) {
  return message => {
    try {
      if (fs.statSync(file, { throwIfNoEntry: false })?.size > 256 * 1024) fs.renameSync(file, file + '.old')
      fs.appendFileSync(file, `${new Date().toISOString()} ${message}\n`)
    } catch { /* logging must never break updates */ }
  }
}
function updater(logFile) {
  const { autoUpdater } = require('electron-updater')
  const log = fileLog(logFile)
  autoUpdater.logger = { info: log, warn: m => log(`warn ${m}`), error: m => log(`error ${m}`) }
  autoUpdater.autoDownload = false
  autoUpdater.autoInstallOnAppQuit = false
  autoUpdater.allowDowngrade = false
  autoUpdater.disableDifferentialDownload = true
  autoUpdater.disableWebInstaller = true
  autoUpdater.setFeedURL({ provider: 'custom', updateProvider: SignedProvider })
  let ready = false, checking
  autoUpdater.on('error', e => log(`error ${e?.stack || e}`))
  return {
    check() {
      if (ready) return Promise.resolve(true)
      if (!checking) checking = (async () => {
        const result = await autoUpdater.checkForUpdates()
        if (!result?.isUpdateAvailable) return false
        await autoUpdater.downloadUpdate(); ready = true; return true
      })().finally(() => { checking = null })
      return checking
    },
    restart() { if (!ready) throw Error('No verified update is ready'); autoUpdater.quitAndInstall(false, true) },
  }
}
module.exports = { updater, SignedProvider, verifyManifest, publicKey }
