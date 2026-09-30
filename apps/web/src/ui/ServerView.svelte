<script lang="ts">
  import SidebarToggle from './SidebarToggle.svelte'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'
  import InlineConfirm from './InlineConfirm.svelte'
  import ServerChart from './ServerChart.svelte'
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import type { ServerAction, ServerDetail, ServerActionResult } from '../lib/types'
  import { dotTone, formatStat, gameGlyph, gameLabel, lagging, playtime, stateLabel, usableActions } from '../lib/servers'

  let { slug, onmenu, narrow }: { slug: string; onmenu: () => void; narrow: boolean } = $props()

  const server = $derived(store.servers.find((s) => s.slug === slug))
  const admin = $derived(store.me?.role === 'admin')
  const actions = $derived(server ? usableActions(server, admin) : [])
  let detail = $state<ServerDetail | null>(null)
  let error = $state('')

  // Live numbers arrive on the socket; history and playtime are read here,
  // and again each minute while the page is open, since that is their grain.
  async function load() {
    try { detail = await store.api.get<ServerDetail>(`/servers/${slug}`); error = '' }
    catch (e) { error = (e as Error).message }
  }
  $effect(() => {
    void slug
    detail = null; void load()
    const t = setInterval(() => { if (document.visibilityState === 'visible') void load() }, 60_000)
    return () => clearInterval(t)
  })

  let pending = $state(''), result = $state<{ ok: boolean; text: string } | null>(null)
  async function run(a: ServerAction) {
    pending = a.id; result = null
    try {
      const r = await store.api.post<ServerActionResult>(`/servers/${slug}/actions/${a.id}`)
      result = { ok: true, text: r.message || `${a.label}: done` }
    } catch (e) {
      result = { ok: false, text: (e as Error).message }
    } finally { pending = '' }
  }
  const confirmSentence = (a: ServerAction) => {
    const n = server?.players.length ?? 0
    const who = n ? ` ${n === 1 ? 'The 1 player' : `All ${n} players`} online will be disconnected.` : ''
    return a.id === 'stop' ? `Stop the server now?${who}` : `${a.label} the server now?${who}`
  }
  // Stopping or restarting throws people out of the game; saving and starting do not.
  const disruptive = (a: ServerAction) => a.id === 'stop' || a.id === 'restart'

  let copied = $state(false)
  async function copy() {
    if (!server?.address) return
    await navigator.clipboard.writeText(server.address)
    copied = true; setTimeout(() => (copied = false), 1600)
  }

  const who = (p: { name: string; user_id?: string | null }) => p.user_id && store.users.get(p.user_id) ? store.name(p.user_id) : null
  const players = $derived(detail?.history.find((h) => h.key === 'players'))
  const graphs = $derived(detail?.history.filter((h) => h.key !== 'players' && h.points.length) ?? [])
  const tone = $derived(server ? dotTone(server) : 'down')
  const lag = $derived(server?.stats.find(lagging))
</script>

<section class="page">
  <header class="head">
    {#if !narrow && !store.layout.sidebar}<SidebarToggle />{/if}
    {#if narrow}<button class="btn quiet iconbtn" onclick={onmenu} aria-label="Menu"><Icon name="menu" /></button>{/if}
    <span class="crumb eyebrow">Servers</span>
    <span class="faint" aria-hidden="true">›</span>
    <h1 class="display">{server ? gameLabel(server) : slug}</h1>
  </header>

  <div class="scroll">
    {#if !server}
      <div class="empty">
        <p class="muted">That server isn't here.</p>
        <a href="/" onclick={(e) => { e.preventDefault(); router.go('/') }}>Back to the den</a>
      </div>
    {:else}
      <div class="body">
        <article class="hero" data-tone={tone}>
          <div class="icon" aria-hidden="true">
            {#if server.icon_url}<img src={server.icon_url} alt="" width="64" height="64" />{:else}<span>{gameGlyph(server.game)}</span>{/if}
          </div>
          <div class="who">
            <h2 class="display name">{server.name}</h2>
            <p class="state" data-testid="server-state">
              <span class="dot {tone}" aria-hidden="true"></span>
              <b>{stateLabel(server)}</b>
              {#if server.connected && server.state === 'up'}
                <span class="muted">· {server.players.length}{server.max_players != null ? ` of ${server.max_players}` : ''} online</span>
              {:else if server.connected && server.state === 'asleep'}
                <span class="muted">· wakes when someone joins</span>
              {/if}
              {#if lag}<span class="lag">· Laggy</span>{/if}
            </p>
            {#if server.details.length}<p class="facts muted">{server.details.join(' · ')}</p>{/if}
            {#if server.address}
              <p class="address">
                <code class="mono">{server.address}</code>
                <button class="btn quiet small" onclick={copy} aria-live="polite"><Icon name={copied ? 'check' : 'copy'} size={14} /> {copied ? 'Copied' : 'Copy'}</button>
              </p>
            {/if}
          </div>
          {#if actions.length}
            <div class="actions">
              {#each actions as a (a.id)}
                {#if disruptive(a)}
                  <InlineConfirm action={pending === a.id ? `${a.label}…` : a.label} sentence={confirmSentence(a)} disabled={!!pending} confirm={() => run(a)} />
                {:else}
                  <button class="btn" class:lit={a.id === 'start'} disabled={!!pending} onclick={() => run(a)}>{pending === a.id ? `${a.label}…` : a.label}</button>
                {/if}
              {/each}
            </div>
          {/if}
        </article>

        {#if result}<p class="result" class:bad={!result.ok} role="status">{result.text}</p>{/if}
        {#if !server.connected}
          <p class="notice" role="status">Den can't reach the machine this server runs on, so its numbers and buttons are unavailable. It comes back on its own once that machine is online.</p>
        {/if}

        {#if server.stats.length}
          <dl class="stats">
            {#each server.stats as s (s.key)}
              <div class="stat" class:warn={lagging(s)}><dt class="eyebrow">{s.label}</dt><dd class="mono">{formatStat(s)}</dd></div>
            {/each}
          </dl>
        {/if}

        <div class="split">
          <section class="card online" aria-labelledby="online-h">
            <h3 id="online-h" class="eyebrow">Online now</h3>
            {#if server.players.length}
              <ul>
                {#each server.players as p (p.name)}
                  <li>
                    {#if p.user_id}<Avatar userId={p.user_id} size={28} presence={false} />{:else}<span class="initial" aria-hidden="true">{p.name[0]?.toUpperCase()}</span>{/if}
                    <span class="pname">{who(p) ?? p.name}</span>
                    {#if who(p)}<span class="faint mono">{p.name}</span>{/if}
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="faint none">{!server.connected ? "Den can't see who's playing right now." : server.state === 'up' ? 'Nobody is playing right now.' : `Nobody can play while the server is ${stateLabel(server).toLowerCase()}.`}</p>
            {/if}
          </section>
          <div class="charts">
            {#if players}<ServerChart series={players} ceiling={server.max_players ?? undefined} step />{/if}
            {#each graphs as g (g.key)}<ServerChart series={g} />{/each}
            {#if !detail && !error}<p class="faint">Loading history…</p>{/if}
          </div>
        </div>

        {#if detail?.playtime.length}
          <section class="card" aria-labelledby="play-h">
            <h3 id="play-h" class="eyebrow">Playtime</h3>
            <table>
              <thead><tr><th scope="col">Player</th><th scope="col" class="num">Last 7 days</th><th scope="col" class="num">All time</th></tr></thead>
              <tbody>
                {#each detail.playtime as p (p.name)}
                  <tr>
                    <th scope="row">
                      <span class="prow">
                        {#if p.user_id}<Avatar userId={p.user_id} size={22} presence={false} />{:else}<span class="initial small" aria-hidden="true">{p.name[0]?.toUpperCase()}</span>{/if}
                        {who(p) ?? p.name}
                      </span>
                    </th>
                    <td class="num mono">{playtime(p.week_seconds ?? 0)}</td>
                    <td class="num mono">{p.total_seconds != null ? playtime(p.total_seconds) : '—'}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </section>
        {/if}
        {#if error}<p class="result bad" role="alert">{error} <button class="btn quiet small" onclick={load}>Retry</button></p>{/if}
      </div>
    {/if}
  </div>
</section>

<style>
  .page { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .head { display: flex; align-items: center; gap: 8px; padding: 10px var(--gutter); min-height: 52px; border-bottom: 1px solid var(--line); }
  h1 { font-size: 19px; margin: 0; font-weight: 600; }
  .crumb { margin-top: 2px; }
  .iconbtn { padding: 6px; }
  .scroll { flex: 1; overflow-y: auto; padding: 20px var(--gutter) 32px; }
  .body { max-width: 920px; margin: 0 auto; display: grid; gap: 14px; }
  .empty { padding: 80px 20px; text-align: center; display: grid; justify-items: center; gap: 6px; }

  .hero {
    display: grid; grid-template-columns: auto minmax(0, 1fr) auto; gap: 16px; align-items: start;
    padding: 18px; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-2); box-shadow: var(--lift);
  }
  .hero[data-tone='up'] { border-color: color-mix(in srgb, var(--moss) 35%, var(--line)); }
  .icon { width: 64px; height: 64px; border-radius: var(--r); overflow: hidden; display: grid; place-items: center; background: var(--bg-3); font-size: 32px; }
  .icon img { width: 100%; height: 100%; image-rendering: pixelated; }
  .hero[data-tone='asleep'] .icon, .hero[data-tone='down'] .icon { filter: grayscale(.7); opacity: .75; }
  .who { min-width: 0; display: grid; gap: 4px; }
  .name { margin: 0; font-size: 22px; line-height: 1.2; overflow-wrap: anywhere; }
  .state, .facts, .address { margin: 0; display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .facts { font-size: 13.5px; }
  .lag { color: var(--lamp); font-weight: 700; }
  .address code { padding: 3px 8px; border-radius: var(--r); background: var(--bg-3); font-size: 13px; overflow-wrap: anywhere; }
  .small { padding: 3px 8px; font-size: 13px; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; justify-content: flex-end; align-items: flex-start; max-width: 380px; }

  .dot { width: 9px; height: 9px; border-radius: 50%; flex: none; }
  .dot.up { background: var(--moss); box-shadow: 0 0 8px color-mix(in srgb, var(--moss) 70%, transparent); }
  .dot.busy { background: var(--lamp); }
  .dot.asleep { background: transparent; box-shadow: inset 0 0 0 1.5px var(--ink-3); }
  .dot.down { background: var(--danger); }
  @media (prefers-reduced-motion: no-preference) { .dot.busy { animation: pulse 1.2s infinite alternate; } }
  @keyframes pulse { to { opacity: .45; } }

  .result { margin: 0; padding: 8px 12px; border-radius: var(--r); background: color-mix(in srgb, var(--moss) 14%, transparent); font-size: 14px; }
  .result.bad { background: color-mix(in srgb, var(--danger) 16%, transparent); }
  .notice { margin: 0; padding: 10px 14px; border-radius: var(--r); background: color-mix(in srgb, var(--danger) 12%, var(--bg-2)); font-size: 14px; line-height: 1.45; }

  .stats { margin: 0; display: grid; grid-template-columns: repeat(auto-fit, minmax(118px, 1fr)); gap: 10px; }
  .stat { padding: 12px 14px; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-2); }
  .stat dd { margin: 4px 0 0; font-size: 20px; font-variant-numeric: tabular-nums; color: var(--ink); }
  .stat.warn { border-color: var(--lamp-dim); }
  .stat.warn dd { color: var(--lamp); }

  .split { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1.6fr); gap: 14px; align-items: start; }
  .charts { display: grid; gap: 14px; min-width: 0; }
  .card { padding: 14px; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-2); }
  .card h3 { margin: 0 0 10px; }
  .online ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
  .online li { display: flex; align-items: center; gap: 10px; min-width: 0; }
  .pname { font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .online li .mono { font-size: 11.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .none { margin: 0; font-size: 14px; }
  .initial { width: 28px; height: 28px; flex: none; display: grid; place-items: center; border-radius: var(--avatar-r, 35%); background: var(--bg-3); color: var(--ink-2); font-weight: 700; font-size: 13px; }
  .initial.small { width: 22px; height: 22px; font-size: 11px; }

  table { width: 100%; border-collapse: collapse; font-size: 14px; }
  th, td { padding: 7px 4px; text-align: left; border-top: 1px solid var(--line); }
  thead th { border-top: 0; font-size: 12px; font-weight: 600; color: var(--ink-3); }
  tbody th { font-weight: 600; }
  .prow { display: inline-flex; align-items: center; gap: 8px; }
  .num { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }

  @media (max-width: 720px) {
    .hero { grid-template-columns: auto minmax(0, 1fr); }
    .actions { grid-column: 1 / -1; justify-content: flex-start; max-width: none; }
    .split { grid-template-columns: minmax(0, 1fr); }
    .icon { width: 52px; height: 52px; }
  }
</style>
