// What you're playing or using, for your Den profile. Only an app's name leaves
// this machine. Window titles and command lines are read locally, and only to
// recognise a game, because they carry documents, chats and URLs.
const { execFile, spawn } = require('node:child_process')
const fs = require('node:fs')
const path = require('node:path')

const run = (command, args) => new Promise(resolve =>
  execFile(command, args, { timeout: 4000, windowsHide: true, maxBuffer: 256 * 1024 }, (error, stdout) => resolve(error ? '' : String(stdout))))

// --- reading the focused window, per platform --------------------------------

/** Window managers print `window id # 0x…`; some set it as a plain CARDINAL. */
function activeId(root) {
  const raw = root.match(/_NET_ACTIVE_WINDOW\([A-Z]+\)\s*[:=]\s*(?:window id # )?(0x[0-9a-f]+|\d+)/i)?.[1]
  const value = raw ? Number(raw) : 0
  return value ? `0x${value.toString(16)}` : null
}
function parseXprop(root, props) {
  const id = activeId(root)
  if (!id) return null
  const pid = Number(props.match(/_NET_WM_PID\(CARDINAL\) = (\d+)/)?.[1]) || 0
  const name = props.match(/WM_CLASS\(STRING\) = "[^"]*", "([^"]*)"/)?.[1] || ''
  const title = props.match(/_NET_WM_NAME\(UTF8_STRING\) = "(.*)"/)?.[1] || ''
  return { id, pid, name, title }
}
/** `xprop -root _NET_ACTIVE_WINDOW` then `xprop -id <id> …`. X11 and XWayland only. */
async function linux() {
  const root = await run('xprop', ['-root', '_NET_ACTIVE_WINDOW'])
  const id = activeId(root)
  if (!id) return null
  const win = parseXprop(root, await run('xprop', ['-id', id, '_NET_WM_PID', 'WM_CLASS', '_NET_WM_NAME']))
  if (!win) return null
  let exe = '', hint = ''
  try { exe = fs.readlinkSync(`/proc/${win.pid}/exe`) } catch { /* another user's process */ }
  try { hint = fs.readFileSync(`/proc/${win.pid}/cmdline`, 'utf8').replaceAll('\0', ' ') } catch { /* gone */ }
  return { name: win.name || path.basename(exe), path: exe, title: win.title, hint, pid: win.pid }
}

/** `lsappinfo` ships with macOS and needs no Accessibility or Screen Recording permission. */
function parseLsappinfo(info) {
  const name = info.match(/"LSDisplayName"="([^"]*)"/)?.[1] || ''
  const bundle = info.match(/"LSBundlePath"="([^"]*)"/)?.[1] || ''
  const pid = Number(info.match(/"pid"\s*=\s*(\d+)/)?.[1]) || 0
  return name ? { name, path: bundle, pid } : null
}
async function macos() {
  const front = (await run('lsappinfo', ['front'])).trim()
  if (!front) return null
  const app = parseLsappinfo(await run('lsappinfo', ['info', '-only', 'name', '-only', 'bundlepath', '-only', 'pid', front]))
  if (!app) return null
  // Java games (Minecraft) show as "java"; the command line says which.
  const hint = app.pid ? await run('ps', ['-o', 'command=', '-p', String(app.pid)]) : ''
  return { ...app, title: '', hint }
}

// PowerShell is on every Windows install. One long-lived process loads the
// P/Invoke once (Add-Type compiles C#, far too slow to repeat every sample) and
// answers one tab-separated line per request on stdin. FileVersionInfo gives the
// product's own name ("Google Chrome"); the command line tells java games apart.
const WINDOWS_SCRIPT = [
  '$s=\'[DllImport("user32.dll")] public static extern System.IntPtr GetForegroundWindow(); [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(System.IntPtr h, out int p);\'',
  'Add-Type -Name W -Namespace DenFg -MemberDefinition $s',
  'while ($null -ne [Console]::In.ReadLine()) {',
  '  $p=0; [void][DenFg.W]::GetWindowThreadProcessId([DenFg.W]::GetForegroundWindow(), [ref]$p)',
  '  $x=Get-Process -Id $p -ErrorAction SilentlyContinue',
  '  $c=(Get-CimInstance Win32_Process -Filter "ProcessId=$p" -ErrorAction SilentlyContinue).CommandLine -replace "[\t\r\n]", " "',
  '  if ($x) { [Console]::Out.WriteLine("$($x.ProcessName)`t$($x.Path)`t`t$($x.MainModule.FileVersionInfo.FileDescription)`t$p`t$c") } else { [Console]::Out.WriteLine("") }',
  '  [Console]::Out.Flush()',
  '}',
].join('\n')
function parseWindows(line) {
  const [process, exe = '', title = '', description = '', pid = '0', command = ''] = line.trim().split('\t')
  return process ? { name: description.trim() || process, process, path: exe, title, hint: `${exe} ${command}`.trim(), pid: Number(pid) || 0 } : null
}

/**
 * Asks a long-lived helper for one line at a time. A helper that exits or goes
 * quiet is replaced on the next request, so a hung PowerShell costs one sample.
 * `start` is injectable so the protocol can be tested without Windows.
 */
function lineReader(start, timeout = 4000) {
  let child = null, buffer = '', waiting = null
  const restart = () => {
    child?.kill(); child = start(); buffer = ''
    child.stdout.setEncoding('utf8')
    child.stdout.on('data', chunk => {
      buffer += chunk
      let at
      while ((at = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, at); buffer = buffer.slice(at + 1)
        if (waiting) { const w = waiting; waiting = null; clearTimeout(w.timer); w.resolve(line) }
      }
    })
    const gone = () => { child = null; if (waiting) { const w = waiting; waiting = null; clearTimeout(w.timer); w.resolve('') } }
    child.on('exit', gone); child.on('error', gone)
  }
  return {
    ask() {
      if (!child) restart()
      if (waiting) return Promise.resolve('')
      return new Promise(resolve => {
        waiting = { resolve, timer: setTimeout(() => { waiting = null; child?.kill(); child = null; resolve('') }, timeout) }
        child.stdin.write('\n')
      })
    },
    stop() { child?.kill(); child = null },
  }
}
let powershell = null
async function windows() {
  powershell ??= lineReader(() => spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-NoLogo', '-ExecutionPolicy', 'Bypass', '-Command', WINDOWS_SCRIPT], { windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'] }))
  return parseWindows(await powershell.ask())
}

// --- deciding what to say ----------------------------------------------------

// Desktop shells, lock screens and launchers are not activities.
const IGNORED = new Set(['explorer', 'finder', 'dock', 'loginwindow', 'systemuiserver', 'screensaverengine', 'lockapp',
  'gnome-shell', 'plasmashell', 'xfdesktop', 'xfce4-panel', 'desktop', 'searchhost', 'shellexperiencehost',
  'startmenuexperiencehost', 'applicationframehost', 'windows explorer', 'task switching', 'control center', 'notification center'])
const FRIENDLY = {
  code: 'Visual Studio Code', 'code - oss': 'Visual Studio Code', chrome: 'Google Chrome', 'google-chrome': 'Google Chrome',
  firefox: 'Firefox', 'firefox-esr': 'Firefox', obs: 'OBS Studio', 'obs64': 'OBS Studio', gimp: 'GIMP',
  'gnome-terminal-server': 'Terminal', 'org.gnome.nautilus': 'Files', 'jetbrains-idea': 'IntelliJ IDEA', 'jetbrains-pycharm': 'PyCharm',
}
const tidy = name => {
  const bare = name.replace(/\.(exe|app)$/i, '').trim()
  return FRIENDLY[bare.toLowerCase()] || (bare === bare.toLowerCase() ? bare.charAt(0).toUpperCase() + bare.slice(1) : bare)
}

/**
 * Minecraft, judged by the process alone. A window title is never enough: a wiki
 * tab or a video about Minecraft is not playing it. Java Edition is a java process
 * whose path or command line names the game; Bedrock is its own executable.
 */
function minecraft(win, where) {
  const exe = (win.process || path.basename(win.path || '') || win.name).replace(/\.exe$/i, '').toLowerCase()
  if (/^minecraft(\.windows)?$/.test(exe)) return true
  if (!/^javaw?$/.test(exe)) return false
  return /net\.minecraft|[\\/.]minecraft[\\/]|minecraft[\\/]runtime|--gamedir\s+\S*minecraft/i.test(where)
}

/** A focused window becomes "Playing X", "Using Y", nothing, or `self` (Den is focused). */
function classify(win, self = { pid: process.pid, exe: process.execPath }) {
  if (!win || !win.name) return null
  const where = `${win.path || ''} ${win.hint || ''}`
  if ((win.pid && win.pid === self.pid) || (win.path && win.path === self.exe) || /^den$/i.test(win.name)) return 'self'
  const steam = where.match(/steamapps[\\/]common[\\/]([^\\/]+)/i)
  if (steam) return { kind: 'playing', name: steam[1].trim().slice(0, 64) }
  if (minecraft(win, where)) return { kind: 'playing', name: 'Minecraft' }
  const name = tidy(win.name)
  if (!name || IGNORED.has(name.toLowerCase()) || IGNORED.has(win.name.toLowerCase())) return null
  return { kind: 'using', name: name.slice(0, 64) }
}

const IDLE_SECONDS = 600
const INTERVAL = 15_000

/**
 * Samples the focused window and emits `activity` when what people would see
 * changes. Den being focused keeps the previous answer, so alt-tabbing to chat
 * mid-game does not drop "Playing". Idle, locked or asleep means nothing.
 */
function activity(emit, powerMonitor) {
  const read = { linux, darwin: macos, win32: windows }[process.platform]
  let current = null, away = false, timer
  const publish = next => {
    if (JSON.stringify(next) === JSON.stringify(current)) return
    current = next
    emit('activity', current)
  }
  async function sample() {
    if (!read || away || powerMonitor.getSystemIdleTime() >= IDLE_SECONDS) return publish(null)
    const next = classify(await read().catch(() => null))
    if (next !== 'self') publish(next)
  }
  const sleep = () => { away = true; publish(null) }
  const wake = () => { away = false; void sample() }
  powerMonitor.on('suspend', sleep); powerMonitor.on('lock-screen', sleep)
  powerMonitor.on('resume', wake); powerMonitor.on('unlock-screen', wake)
  void sample()
  timer = setInterval(sample, INTERVAL)
  return { current: () => current, stop: () => clearInterval(timer) }
}

module.exports = { activity, classify, parseXprop, parseLsappinfo, parseWindows, lineReader }
