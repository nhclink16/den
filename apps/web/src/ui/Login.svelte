<script lang="ts">
  import { store, instances } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { native } from '../lib/native'
  import Mark from './Mark.svelte'

  // Kept as one string so the attribute below and the check beside it cannot drift.
  // Mirrors `username()` in crates/den-server/src/auth.rs.
  // The hyphen is escaped because browsers compile the HTML pattern attribute with the
  // `v` flag, where an unescaped `-` inside a class is a syntax error and the whole
  // pattern is discarded, silently disabling client-side validation.
  const USERNAME_PATTERN = '[A-Za-z0-9][A-Za-z0-9_.\\-]{1,30}[A-Za-z0-9]'
  const USERNAME_RE = new RegExp(`^${USERNAME_PATTERN}$`)

  let mode = $state<'login' | 'register'>('login')
  let username = $state('')
  let displayName = $state('')
  let password = $state('')
  let invite = $state(new URLSearchParams(location.search).get('invite') || '')
  let error = $state('')
  let busy = $state(false)

  $effect(() => { if (invite) mode = 'register' })

  const usernameOk = $derived(USERNAME_RE.test(username.trim()))
  const passwordOk = $derived(password.length >= 12)

  async function submit(e: SubmitEvent) {
    e.preventDefault()
    error = ''
    busy = true
    try {
      if (mode === 'login') await store.login(username.trim(), password)
      else await store.register(username.trim(), password, invite.trim(), displayName)
      router.go('/', true)
    } catch (err) {
      // The server returns one 401 string for a dozen unrelated cases. On the login
      // form there is only one thing it can mean, so say that instead.
      const e = err as { status?: number; message?: string }
      error = e.status === 401
        ? "That username and password don't match. Check for typos, or ask an admin to reset your password."
        : e.message || 'Something went wrong. Try again.'
    } finally {
      busy = false
    }
  }
</script>

<main class="wrap">
  <form class="card" onsubmit={submit}>
    <h1 class="display"><Mark size={40} /><span>{store.settings.instance_name}</span></h1>
    {#if native}<button type="button" class="btn quiet mono" onclick={() => instances.add()}>{store.origin} · Change server</button>{:else if invite}<a class="btn quiet" href={`den://join?url=${encodeURIComponent(location.origin)}&invite=${encodeURIComponent(invite)}`}>Open in Den</a>{/if}
    <p class="muted">{mode === 'login' ? 'Welcome back.' : 'Someone saved you a seat.'}</p>

    {#if mode === 'register'}
      <label>
        <span class="eyebrow">Display name</span>
        <input class="field" bind:value={displayName} autocomplete="nickname" autocapitalize="words" maxlength="100" placeholder={username.trim() || 'Andy'} aria-describedby="hint-display" />
      </label>
      <p class="hint" id="hint-display">What everyone sees. Spaces, emoji, any language. Change it whenever.</p>
    {/if}

    <label>
      <span class="eyebrow">Username</span>
      <input class="field" bind:value={username} autocomplete="username" autocapitalize="off" spellcheck="false" required minlength="3" maxlength="32" pattern={USERNAME_PATTERN} title="3-32 letters, digits, dots, hyphens or underscores, starting and ending with a letter or digit" aria-describedby={mode === 'register' ? 'hint-username' : undefined} />
    </label>
    {#if mode === 'register'}
      <p class="hint" class:done={usernameOk} id="hint-username">
        <span class="tick" aria-hidden="true">{usernameOk ? '✓' : '·'}</span>
        3 to 32 letters, digits, dots, hyphens or underscores. Capitals are fine.
      </p>
    {/if}

    <label>
      <span class="eyebrow">Password</span>
      <input class="field" type="password" bind:value={password} autocomplete={mode === 'login' ? 'current-password' : 'new-password'} required minlength={mode === 'register' ? 12 : 1} aria-describedby={mode === 'register' ? 'hint-password' : undefined} />
    </label>
    {#if mode === 'register'}
      <p class="hint" class:done={passwordOk} id="hint-password">
        <span class="tick" aria-hidden="true">{passwordOk ? '✓' : '·'}</span>
        At least 12 characters. A sentence works.
      </p>

      <label>
        <span class="eyebrow">Invite code</span>
        <input class="field mono" bind:value={invite} autocomplete="off" spellcheck="false" required />
      </label>
    {/if}

    <p class="error" role="alert">{error}</p>

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
  /* Stated before you type, not after a rejected submit. Satisfied rules recede
     rather than vanish, so the form does not reflow while you are filling it in. */
  .hint {
    font-size: 13px; line-height: 1.4; margin: -8px 0 0;
    color: var(--ink-2); display: flex; gap: 6px; align-items: baseline;
    transition: color .12s;
  }
  .hint.done { color: var(--ink-3); }
  .tick { font-variant-numeric: tabular-nums; width: 1em; flex-shrink: 0; text-align: center; }
  .hint.done .tick { color: var(--accent); }
  .error { color: var(--danger); margin: 0; font-size: 14px; min-height: 1.4em; line-height: 1.4; }
  .btn { justify-content: center; }
</style>
