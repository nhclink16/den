<script lang="ts">
  // The Jam card. Pinned under the room header so it cannot scroll away, and
  // deliberately not shaped like a link: art, a live track line, and one Join.
  //
  // What it must never claim: Spotify does not report who is listening to a Jam.
  // The faces are Den members who pressed Join. The copy says exactly that.
  import { store, type Store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import type { Jam } from '../lib/types'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import InlineConfirm from './InlineConfirm.svelte'

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

  // --- live progress -------------------------------------------------------
  // `sampled_at` is the server's clock. Comparing it to Date.now() would bake in
  // any skew, so anchor on when this browser received the sample instead, the way
  // MusicPanel anchors on musicReceivedAt.
  let clock = $state(Date.now())
  let base = $state({ at: Date.now(), ms: 0 })
  $effect(() => { base = { at: Date.now(), ms: playing?.progress_ms ?? 0 } })
  const duration = $derived(playing?.duration_ms ?? 0)
  const elapsed = $derived(
    Math.max(0, Math.min(duration || Infinity, base.ms + (playing?.is_playing ? clock - base.at : 0))),
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
  function join() { if (!mine) void act(async () => owner.receiveJam(channelId, await owner.api.post<Jam>(`/rooms/${channelId}/jam/join`, {}))) }
  async function end() { await act(async () => { await owner.api.del(`/rooms/${channelId}/jam`); owner.receiveJam(channelId, null) }) }

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

    <div class="art" aria-hidden="true">
      {#if playing?.album_art && !artFailed}
        <img src={playing.album_art} alt="" referrerpolicy="no-referrer" onerror={() => (artFailed = true)} />
      {:else}
        <Icon name="music" size={20} />
      {/if}
    </div>

    <div class="copy">
      <span class="eyebrow">
        {#if playing}<span class="pulse" class:paused={!playing.is_playing} aria-hidden="true"></span>{/if}
        {playing ? (playing.is_playing ? 'Jam · now playing' : 'Jam · paused') : 'Spotify Jam'}
      </span>
      <h2 class="display" title={playing ? `${playing.track} — ${playing.artists}` : undefined}>
        {playing ? playing.track : 'Listening together'}
      </h2>
      <p class="line" class:has-track={!!playing}>
        {#if playing}<span class="artists">{playing.artists}</span><span class="dot" aria-hidden="true">·</span>{/if}
        <span class="starter">Started by {owner.name(jam.host_id)}</span>
        {#if hint}<a class="hint" href="/settings/spotify" title={hint.title} onclick={settings}>{hint.label}</a>{/if}
      </p>
    </div>

    {#if joined.length}
      <div class="who" title="Den members who opened this Jam from here. Spotify does not say who is listening.">
        <span class="faces">
          {#each joined.slice(0, FACES) as id (id)}<Avatar userId={id} size={24} instance={owner} />{/each}
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
    position: relative; isolation: isolate; overflow: hidden;
    display: flex; align-items: center; gap: 14px;
    padding: 10px var(--gutter) 12px;
    background: var(--bg-2);
    border-bottom: 1px solid var(--line);
  }
  /* The art, blurred past recognition, fading out before the text starts. Pure
     atmosphere: it carries no information and is hidden from assistive tech. */
  .wash {
    position: absolute; inset: -40%; z-index: -2;
    background-size: cover; background-position: center;
    filter: blur(50px) saturate(1.6); opacity: 0.45;
  }
  /* The scrim sits between the wash and the content, thickening toward the
     controls so every word on the strip keeps its contrast. */
  .jam.with-art::before {
    content: ''; position: absolute; inset: 0; z-index: -1;
    background: linear-gradient(to right,
      color-mix(in srgb, var(--bg-2) 66%, transparent),
      color-mix(in srgb, var(--bg-2) 86%, transparent) 55%,
      var(--bg-2));
  }

  .art {
    flex: none; width: 46px; height: 46px; border-radius: var(--r);
    display: grid; place-items: center; overflow: hidden;
    background: var(--bg-3); color: var(--lamp);
    box-shadow: 0 2px 10px var(--shadow);
  }
  .art img { width: 100%; height: 100%; object-fit: cover; }

  .copy { min-width: 0; flex: 1; display: grid; gap: 1px; }
  /* Grid items default to min-width:auto, which would stop the strip shrinking. */
  .copy > * { min-width: 0; }
  .eyebrow { display: flex; align-items: center; gap: 6px; font-size: 10px; }
  .pulse {
    width: 6px; height: 6px; border-radius: 50%; background: var(--lamp);
    box-shadow: 0 0 0 0 var(--lamp-glow); animation: breathe 2.4s ease-in-out infinite;
  }
  .pulse.paused { background: var(--ink-3); animation: none; box-shadow: none; }
  @keyframes breathe { 50% { box-shadow: 0 0 0 4px transparent; opacity: 0.55; } }

  h2 { margin: 0; font-size: 15px; line-height: 1.25; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .line {
    margin: 0; font-size: 12px; color: var(--ink-2);
    display: flex; align-items: baseline; gap: 6px; min-width: 0; overflow: hidden;
  }
  .artists { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .dot { color: var(--ink-3); }
  .starter { white-space: nowrap; color: var(--ink-3); }
  .hint { white-space: nowrap; border-bottom: 1px dotted currentColor; }
  .hint:hover { text-decoration: none; }

  .who { flex: none; display: grid; justify-items: center; gap: 2px; }
  .faces { display: flex; }
  .faces > :global(*) { margin-left: -7px; box-shadow: 0 0 0 2px var(--bg-2); border-radius: 35%; }
  .faces > :global(*:first-child) { margin-left: 0; }
  .more {
    display: grid; place-items: center; width: 24px; height: 24px; border-radius: 35%;
    background: var(--bg-3); color: var(--ink-2); font: 11px/1 var(--mono);
  }
  .who-label { font: 10px var(--mono); letter-spacing: 0.06em; text-transform: uppercase; color: var(--ink-3); }

  .actions { flex: none; display: flex; align-items: center; gap: 6px; margin-left: 4px; }
  .open { text-decoration: none; }
  .open:hover { text-decoration: none; }

  /* Progress sits on the card's own bottom edge, replacing the hairline there. */
  .rail { position: absolute; left: 0; right: 0; bottom: 0; height: 2px; background: var(--line); }
  .rail span {
    display: block; height: 100%; width: 100%; transform-origin: left;
    background: var(--lamp); transition: transform 0.5s linear;
  }
  .jam-error { margin: 0; padding: 6px var(--gutter); font-size: 12px; color: var(--danger); background: var(--bg-2); border-bottom: 1px solid var(--line); }

  @media (max-width: 760px) {
    .jam { gap: 10px; padding-inline: 10px; }
    /* With a track showing, the artists earn the room; without one the host line
       is all there is, so it stays. */
    .line.has-track .starter, .line.has-track .dot { display: none; }
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
