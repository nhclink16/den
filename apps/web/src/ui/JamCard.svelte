<script lang="ts">
  // The Jam card. Pinned under the room header so it cannot scroll away, and
  // deliberately not shaped like a link: art, a live track line, and one Join.
  //
  // What it must never claim: Spotify does not report who is listening to a Jam.
  // The faces are Den members who pressed Join. The copy says exactly that.
  import { store, type Store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { endJam, joinJam } from '../lib/jam'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import InlineConfirm from './InlineConfirm.svelte'
  import { anchor, type Anchor } from '../lib/jam-clock'

  let { channelId, owner = store }: { channelId: string; owner?: Store } = $props()

  const jam = $derived(owner.jams.get(channelId))
  const playing = $derived(jam?.now_playing ?? null)
  const isHost = $derived(!!jam && jam.host_id === owner.me?.id)
  const canEnd = $derived(isHost || owner.me?.role === 'admin')
  const joined = $derived(jam?.joined_user_ids ?? [])
  const mine = $derived(!!owner.me && joined.includes(owner.me.id))
  const FACES = 4
  const overflow = $derived(Math.max(0, joined.length - FACES))

  let error = $state('')
  let busy = $state(false)
  let artFailed = $state(false)
  $effect(() => { void playing?.album_art; artFailed = false })

  // --- live progress -------------------------------------------------------
  // `sampled_at` is the server's clock, so it is never compared to Date.now().
  // The server re-serves a cached sample for a few seconds; anchor() keeps the
  // running clock for a repeat and places a newer sample by the gap between them.
  let clock = $state(Date.now())
  let base = $state<Anchor | null>(null)
  $effect(() => {
    const p = playing
    base = p ? anchor(base, { track: `${p.track}\u0000${p.artists}`, sampled_at: p.sampled_at, progress_ms: p.progress_ms ?? 0 }, Date.now()) : null
  })
  const duration = $derived(playing?.duration_ms ?? 0)
  const elapsed = $derived(
    Math.max(0, Math.min(duration || Infinity, base ? base.ms + (playing?.is_playing ? clock - base.at : 0) : 0)),
  )
  const fraction = $derived(duration > 0 ? Math.min(1, elapsed / duration) : 0)
  const time = (ms: number) => `${Math.floor(ms / 60000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, '0')}`
  $effect(() => {
    if (!playing?.is_playing || !duration) return
    const id = setInterval(() => (clock = Date.now()), 500)
    return () => clearInterval(id)
  })

  // --- freshness -----------------------------------------------------------
  // Jam mutations arrive over the socket, but a track change is only a change on
  // Spotify's side. Poll while a Jam is up: often when a track is showing, rarely
  // when it is not, and never while the tab is hidden.
  $effect(() => { void owner.loadJam(channelId).catch(() => {}) })
  const live = $derived(!!playing)
  const present = $derived(!!jam)
  $effect(() => {
    if (!present) return
    const id = setInterval(() => {
      if (document.visibilityState === 'visible') void owner.loadJam(channelId).catch(() => {})
    }, live ? 5000 : 20000)
    return () => clearInterval(id)
  })

  async function act(fn: () => Promise<unknown>) {
    if (busy) return
    busy = true; error = ''
    try { await fn() } catch (e) { error = e instanceof Error ? e.message : 'Could not reach the Jam.' }
    finally { busy = false }
  }
  // The anchor navigates on its own; this only records the click for the count.
  function join() { if (!mine) void act(() => joinJam(owner, channelId)) }
  async function end() { await act(() => endJam(owner, channelId)) }

  const hint = $derived(
    !isHost || playing ? null
      : owner.spotify.connection === 'disconnected' ? { label: 'Show what you are playing', title: 'Connect your Spotify account' }
      : owner.spotify.connection === 'reauthorize' ? { label: 'Reconnect Spotify', title: 'Spotify stopped accepting the stored token' }
      : null,
  )
  const settings = (e: MouseEvent) => { e.preventDefault(); router.go('/settings/spotify') }
</script>

{#if jam}
  <section class="jam" class:with-art={!!playing?.album_art && !artFailed} aria-label="Spotify Jam">
    {#if playing?.album_art && !artFailed}
      <!-- Ambience only: a blurred, scaled copy of the art bled behind the strip. -->
      <div class="wash" style="background-image:url({playing.album_art})" aria-hidden="true"></div>
    {/if}

    <!-- The sleeve, with the record sliding out of it while a track is up. The
         record spins only while Spotify reports playback, so paused reads as paused. -->
    <div class="sleeve" class:out={!!playing} class:spinning={!!playing?.is_playing} aria-hidden="true">
      {#if playing}
        <span class="record" style={playing.album_art && !artFailed ? `--label:url(${JSON.stringify(playing.album_art)})` : ''}></span>
      {/if}
      <div class="art">
        {#if playing?.album_art && !artFailed}
          <img src={playing.album_art} alt="" referrerpolicy="no-referrer" onerror={() => (artFailed = true)} />
        {:else}
          <Icon name="music" size={22} />
        {/if}
      </div>
    </div>

    <div class="copy">
      <span class="eyebrow">
        <svg class="spotify" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="8" fill="currentColor" /><path d="M4 6.2c2.7-.8 5.6-.5 8 .9M4.5 8.6c2.2-.6 4.6-.4 6.6.8M5 10.9c1.8-.4 3.6-.2 5.1.6" fill="none" stroke="var(--jam-bg)" stroke-width="1.2" stroke-linecap="round" /></svg>
        <span class="brand" class:playing={!!playing}>Spotify Jam</span>
        {#if playing}<span class="sep" aria-hidden="true">·</span><span class="state" class:paused={!playing.is_playing}>{playing.is_playing ? 'Now playing' : 'Paused'}</span>{/if}
      </span>
      <h2 class="display" title={playing ? `${playing.track} — ${playing.artists}` : undefined}>
        {playing ? playing.track : 'Listening together'}
      </h2>
      <p class="line" class:has-track={!!playing}>
        {#if playing}<span class="artists">{playing.artists}</span><span class="dot" aria-hidden="true">·</span>{/if}
        <span class="starter">Started by {owner.name(jam.host_id)}</span>
        {#if duration > 0}<span class="clock mono" aria-hidden="true">{time(elapsed)} / {time(duration)}</span>{/if}
        {#if hint}<a class="hint" href="/settings/spotify" title={hint.title} onclick={settings}>{hint.label}</a>{/if}
      </p>
    </div>

    {#if joined.length}
      <div class="who" title="Den members who opened this Jam from here. Spotify does not say who is listening.">
        <span class="faces">
          {#each joined.slice(0, FACES) as id (id)}<Avatar userId={id} size={26} instance={owner} presence={false} />{/each}
          {#if overflow}<span class="more">+{overflow}</span>{/if}
        </span>
        <span class="who-label">Joined from Den</span>
        <span class="sr-only">{joined.length} {joined.length === 1 ? 'person' : 'people'} opened this Jam from Den. Spotify does not report who is listening.</span>
      </div>
    {/if}

    <div class="actions">
      <a class="btn open" class:lit={!mine} href={jam.url} target="_blank" rel="noopener noreferrer" onclick={join}>
        <Icon name="popout" size={14} />
        {mine ? 'Open' : 'Join'}
        <span class="sr-only"> Spotify Jam in a new window</span>
      </a>
      {#if canEnd}
        <InlineConfirm action="End" sentence="End this Jam? The card goes away for everyone. Spotify keeps playing until each person stops it." confirm={end} disabled={busy} />
      {/if}
    </div>

    {#if duration > 0}
      <div class="rail" aria-hidden="true"><span style="transform:scaleX({fraction})"></span></div>
      <span class="sr-only">{time(elapsed)} of {time(duration)}</span>
    {/if}
  </section>
  {#if error}<p class="jam-error" role="alert">{error}</p>{/if}
{/if}

<style>
  .jam {
    --jam-bg: var(--bg-2);
    position: relative; isolation: isolate; overflow: hidden;
    display: flex; align-items: center; gap: 16px;
    padding: 12px var(--gutter) 14px;
    background: var(--jam-bg);
    border-bottom: 1px solid var(--line);
  }
  /* The art, blurred past recognition, strongest behind the sleeve and gone by the
     controls. Pure atmosphere: it carries no information and is hidden from AT. */
  .wash {
    position: absolute; inset: -60% 30% -60% -10%; z-index: -2;
    background-size: cover; background-position: center;
    filter: blur(44px) saturate(1.7); opacity: 0.75;
  }
  /* The scrim thickens toward the text and controls so every word keeps its contrast. */
  .jam.with-art::before {
    content: ''; position: absolute; inset: 0; z-index: -1;
    background: linear-gradient(to right,
      color-mix(in srgb, var(--jam-bg) 35%, transparent),
      color-mix(in srgb, var(--jam-bg) 78%, transparent) 150px,
      color-mix(in srgb, var(--jam-bg) 90%, transparent) 55%,
      var(--jam-bg));
  }

  /* Sleeve and record. The record is a pressed disc: grooves, a sheen, and the
     cover as its centre label. */
  .sleeve { position: relative; flex: none; width: 58px; height: 58px; transition: margin-right .5s var(--ease-out); }
  .sleeve.out { margin-right: 24px; }
  .art {
    position: relative; z-index: 1; width: 100%; height: 100%; border-radius: var(--r);
    display: grid; place-items: center; overflow: hidden;
    background: var(--bg-3); color: var(--lamp);
    box-shadow: 0 1px 0 color-mix(in srgb, #fff 12%, transparent) inset, 0 6px 18px -4px var(--shadow-lg);
  }
  .art img { width: 100%; height: 100%; object-fit: cover; }
  .record {
    position: absolute; inset: 3px; border-radius: 50%;
    background: repeating-radial-gradient(circle, #151515 0 1.5px, #1f1f1f 1.5px 3px);
    box-shadow: 0 4px 14px -4px rgba(0, 0, 0, .6);
    -webkit-mask: radial-gradient(circle, transparent 0 3%, #000 3.5%);
    mask: radial-gradient(circle, transparent 0 3%, #000 3.5%);
    transition: transform .6s var(--ease-out);
  }
  /* A sheen across the grooves; the cover, cropped round, is the centre label. */
  .record::before {
    content: ''; position: absolute; inset: 0; border-radius: 50%;
    background: conic-gradient(from 30deg, transparent 0 10%, rgba(255, 255, 255, .09) 14%, transparent 22% 55%, rgba(255, 255, 255, .06) 60%, transparent 68%);
  }
  .record::after {
    content: ''; position: absolute; inset: 28%; border-radius: 50%;
    background: var(--label, var(--lamp)) center / cover no-repeat, var(--lamp);
    box-shadow: 0 0 0 2px #0a0a0a;
  }
  .sleeve.out .record { transform: translateX(26px); }
  @media (prefers-reduced-motion: no-preference) {
    .sleeve.spinning .record::after, .sleeve.spinning .record::before { animation: spin 3.2s linear infinite; }
  }
  @keyframes spin { to { transform: rotate(1turn); } }

  .copy { min-width: 0; flex: 1; display: grid; gap: 2px; }
  /* Grid items default to min-width:auto, which would stop the strip shrinking. */
  .copy > * { min-width: 0; }
  .eyebrow { display: flex; align-items: center; gap: 6px; font-size: 11px; color: var(--ink-2); white-space: nowrap; }
  .spotify { width: 13px; height: 13px; flex: none; color: var(--ink); }
  .sep { color: var(--ink-3); }
  .state { color: var(--lamp); }
  .state.paused { color: var(--ink-3); }

  h2 { margin: 0; font-size: 17px; line-height: 1.25; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .line {
    margin: 0; font-size: 13px; color: var(--ink-2);
    display: flex; align-items: baseline; gap: 6px; min-width: 0; overflow: hidden;
  }
  .artists { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; color: var(--ink); }
  .dot { color: var(--ink-3); }
  .starter { white-space: nowrap; color: var(--ink-2); }
  .clock { margin-left: auto; padding-left: 10px; white-space: nowrap; font-size: 11px; color: var(--ink-2); font-variant-numeric: tabular-nums; }
  .hint { white-space: nowrap; border-bottom: 1px dotted currentColor; }
  .hint:hover { text-decoration: none; }

  .who { flex: none; display: grid; justify-items: center; gap: 4px; }
  .faces { display: flex; }
  .faces > :global(*) { margin-left: -7px; box-shadow: 0 0 0 2px var(--jam-bg); border-radius: var(--avatar-r, 35%); }
  .faces > :global(*:first-child) { margin-left: 0; }
  .more {
    display: grid; place-items: center; width: 26px; height: 26px; margin-left: -7px; border-radius: var(--avatar-r, 35%);
    background: var(--bg-3); color: var(--ink-2); font: 11px/1 var(--mono); box-shadow: 0 0 0 2px var(--jam-bg);
  }
  .who-label { font: 11px var(--mono); letter-spacing: 0.04em; color: var(--ink-2); }

  .actions { flex: none; display: flex; align-items: center; gap: 6px; margin-left: 4px; }
  .open { text-decoration: none; }
  .open:hover { text-decoration: none; }

  /* Progress sits on the card's own bottom edge, replacing the hairline there,
     and the playhead carries the glow. */
  .rail { position: absolute; inset-inline: 0; bottom: 0; height: 3px; background: color-mix(in srgb, var(--ink) 10%, transparent); }
  .rail span {
    display: block; height: 100%; width: 100%; transform-origin: left;
    background: linear-gradient(90deg, var(--lamp-dim), var(--lamp)); box-shadow: var(--glow);
    transition: transform 0.5s linear;
  }
  .jam-error { margin: 0; padding: 6px var(--gutter); font-size: 12px; color: var(--danger); background: var(--bg-2); border-bottom: 1px solid var(--line); }

  @media (max-width: 760px) {
    .jam { gap: 12px; padding-inline: 10px; }
    .sleeve { width: 48px; height: 48px; }
    .sleeve.out { margin-right: 12px; }
    .sleeve.out .record { transform: translateX(14px); }
    h2 { font-size: 15px; }
    /* With a track showing, the artists earn the room; without one the host line
       is all there is, so it stays. */
    .line.has-track .starter, .line.has-track .dot, .clock { display: none; }
    /* The icon still says Spotify; the words give way to the playing state. */
    .brand.playing, .brand.playing + .sep { display: none; }
    /* Same prompt lives in Settings; the strip does not have room for it here. */
    .hint { display: none; }
  }
  @media (max-width: 640px) {
    /* The row keeps its title, and the screen-reader sentence never moves. */
    .who-label { display: none; }
  }
  @media (max-width: 520px) {
    .who { display: none; }
  }
</style>
