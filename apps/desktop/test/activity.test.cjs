const { test } = require('node:test')
const assert = require('node:assert/strict')
const { spawn } = require('node:child_process')
const { classify, parseXprop, parseLsappinfo, parseWindows, lineReader } = require('../electron/activity.cjs')

const self = { pid: 42, exe: '/opt/Den/den' }

test('games are playing, apps are using, shells and Den itself are neither', () => {
  assert.deepEqual(classify({ name: 'java', title: 'Minecraft* 1.21.1 - Multiplayer', path: '/usr/lib/jvm/java-21/bin/java', hint: 'java -cp … net.minecraft.client.main.Main --gameDir /home/n/.local/share/PrismLauncher/instances/fabric' }, self), { kind: 'playing', name: 'Minecraft' })
  assert.deepEqual(classify({ name: 'javaw', process: 'javaw', title: '', path: 'C:\\Users\\n\\AppData\\Local\\Packages\\Microsoft.4297127D64EC6_8wekyb3d8bbwe\\LocalCache\\Local\\runtime\\java-runtime-delta\\bin\\javaw.exe', hint: 'javaw.exe -cp …\\.minecraft\\libraries net.minecraft.client.main.Main' }, self), { kind: 'playing', name: 'Minecraft' })
  assert.deepEqual(classify({ name: 'Minecraft', process: 'Minecraft.Windows', title: 'Minecraft', path: 'C:\\Program Files\\WindowsApps\\Minecraft.Windows.exe' }, self), { kind: 'playing', name: 'Minecraft' })
  assert.deepEqual(classify({ name: 'java', title: '', path: '/usr/bin/java', hint: 'java -Djava.library.path=/home/n/.minecraft/natives' }, self), { kind: 'playing', name: 'Minecraft' })
  assert.deepEqual(classify({ name: 'eldenring.exe', title: 'ELDEN RING', path: 'C:\\Program Files (x86)\\Steam\\steamapps\\common\\ELDEN RING\\Game\\eldenring.exe' }, self), { kind: 'playing', name: 'ELDEN RING' })
  assert.deepEqual(classify({ name: 'code', title: 'secret-plan.md - Visual Studio Code', path: '/usr/share/code/code' }, self), { kind: 'using', name: 'Visual Studio Code' })
  assert.deepEqual(classify({ name: 'firefox', title: 'Bank login', path: '/usr/lib/firefox/firefox' }, self), { kind: 'using', name: 'Firefox' })
  assert.deepEqual(classify({ name: 'Blender', title: '', path: '' }, self), { kind: 'using', name: 'Blender' })
  for (const shell of ['explorer', 'Finder', 'gnome-shell', 'LockApp']) assert.equal(classify({ name: shell, title: '', path: '' }, self), null, shell)
  assert.equal(classify({ name: 'den', title: '', path: '/opt/Den/den', pid: 42 }, self), 'self')
  assert.equal(classify(null, self), null)
})

test('talking about Minecraft is not playing it', () => {
  assert.deepEqual(classify({ name: 'Google Chrome', process: 'chrome', title: 'Redstone clock - Minecraft Wiki - Google Chrome', path: 'C:\\Program Files\\Google\\Chrome\\chrome.exe', hint: 'C:\\Program Files\\Google\\Chrome\\chrome.exe --type=browser' }, self), { kind: 'using', name: 'Google Chrome' })
  assert.deepEqual(classify({ name: 'code', title: 'server.properties - minecraft', path: '/usr/share/code/code', hint: '/usr/share/code/code /home/n/minecraft/server' }, self), { kind: 'using', name: 'Visual Studio Code' })
  assert.deepEqual(classify({ name: 'java', title: '', path: '/usr/bin/java', hint: 'java -jar /opt/tools/gradle.jar build' }, self), { kind: 'using', name: 'Java' })
})

test('a window title never becomes the shared name', () => {
  const shared = classify({ name: 'Slack', title: 'DM with a coworker about layoffs', path: '/usr/lib/slack/slack' }, self)
  assert.equal(JSON.stringify(shared).includes('layoffs'), false)
})

test('platform output parses into a focused window', () => {
  const root = '_NET_ACTIVE_WINDOW(WINDOW): window id # 0x4a00007\n'
  const props = '_NET_WM_PID(CARDINAL) = 3141\nWM_CLASS(STRING) = "code", "Code"\n_NET_WM_NAME(UTF8_STRING) = "den - Visual Studio Code"\n'
  assert.deepEqual(parseXprop(root, props), { id: '0x4a00007', pid: 3141, name: 'Code', title: 'den - Visual Studio Code' })
  assert.equal(parseXprop('_NET_ACTIVE_WINDOW(WINDOW): window id # 0x0\n', ''), null, 'no window focused')
  assert.equal(parseXprop('_NET_ACTIVE_WINDOW(CARDINAL) = 2097164\n', props)?.id, '0x20000c', 'set as a plain number')

  const mac = '"LSDisplayName"="Minecraft"\n"LSBundlePath"="/Applications/Minecraft.app"\n"pid" = 812\n'
  assert.deepEqual(parseLsappinfo(mac), { name: 'Minecraft', path: '/Applications/Minecraft.app', pid: 812 })
  assert.equal(parseLsappinfo(''), null)

  assert.deepEqual(parseWindows('chrome\tC:\\Program Files\\Google\\Chrome\\chrome.exe\t\tGoogle Chrome\t9001\t"chrome.exe" --flag\r\n'),
    { name: 'Google Chrome', process: 'chrome', path: 'C:\\Program Files\\Google\\Chrome\\chrome.exe', title: '', hint: 'C:\\Program Files\\Google\\Chrome\\chrome.exe "chrome.exe" --flag', pid: 9001 })
  assert.equal(parseWindows(''), null)
})

// Stands in for the long-lived PowerShell: answers each request line, optionally
// dying after the first answer or never answering at all.
const helper = mode => () => spawn(process.execPath, ['-e', `
  let n = 0
  process.stdin.on('data', () => {
    n++
    if (${JSON.stringify(mode)} === 'hang') return
    process.stdout.write('answer ' + n + '\\n')
    if (${JSON.stringify(mode)} === 'die') process.exit(0)
  })`], { stdio: ['pipe', 'pipe', 'ignore'] })

test('one helper answers every sample', async () => {
  const reader = lineReader(helper('ok'))
  try {
    assert.equal(await reader.ask(), 'answer 1')
    assert.equal(await reader.ask(), 'answer 2', 'the same process, not a new one per sample')
  } finally { reader.stop() }
})

test('a helper that exits or hangs is replaced on the next sample', async () => {
  const dying = lineReader(helper('die'))
  try {
    assert.equal(await dying.ask(), 'answer 1')
    await new Promise(r => setTimeout(r, 100))
    assert.equal(await dying.ask(), 'answer 1', 'a fresh helper after the first exited')
  } finally { dying.stop() }
  const hung = lineReader(helper('hang'), 300)
  try { assert.equal(await hung.ask(), '', 'a timeout reads as nothing detected') } finally { hung.stop() }
})
