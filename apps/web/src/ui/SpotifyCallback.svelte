<script lang="ts">
  // Where Spotify sends the browser back. `spotify` is not an API segment on
  // den-server, so this path serves index.html and the exchange happens from
  // here, with the session cookie this origin already holds.
  import { store } from '../lib/store.svelte'
  import { router, type Route } from '../lib/router.svelte'
  import type { SpotifyAccount } from '../lib/types'

  let { route }: { route: Extract<Route, { name: 'spotify-callback' }> } = $props()
  let error = $state('')

  $effect(() => {
    const { code, state, error: denied } = route
    // The code is single use and spent the moment it is exchanged. Take it out of
    // the address bar before anything can reload the page onto a dead one.
    history.replaceState({}, '', '/spotify/callback')
    if (denied) { error = denied === 'access_denied' ? 'You cancelled the Spotify sign-in. Nothing was connected.' : 'Spotify did not complete the sign-in. Try connecting again.'; return }
    if (!code || !state) { error = 'That Spotify link was incomplete. Start again from Settings.'; return }
    void (async () => {
      try {
        store.spotify = await store.api.post<SpotifyAccount>('/users/me/spotify/callback', { code, state })
        router.go('/settings/spotify', true)
      } catch (e) { error = e instanceof Error ? e.message : 'Could not finish connecting Spotify.' }
    })()
  })
</script>

<section class="callback">
  {#if error}
    <h1 class="display">Spotify did not connect</h1>
    <p role="alert" class="muted">{error}</p>
    <a href="/settings/spotify" onclick={(e) => { e.preventDefault(); router.go('/settings/spotify', true) }}>Back to Spotify settings</a>
  {:else}
    <h1 class="display">Connecting Spotify</h1>
    <p class="muted" role="status">Trading the sign-in for a token. This takes a second.</p>
  {/if}
</section>

<style>
  .callback { flex: 1; display: grid; place-content: center; justify-items: center; gap: 8px; text-align: center; padding: 24px; }
  h1 { margin: 0; font-size: 22px; }
  p { margin: 0; max-width: 42ch; font-size: 13px; }
</style>
