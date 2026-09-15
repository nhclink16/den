// Run per platform after electron-builder, then sign the result with the existing Tauri signer.
const fs = require('node:fs')
const path = require('node:path')
const { createHash } = require('node:crypto')
const directory = process.argv[2] || 'apps/desktop/dist'
const platform = process.argv[3] || process.platform
const arch = platform === 'darwin' ? 'universal' : (process.argv[4] || process.arch)
const version = require('../apps/desktop/package.json').version
const extensions = { linux: ['.AppImage', '.deb'], win32: ['.exe'], darwin: ['.zip'] }[platform]
if (!extensions) throw Error('Unsupported updater platform')
const files = fs.readdirSync(directory).filter(name => extensions.some(ext => name.endsWith(ext))).map(name => {
  const bytes = fs.readFileSync(path.join(directory, name))
  return { url: name, sha512: createHash('sha512').update(bytes).digest('base64'), size: bytes.length }
})
if (files.length !== extensions.length) throw Error('Expected exactly one updater artifact per package type')
fs.writeFileSync(path.join(directory, `electron-${platform}-${arch}.json`), JSON.stringify({ version, files, releaseDate: new Date().toISOString() }, null, 2) + '\n')
