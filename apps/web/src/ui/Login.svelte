<script lang="ts">
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import Mark from './Mark.svelte'

  let mode = $state<'login' | 'register'>('login')
  let username = $state('')
  let password = $state('')
  let invite = $state(new URLSearchParams(location.search).get('invite') || '')
  let error = $state('')
  let busy = $state(false)

  $effect(() => { if (invite) mode = 'register' })

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    error = ''
    busy = true
    try {
      if (mode === 'login') await store.login(username.trim(), password)
      else await store.register(username.trim(), password, invite.trim())
      router.go('/', true)
    } catch (err) {
      error = (err as Error).message || 'Something went wrong'
    } finally {
      busy = false
    }
  }
</script>

<main class="wrap">
  <form class="card" onsubmit={submit}>
    <h1 class="display"><Mark size={40} /><span>{store.settings.instance_name}</span></h1>
    <p class="muted">{mode === 'login' ? 'Welcome back.' : 'Someone saved you a seat.'}</p>

    <label>
      <span class="eyebrow">Username</span>
      <input class="field" bind:value={username} autocomplete="username" autocapitalize="off" spellcheck="false" required minlength="3" maxlength="32" pattern="[a-z0-9_]+" title="Lowercase letters, digits, underscores" />
    </label>
    <label>
      <span class="eyebrow">Password</span>
      <input class="field" type="password" bind:value={password} autocomplete={mode === 'login' ? 'current-password' : 'new-password'} required minlength={mode === 'register' ? 12 : 1} />
    </label>
    {#if mode === 'register'}
      <label>
        <span class="eyebrow">Invite code</span>
        <input class="field mono" bind:value={invite} autocomplete="off" spellcheck="false" required />
      </label>
      <p class="faint small">Passwords need at least 12 characters. A sentence works.</p>
    {/if}

    {#if error}<p class="error" role="alert">{error}</p>{/if}

    <button class="btn lit" type="submit" disabled={busy}>{mode === 'login' ? 'Come in' : 'Join'}</button>
    <button class="btn quiet" type="button" onclick={() => { mode = mode === 'login' ? 'register' : 'login'; error = '' }}>
      {mode === 'login' ? 'Have an invite?' : 'Already a member? Log in'}
    </button>
  </form>
</main>

<style>
  .wrap { height: 100%; display: grid; place-items: center; padding: 24px; }
  .card {
    width: min(360px, 100%); display: flex; flex-direction: column; gap: 14px;
    padding: 28px; background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r-lg);
  }
  h1 { font-size: 40px; margin: 0; line-height: 1; display: flex; align-items: center; gap: 10px; }
  h1 span { min-width: 0; overflow-wrap: anywhere; }
  h1 :global(svg) { flex-shrink: 0; }
  h1 + p { margin: -6px 0 6px; }
  label { display: flex; flex-direction: column; gap: 6px; }
  .small { font-size: 13px; margin: -4px 0 0; }
  .error { color: var(--ember); margin: 0; font-size: 14px; }
  .btn { justify-content: center; }
</style>
