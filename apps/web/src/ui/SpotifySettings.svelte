<script lang="ts">
  // Connecting an account is optional and only the Jam host benefits from it.
  // Everything here has to survive three states the spec calls out: no client
  // secret on this server, a refresh token Spotify has stopped honouring, and a
  // native shell that cannot complete a browser redirect.
  import { store, instances } from '../lib/store.svelte'
  import { native } from '../lib/native'
  import type { SpotifyAuthorization } from '../lib/types'
  import InlineConfirm from './InlineConfirm.svelte'
  import Icon from './Icon.svelte'

  const owner = instances.active
  let busy = $state(false), error = $state('')
  const account = $derived(store.spotify)
  const link = $derived(account.connection)
  // Advisory only. Spotify rejecting the token is what actually decides.
  const daysLeft = $derived(account.expires_at ? Math.ceil((account.expires_at * 1000 - Date.now()) / 86_400_000) : null)
  const when = (seconds: number) => new Date(seconds * 1000).toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' })

  $effect(() => { void owner.loadSpotify().catch(() => {}) })

  async function connect() {
    if (busy) return
    busy = true; error = ''
    try {
      const auth = await owner.api.post<SpotifyAuthorization>('/users/me/spotify/authorize')
      // A full navigation, not a popup: Spotify's sign-in refuses to be framed,
      // and the redirect has to land on this same origin to keep the session.
      location.assign(auth.url)
    } catch (e) { error = e instanceof Error ? e.message : 'Could not start the Spotify sign-in.'; busy = false }
  }
  async function disconnect() {
    error = ''
    try { await owner.api.del('/users/me/spotify'); await owner.loadSpotify() }
    catch (e) { error = e instanceof Error ? e.message : 'Could not disconnect Spotify.' }
  }
</script>

<h2 class="display">Spotify</h2>
<p class="muted">
  Anyone can start a Jam by pasting its link into a room; nobody has to connect anything for that.
  Connecting your account does one extra thing: a Jam you host shows the track you are playing,
  with its art, on the card at the top of the room.
</p>

{#if link === 'unavailable'}
  <div class="callout">
    <span><b>Not set up on this Den.</b> This server has no Spotify credentials, so accounts cannot be connected here. Jam cards still work.</span>
  </div>
{:else}
  {#if link === 'reauthorize'}
    <div class="callout">
      <span><b>Spotify stopped accepting your connection.</b> Spotify retires a saved connection after 180 days, whether or not you used it. Reconnect and now playing comes back.</span>
    </div>
  {/if}

  <div class="status" class:on={link === 'connected'}>
    <span class="mark" aria-hidden="true"><Icon name={link === 'connected' ? 'check' : 'music'} size={14} /></span>
    <div>
      <b>{link === 'connected' ? `Connected as ${account.account_name || 'your Spotify account'}` : link === 'reauthorize' ? 'Connection expired' : 'Not connected'}</b>
      {#if account.connected_at}<span class="faint small">Connected {when(account.connected_at)}{#if link === 'connected' && account.expires_at} · renew by {when(account.expires_at)}{/if}</span>{/if}
    </div>
  </div>

  {#if link === 'connected' && daysLeft !== null && daysLeft <= 21}
    <p class="small warn">Spotify drops this connection in {daysLeft} {daysLeft === 1 ? 'day' : 'days'}. Reconnect whenever you like; it takes one tap.</p>
  {/if}

  {#if native}
    <p class="muted small">Spotify's sign-in has to finish in a browser on the same address as this Den, so connect from the web app. Once it is connected there, your Jams show now playing everywhere, including here.</p>
    {#if link !== 'disconnected'}
      <div class="row"><InlineConfirm action="Disconnect Spotify" sentence="Disconnect Spotify? Den deletes the saved token for good — it is not kept, disabled, or archived. Your Jams keep working; they stop showing what you are playing." confirm={disconnect} /></div>
    {/if}
  {:else}
    <div class="row">
      <button class="btn lit" onclick={connect} disabled={busy}>{busy ? 'Opening Spotify…' : link === 'disconnected' ? 'Connect Spotify' : 'Reconnect Spotify'}</button>
      {#if link !== 'disconnected'}
        <InlineConfirm action="Disconnect" sentence="Disconnect Spotify? Den deletes the saved token for good — it is not kept, disabled, or archived. Your Jams keep working; they stop showing what you are playing." confirm={disconnect} />
      {/if}
    </div>
  {/if}

  <h3 class="eyebrow">What Den can see</h3>
  <ul class="scope">
    <li>The track playing on your account right now, and its art.</li>
    <li>Nothing else. Den cannot play, pause, skip, or touch your library.</li>
    <li>Only while you are hosting a Jam, and only shown to people who can see that room.</li>
  </ul>
  <p class="faint small">
    Spotify never tells Den who is listening to a Jam, so the faces on a card are Den members who
    tapped Join and nothing more. This Den's Spotify app is in development mode, which allows five
    connected accounts across the whole instance — only Jam hosts need one.
  </p>
{/if}

{#if error}<p role="alert" class="error">{error}</p>{/if}

<style>
  .callout { display: flex; align-items: center; gap: 12px; padding: 10px 12px; margin: 14px 0; border: 1px solid var(--line-strong); border-radius: var(--r); background: var(--bg-2); font-size: 13px; }
  .status { display: flex; align-items: center; gap: 10px; padding: 12px; margin: 14px 0; border: 1px solid var(--line); border-radius: var(--r); }
  .status.on { border-color: var(--lamp-dim); }
  .status div { display: grid; gap: 2px; }
  .mark { display: grid; place-items: center; width: 28px; height: 28px; flex: none; border-radius: 50%; background: var(--bg-3); color: var(--ink-3); }
  .status.on .mark { color: var(--lamp); }
  .row { display: flex; align-items: flex-start; gap: 8px; flex-wrap: wrap; margin-bottom: 18px; }
  .small { font-size: 12px; }
  .warn { color: var(--lamp); }
  .scope { margin: 6px 0 12px; padding-left: 18px; color: var(--ink-2); font-size: 13px; display: grid; gap: 3px; }
  .error { color: var(--danger); }
</style>
