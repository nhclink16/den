const { test } = require('node:test')
const assert = require('node:assert/strict')
const { classify, parseXprop, parseLsappinfo, parseWindows } = require('../electron/activity.cjs')

const self = { pid: 42, exe: '/opt/Den/den' }

test('games are playing, apps are using, shells and Den itself are neither', () => {
  assert.deepEqual(classify({ name: 'java', title: 'Minecraft* 1.21.1 - Multiplayer', path: '/usr/lib/jvm/java-21/bin/java', hint: '' }, self), { kind: 'playing', name: 'Minecraft' })
  assert.deepEqual(classify({ name: 'java', title: '', path: '/usr/bin/java', hint: 'java -Djava.library.path=/home/n/.minecraft/natives' }, self), { kind: 'playing', name: 'Minecraft' })
  assert.deepEqual(classify({ name: 'eldenring.exe', title: 'ELDEN RING', path: 'C:\\Program Files (x86)\\Steam\\steamapps\\common\\ELDEN RING\\Game\\eldenring.exe' }, self), { kind: 'playing', name: 'ELDEN RING' })
  assert.deepEqual(classify({ name: 'code', title: 'secret-plan.md - Visual Studio Code', path: '/usr/share/code/code' }, self), { kind: 'using', name: 'Visual Studio Code' })
  assert.deepEqual(classify({ name: 'firefox', title: 'Bank login', path: '/usr/lib/firefox/firefox' }, self), { kind: 'using', name: 'Firefox' })
  assert.deepEqual(classify({ name: 'Blender', title: '', path: '' }, self), { kind: 'using', name: 'Blender' })
  for (const shell of ['explorer', 'Finder', 'gnome-shell', 'LockApp']) assert.equal(classify({ name: shell, title: '', path: '' }, self), null, shell)
  assert.equal(classify({ name: 'den', title: '', path: '/opt/Den/den', pid: 42 }, self), 'self')
  assert.equal(classify(null, self), null)
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

  assert.deepEqual(parseWindows('chrome\tC:\\Program Files\\Google\\Chrome\\chrome.exe\tInbox\tGoogle Chrome\t9001\r\n'),
    { name: 'Google Chrome', path: 'C:\\Program Files\\Google\\Chrome\\chrome.exe', title: 'Inbox', hint: 'C:\\Program Files\\Google\\Chrome\\chrome.exe', pid: 9001 })
  assert.equal(parseWindows(''), null)
})
