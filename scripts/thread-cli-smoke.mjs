// Drives the BUILT den CLI against an isolated server on a spare loopback port.
// Nothing here touches the shared :7000 or :5173. Every DEN_* variable in the
// caller's environment is dropped, and both den-host commands are given an
// explicit DEN_HOST_CONFIG_DIR, so a real machine's credentials cannot be
// overwritten by this script. No code, token or credential is ever printed.
import { mkdtemp, rm, readFile } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import { spawn, execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'
import net from 'node:net'

const target = process.env.CARGO_TARGET_DIR || '/mnt/storage/den-electron-target'
const bin = n => `${target}/debug/${n}`
// DEN_HOST_CONFIG_DIR overrides any HOME substitution, so the only safe isolation
// is to remove the caller's DEN_* entirely and set what we need explicitly.
const ambient = Object.fromEntries(
  Object.entries(process.env).filter(([k]) => !k.startsWith('DEN_')),
)
const dir = await mkdtemp('/tmp/den-cli-smoke-')
const port = await new Promise(r => {
  const s = net.createServer()
  s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => r(p)) })
})
const url = `http://127.0.0.1:${port}`
// A fixed allowance for a graceful stop before the harness forces it. It is an
// allowance, not a measured product deadline, and not a claim about any past run.
// Each stop records how long it actually took.
const GRACE = 10000
const evidence = { server: 'http://127.0.0.1:<ephemeral>', steps: [], limits: [], grace_ms: GRACE }
let server, host, keepDir = false
const tokens = {}

const wait = ms => new Promise(r => setTimeout(r, ms))
const until = async (f, label, ms = 20000) => {
  const end = Date.now() + ms
  while (Date.now() < end) { if (await f()) return; await wait(80) }
  throw Error(`Timed out: ${label}`)
}
const step = (name, detail) => evidence.steps.push({ name, ...detail })

/// Run the CLI as an agent would. `as` picks which account's saved session is used.
function den(args, { as = 'runner', expect = 0, env = {}, input, label } = {}) {
  let out = '', code = 0
  try {
    out = execFileSync(bin('den'), ['--url', url, ...args], {
      encoding: 'utf8',
      input,
      timeout: 30000,
      env: { ...ambient, DEN_CONFIG: `${dir}/${as}.json`, ...env },
    })
  } catch (e) {
    code = e.status ?? 1
    out = `${e.stdout || ''}${e.stderr || ''}`
  }
  assert.equal(code, expect, `${label || `den ${args[0]} ${args[1] || ''}`} exited ${code}: ${out.slice(0, 300)}`)
  const trimmed = out.trim()
  return trimmed.startsWith('{') || trimmed.startsWith('[') ? JSON.parse(trimmed) : trimmed
}
/// Read-only observation of server state. Mutations under test go through the CLI.
const observe = async (path, as) =>
  (await fetch(url + path, { headers: { authorization: `Bearer ${tokens[as]}` } })).json()
const ids = v => v.map(x => x.id)
// `thread list` returns ThreadView, so its ID lives under .thread.
const listIds = v => v.map(x => x.thread.id)
const threadOf = v => (v.thread_id ?? null) || (v.thread ? v.thread.id : null)

try {
  server = spawn(bin('den-server'), [], {
    env: {
      ...ambient,
      DEN_BIND: `127.0.0.1:${port}`,
      DEN_DB: `${dir}/den.db`,
      DEN_UPLOADS: `${dir}/uploads`,
      DEN_BOOTSTRAP_FILE: `${dir}/bootstrap.key`,
      DEN_WEB_DIR: `${dir}/spa`,
    },
    stdio: ['ignore', 'ignore', 'ignore'],
  })
  await until(async () => { try { return (await fetch(`${url}/health`)).ok } catch { return false } }, 'server start')

  // The runner, and a separate person who will actually ask for things and reply.
  const runner = den(['init', 'runner', '--bootstrap-file', `${dir}/bootstrap.key`, '--password-stdin'],
    { input: 'smoke-password-123\n', label: 'den init' })
  assert.equal(runner.username, 'runner')
  const invite = den(['invite', 'create'])
  den(['register', 'sam', '--invite', invite.code, '--password-stdin'],
    { as: 'sam', input: 'smoke-password-456\n', label: 'den register' })
  // The CLI stores sessions keyed by server URL; read-only observation reuses the
  // same saved session rather than minting a second credential.
  const saved = async who => JSON.parse(await readFile(`${dir}/${who}.json`, 'utf8'))[url].token
  tokens.runner = await saved('runner')
  tokens.sam = await saved('sam')
  const room = den(['channel', 'create', 'ops']).id
  step('two real accounts', { room, accounts: ['runner', 'sam'] })

  // Sam asks for something. This is a different person from the runner.
  const request = den(['send', room, 'why is deploy failing'], { as: 'sam' }).id

  // ---- job alpha: one stable ID in the environment, everything inherits it ----
  const alpha = { DEN_TASK_ID: 'deploy-alpha' }
  const first = den(['send', room, 'on it', '--reply-to', request], { env: alpha })
  const alphaThread = first.thread_id
  assert(alphaThread, 'the first reply did not open a conversation')
  assert.equal(first.reply_to, request, 'the reply lost the request it answered')
  const progress = den(['send', room, 'reproduced on staging'], { env: alpha })
  assert.equal(progress.thread_id, alphaThread)
  // A canvas with an explicit reply target: the arrow must persist too.
  const board = den(['canvas', 'create', room, 'Deploy board', '--reply-to', progress.id], { env: alpha })
  const boardMessage = (await observe(`/messages/${board.message_id}`, 'runner'))
  assert.equal(boardMessage.thread_id, alphaThread, 'the card left the job')
  assert.equal(boardMessage.reply_to, progress.id, 'the card lost its reply target')
  step('alpha inherits env task', { thread: alphaThread, card: board.id, card_reply_to: boardMessage.reply_to })

  // ---- job beta: explicit --task overrides the environment and is its own root ----
  const betaRoot = den(['send', room, 'separate investigation', '--task', 'deploy-beta'], { env: alpha })
  const betaThread = betaRoot.thread ? betaRoot.thread.id : null
  assert(betaThread, 'the overriding task did not open its own conversation')
  assert.notEqual(betaThread, alphaThread, 'the second job reused the first conversation')
  assert.equal(betaRoot.thread_id ?? null, null, 'a task root should stay in the room')
  // Continuing beta by its ID rejoins that exact conversation.
  const betaMore = den(['send', room, 'beta progress', '--task', 'deploy-beta'], { env: alpha })
  assert.equal(betaMore.thread_id, betaThread, 'the explicit task did not rejoin its own conversation')
  step('beta is a distinct job', { thread: betaThread, rejoined: betaMore.thread_id })

  // Opting out: neither a reply in a job nor a new task root.
  const plain = den(['send', room, 'unrelated: standup moved'], { env: { DEN_TASK_ID: '' } })
  assert.equal(plain.thread_id ?? null, null, 'an unscoped post joined a job')
  assert.equal(plain.thread ?? null, null, 'an unscoped post opened a new job')
  step('opting out with an empty variable', { message: plain.id })

  // ---- reads never mutate ----
  const flat = den(['read', room, '--limit', '100'])
  const roots = den(['read', room, '--limit', '100', '--roots-only'])
  assert(flat.length > roots.length, 'roots-only returned the flat list')
  assert(roots.every(m => !m.thread_id), 'roots-only returned a reply')
  const beforeRead = den(['thread', 'get', alphaThread]).read_state
  den(['thread', 'read', alphaThread, '--limit', '5'])
  den(['thread', 'list', room])
  assert.deepEqual(den(['thread', 'get', alphaThread]).read_state, beforeRead, 'a read moved a position')
  step('reads are read-only', { flat: flat.length, roots: roots.length })

  // ---- reply pagination over known, ordered IDs ----
  const alphaReplies = ids(den(['thread', 'read', alphaThread, '--limit', '100']))
  assert.deepEqual(alphaReplies, [first.id, progress.id, board.message_id], 'unexpected reply order')
  assert.deepEqual(ids(den(['thread', 'read', alphaThread, '--before', board.message_id])),
    [first.id, progress.id], '--before returned the wrong page')
  assert.deepEqual(ids(den(['thread', 'read', alphaThread, '--after', first.id])),
    [progress.id, board.message_id], '--after returned the wrong page')
  assert.deepEqual(ids(den(['thread', 'read', alphaThread, '--limit', '1'])),
    [board.message_id], '--limit returned the wrong page')
  step('reply pagination', { replies: alphaReplies.length })

  // ---- resolution filters over a genuinely mixed set ----
  den(['thread', 'resolve', betaThread])
  const open = listIds(den(['thread', 'list', room, '--resolved', 'false']))
  const closed = listIds(den(['thread', 'list', room, '--resolved', 'true']))
  const every = listIds(den(['thread', 'list', room]))
  assert.deepEqual(closed, [betaThread], 'resolved filter returned the wrong set')
  assert(open.includes(alphaThread) && !open.includes(betaThread), 'open filter returned the wrong set')
  assert.equal(every.length, open.length + closed.length, 'unfiltered list is not the union')
  // Paging that list over its known order.
  assert.deepEqual(listIds(den(['thread', 'list', room, '--limit', '1'])), [every[0]], 'list limit')
  assert.deepEqual(listIds(den(['thread', 'list', room, '--before', every[0]])), every.slice(1), 'list before')
  step('resolution filters and list pagination', { open, closed, every })

  // A late post carrying the resolved job's ID is a conflict, by its actual message.
  const late = den(['send', room, 'too late', '--task', 'deploy-beta'], { expect: 1 })
  assert.match(late, /409 Conflict/, `expected a 409 for a resolved job, saw: ${late.slice(0, 160)}`)
  assert.match(late, /resolved/i, 'the conflict did not explain itself')
  den(['thread', 'reopen', betaThread])
  const resumed = den(['send', room, 'back on beta', '--task', 'deploy-beta'])
  assert.equal(resumed.thread_id, betaThread, 'the same task ID did not resume its job')
  step('resolve conflict and reopen', { conflict: '409 Conflict', resumed: resumed.thread_id })

  // ---- follow/unfollow keep the position; then real unread from another person ----
  const followed = den(['thread', 'follow', alphaThread])
  assert.equal(followed.following, true, 'follow did not take')
  const unfollowed = den(['thread', 'unfollow', alphaThread])
  assert.equal(unfollowed.following, false, 'unfollow did not take')
  assert.equal(unfollowed.last_read_id, followed.last_read_id, 'unfollow forgot the position')
  den(['thread', 'follow', alphaThread])
  den(['thread', 'follow', betaThread])
  const caughtUp = {
    alpha: den(['thread', 'get', alphaThread]).read_state,
    beta: den(['thread', 'get', betaThread]).read_state,
    room: (await observe('/users/me/read-state', 'runner')).find(s => s.channel_id === room),
  }
  // Sam adds new activity everywhere the runner is caught up.
  const samAlpha = den(['send', room, 'any update?', '--thread', alphaThread], { as: 'sam' })
  den(['send', room, 'and on beta?', '--thread', betaThread], { as: 'sam' })
  den(['send', room, 'room notice', '--task', ''], { as: 'sam' })
  const unread = {
    alpha: den(['thread', 'get', alphaThread]).read_state,
    beta: den(['thread', 'get', betaThread]).read_state,
    room: (await observe('/users/me/read-state', 'runner')).find(s => s.channel_id === room),
  }
  assert(unread.alpha.unread_count > 0 && unread.beta.unread_count > 0, 'no unread was created')

  // With something genuinely unread, reading still changes nothing at all.
  den(['thread', 'read', alphaThread, '--limit', '5'])
  den(['thread', 'list', room])
  den(['read', room, '--limit', '5'])
  den(['thread', 'get', betaThread])
  assert.deepEqual(
    {
      alpha: den(['thread', 'get', alphaThread]).read_state,
      beta: den(['thread', 'get', betaThread]).read_state,
      room: (await observe('/users/me/read-state', 'runner')).find(s => s.channel_id === room),
    },
    unread,
    'a read moved a position while unread existed',
  )

  // Marking alpha through Sam's actual new reply clears alpha alone.
  const marked = den(['thread', 'mark-read', alphaThread, samAlpha.id])
  assert.equal(marked.last_read_id, samAlpha.id, 'mark-read did not advance to the message given')
  const after = {
    alpha: den(['thread', 'get', alphaThread]).read_state,
    beta: den(['thread', 'get', betaThread]).read_state,
    room: (await observe('/users/me/read-state', 'runner')).find(s => s.channel_id === room),
  }
  assert.equal(after.alpha.unread_count, 0, 'alpha did not clear')
  assert.equal(after.beta.unread_count, unread.beta.unread_count, 'reading alpha changed beta')
  assert.equal(after.beta.last_read_id, unread.beta.last_read_id, 'reading alpha moved beta')
  assert.equal(after.room.last_read_id ?? null, unread.room.last_read_id ?? null, 'reading a thread moved the room')
  // The aggregate lost exactly what alpha held and nothing else.
  assert.equal(
    after.room.unread_count,
    unread.room.unread_count - unread.alpha.unread_count,
    'the channel total did not drop by exactly alpha',
  )
  const keyed = s => ({ last_read_id: s.last_read_id ?? null, unread: s.unread_count })
  step('mark-read is scoped to one conversation', {
    caught_up: { alpha: keyed(caughtUp.alpha), beta: keyed(caughtUp.beta), room: keyed(caughtUp.room) },
    injected: { alpha: keyed(unread.alpha), beta: keyed(unread.beta), room: keyed(unread.room) },
    after: { alpha: keyed(after.alpha), beta: keyed(after.beta), room: keyed(after.room) },
  })

  // ---- unread-only, against a set with one unread and one caught-up conversation ----
  const unreadOnly = listIds(den(['thread', 'list', room, '--unread-only']))
  assert(unreadOnly.includes(betaThread), 'unread-only dropped an unread conversation')
  assert(!unreadOnly.includes(alphaThread), 'unread-only returned a conversation with nothing unread')
  step('unread-only filter', { unread_only: unreadOnly })

  // ---- a real machine, a real terminal, through the CLI ----
  // A predictable shell and a harmless name, alongside the isolated config dir.
  const hostEnv = {
    DEN_HOST_CONFIG_DIR: `${dir}/host`,
    DEN_HOST_NAME: 'smoke-fixture',
    SHELL: '/bin/sh',
  }
  let code
  try {
    code = den(['host', 'enroll'])
    assert(typeof code === 'string' && code.includes('#'), 'enrolment did not return a usable code')
    execFileSync(bin('den-host'), ['login', code], {
      env: { ...ambient, ...hostEnv },
      stdio: 'ignore',
      timeout: 30000,
    })
  } catch {
    throw Error('den-host login failed (code and command withheld)')
  }
  host = spawn(bin('den-host'), ['run'], {
    env: { ...ambient, ...hostEnv },
    stdio: ['ignore', 'ignore', 'ignore'],
  })
  await until(async () => den(['host', 'list']).some(h => h.online), 'host connection')
  const machine = den(['host', 'list']).find(h => h.online)
  step('real host connected', { host: machine.id })

  const card = den(['terminal', 'open', machine.id, '--in', room], { env: alpha })
  const session = card.state.terminal.id
  const cardMessage = await observe(`/messages/${card.message_id}`, 'runner')
  assert.equal(cardMessage.thread_id, alphaThread, 'the terminal card left the job')
  step('terminal open joins the job', { session, thread: cardMessage.thread_id })

  // Drive the real PTY through the CLI and have the shell report its own PID. This
  // proves actual terminal input, not just that a card was created.
  const pidFile = `${dir}/shell.pid`
  const written = `stty -echo; echo $$ > '${pidFile}'\n`
  den(['terminal', 'write', session, written])
  const shellPid = await (async () => {
    const end = Date.now() + 15000
    while (Date.now() < end) {
      if (existsSync(pidFile)) {
        const v = (await readFile(pidFile, 'utf8')).trim()
        if (/^\d+$/.test(v)) return Number(v)
      }
      await wait(60)
    }
    throw Error('the shell never reported a PID through the CLI')
  })()
  assert.doesNotThrow(() => process.kill(shellPid, 0), 'the reported shell PID is not running')
  step('real PTY input through the CLI', { session, shell_pid: shellPid, command: 'echo $$ > <dir>/shell.pid' })

  // Sharing with explicit context: the group must actually be wired on this path.
  const shared = den(['terminal', 'share', session, '--in', room,
    '--thread', alphaThread, '--reply-to', samAlpha.id])
  assert.equal(shared.state.terminal.id, session, 'sharing started a different session')
  assert.notEqual(shared.id, card.id, 'sharing returned the original card')
  const sharedMessage = await observe(`/messages/${shared.message_id}`, 'runner')
  assert.equal(sharedMessage.thread_id, alphaThread, 'the shared card ignored --thread')
  assert.equal(sharedMessage.reply_to, samAlpha.id, 'the shared card ignored --reply-to')
  step('terminal share honours explicit context', {
    session: shared.state.terminal.id, card: shared.id, reply_to: sharedMessage.reply_to,
  })

  // `terminal close` returns once the server has QUEUED the Close, not once the
  // host has applied it. Waiting for the shell itself to go is the only ordering
  // that means anything, and it has to happen before the host is signalled.
  den(['terminal', 'close', session])
  const gone = await Promise.race([
    (async () => {
      while (true) {
        try { process.kill(shellPid, 0) } catch { return true }
        await wait(50)
      }
    })(),
    wait(10000).then(() => false),
  ])
  if (!gone) {
    keepDir = true
    throw Error(`the shell (pid ${shellPid}) was still running 10s after terminal close`)
  }
  step('terminal closed and its shell ended', { session, shell_pid: shellPid, terminated: true })

  const renamed = den(['thread', 'rename', alphaThread, 'Deploy investigation'])
  assert.equal(renamed.title, 'Deploy investigation', 'rename did not return the new title')
  assert.equal(
    den(['thread', 'get', alphaThread]).thread.title,
    'Deploy investigation',
    'the renamed title did not persist',
  )
  step('rename persists', { thread: alphaThread, title: renamed.title })
  evidence.commands = evidence.steps.length
} catch (e) {
  evidence.result = 'fail'
  evidence.error = String(e && e.message ? e.message : e).slice(0, 300)
  throw e
} finally {
  // Stop both through their real signal handlers, observe their actual exits, and
  // only then remove the data. A shutdown that has to be forced, exits badly, or
  // leaves the port bound is a failure: a smoke that cannot clean up is not green.
  const stop = async (name, p) => {
    if (!p) return { process: name, started: false }
    const pid = p.pid
    const began = Date.now()
    // Register before signalling, and treat an already-exited child as exited.
    const exited =
      p.exitCode !== null || p.signalCode !== null
        ? Promise.resolve({ code: p.exitCode, signal: p.signalCode })
        : new Promise(r => p.once('exit', (code, signal) => r({ code, signal })))
    // den-host observes ctrl_c only at its select points, so how long a graceful
    // stop takes depends on what it is part-way through. Ten seconds is a harness
    // allowance, not a claim about the product.
    p.kill('SIGINT')
    let seen = await Promise.race([exited, wait(GRACE).then(() => null)])
    let forced = false
    if (!seen) {
      forced = true
      p.kill('SIGKILL')
      seen = await Promise.race([exited, wait(GRACE).then(() => null)])
    }
    return {
      process: name,
      pid,
      forced,
      elapsed_ms: Date.now() - began,
      grace_ms: GRACE,
      ...(seen || { exit_observed: false }),
    }
  }
  const stops = []
  for (const [name, p] of [['den-host', host], ['den-server', server]]) {
    stops.push(await stop(name, p))
  }
  for (const s of stops) {
    if (s.started === false) continue
    evidence.steps.push({ name: `${s.process} stopped`, ...s })
    if (s.exit_observed === false) evidence.limits.push(`${s.process} never exited, even after SIGKILL`)
    else if (s.forced) evidence.limits.push(`${s.process} ignored SIGINT and had to be killed`)
    else if (s.code !== 0 && s.signal !== 'SIGINT') {
      evidence.limits.push(`${s.process} exited code ${s.code} signal ${s.signal}`)
    }
  }
  const released = await new Promise(r => {
    const s = net.createServer()
    s.once('error', () => r(false))
    s.listen(port, '127.0.0.1', () => s.close(() => r(true)))
  })
  if (!released) evidence.limits.push('the port was still bound after shutdown')
  evidence.port_released = released
  // Never delete files under a process that is still running. If any child could
  // not be confirmed dead, the directory is left where it is and reported.
  const running = keepDir || stops.some(s => s.started !== false && s.exit_observed === false)
  if (running) {
    evidence.data_dir_kept = dir
    evidence.limits.push(`data directory left in place at ${dir}: something was still running`)
  } else {
    await rm(dir, { recursive: true, force: true })
  }
  if (!evidence.result) evidence.result = evidence.limits.length ? 'fail' : 'pass'
  if (evidence.result !== 'pass') process.exitCode = 1
  console.log(JSON.stringify(evidence, null, 2))
}
