<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { store, instances } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { native } from '../lib/native'
  import { haptic } from '../lib/haptics'
  import Mark from './Mark.svelte'
  import Icon from './Icon.svelte'

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
  let forgot = $state(false)
  let shake = $state(false)
  let mark: HTMLElement

  $effect(() => { if (invite) mode = 'register' })

  const usernameOk = $derived(USERNAME_RE.test(username.trim()))
  const passwordOk = $derived(password.length >= 12)

  // The mark rises from the middle of the screen, where the boot screen (and Den Shell's launch screen) drew it.
  onMount(() => {
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return
    const r = mark.getBoundingClientRect()
    mark.animate([{ transform: `translateY(${innerHeight / 2 - (r.top + r.height / 2)}px)` }, { transform: 'none' }], { duration: 550, easing: 'cubic-bezier(.2, .8, .2, 1)' })
  })

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
      error = e.status === 401 && mode === 'login' ? 'Wrong username or password.' : e.message || 'Something went wrong. Try again.'
      haptic('warning')
      shake = false
      await tick()
      shake = true
    } finally {
      busy = false
    }
  }

  function switchMode() {
    mode = mode === 'login' ? 'register' : 'login'
    error = ''
    forgot = false
  }
</script>

<main class="wrap">
  <div class="titlebar-drag" aria-hidden="true"></div>
  <div class="light" aria-hidden="true"></div>
  <form class="door" onsubmit={submit}>
    <header class="head">
      <span class="mark" bind:this={mark}><Mark size={64} /></span>
      <h1 class="display">{store.settings.instance_name}</h1>
      <p class="muted">{mode === 'login' ? 'Welcome back.' : 'Someone saved you a seat.'}</p>
      {#if native}<button type="button" class="btn quiet server" onclick={() => instances.add()}>{store.origin} · Change server</button>{/if}
    </header>

    <!-- Placeholders name the fields; aria-label keeps the name once something is typed. -->
    <div class="fields" class:shake onanimationend={() => (shake = false)}>
      {#if mode === 'register'}
        <input class="field" bind:value={displayName} aria-label="Display name" placeholder="Display name" autocomplete="nickname" autocapitalize="words" maxlength="100" aria-describedby="hint-display" />
        <p class="note" id="hint-display">What everyone sees. Spaces, emoji, any language. Change it whenever.</p>
      {/if}

      <input class="field" bind:value={username} aria-label="Username" placeholder="Username" autocomplete="username" autocapitalize="off" spellcheck="false" required minlength="3" maxlength="32" pattern={USERNAME_PATTERN} title="3-32 letters, digits, dots, hyphens or underscores, starting and ending with a letter or digit" aria-describedby={mode === 'register' ? 'hint-username' : undefined} />
      {#if mode === 'register'}
        <!-- A rule stays put once met and gains a tick at its end, so nothing moves while you type. -->
        <p class="note" class:met={usernameOk} id="hint-username">3 to 32 letters, digits, dots, hyphens or underscores. Capitals are fine.{#if usernameOk}<span class="tick"><Icon name="check" size={13} /><span class="sr-only">Done</span></span>{/if}</p>
      {/if}

      <input class="field" type="password" bind:value={password} aria-label="Password" placeholder="Password" autocomplete={mode === 'login' ? 'current-password' : 'new-password'} required minlength={mode === 'register' ? 12 : 1} aria-describedby={mode === 'register' ? 'hint-password' : undefined} />
      {#if mode === 'register'}
        <p class="note" class:met={passwordOk} id="hint-password">At least 12 characters. A sentence works.{#if passwordOk}<span class="tick"><Icon name="check" size={13} /><span class="sr-only">Done</span></span>{/if}</p>

        <input class="field mono" bind:value={invite} aria-label="Invite code" placeholder="Invite code" autocomplete="off" autocapitalize="off" spellcheck="false" required />
      {/if}
    </div>

    {#if error}<p class="error" role="alert"><Icon name="alert" size={14} /><span>{error}</span></p>{/if}
    {#if mode === 'login'}
      <button type="button" class="forgot" aria-expanded={forgot} onclick={() => (forgot = !forgot)}>Forgot password?</button>
      {#if forgot}<p class="note">Den has no email to send a reset to. Ask an admin: they can set you a new password.</p>{/if}
    {/if}

    <div class="actions">
      <button class="btn lit" type="submit" disabled={busy}>{busy ? (mode === 'login' ? 'Coming in…' : 'Joining…') : mode === 'login' ? 'Come in' : 'Join'}</button>
      <button class="btn quiet" type="button" onclick={switchMode}>
        {mode === 'login' ? 'Have an invite?' : 'Already a member? Log in'}
      </button>
      <!-- Only useful with the desktop app installed, so it comes after the form. -->
      {#if !native && invite && mode === 'register'}<a class="btn quiet" href={`den://join?url=${encodeURIComponent(location.origin)}&invite=${encodeURIComponent(invite)}`}>Have the desktop app? Open in Den</a>{/if}
    </div>
  </form>
</main>

<style>
  /* No card: the page is the door. Each screen pads its own bottom (Den Shell draws edge to edge). */
  .wrap {
    position: relative; height: 100%; overflow: auto; isolation: isolate; display: flex; flex-direction: column;
    padding: max(24px, env(safe-area-inset-top, 0px)) max(24px, env(safe-area-inset-right, 0px)) max(24px, env(safe-area-inset-bottom, 0px)) max(24px, env(safe-area-inset-left, 0px));
  }
  /* Light spilling out of an open door: a warm pool, brightest where the mark's
     doorway sits. It switches on as the page loads. */
  .light {
    position: fixed; inset: 0; z-index: -1; pointer-events: none;
    background:
      radial-gradient(ellipse 34% 42% at 50% var(--pool-y), var(--pool), transparent 70%),
      radial-gradient(ellipse 80% 70% at 50% calc(var(--pool-y) + 10%), color-mix(in srgb, var(--pool) 35%, transparent), transparent 75%);
    --pool-y: 30%;
  }
  .door { width: min(380px, 100%); margin: auto; display: flex; flex-direction: column; }
  /* On a phone: the mark and greeting in the upper third, the button near the bottom, where a thumb is. */
  @media (max-width: 599px) {
    .door { flex: 1 0 auto; margin: 0 auto; }
    .head { padding-top: 6vh; }
    .actions { margin-top: auto; }
    .light { --pool-y: 18%; }
  }

  .head { display: flex; flex-direction: column; align-items: center; text-align: center; margin-bottom: 32px; }
  h1 { font-size: 34px; line-height: 1.1; margin: 16px 0 0; overflow-wrap: anywhere; }
  .head p { margin: 6px 0 0; font-size: 17px; }
  .server { margin-top: 8px; font-size: var(--fs-sm); white-space: normal; }
  .mark { position: relative; display: grid; color: var(--ink); }
  .mark::before {
    content: ''; position: absolute; left: 30%; right: 30%; top: 40%; bottom: -10%; z-index: -1;
    border-radius: 50% 50% 0 0; background: var(--lamp); filter: blur(14px); opacity: .55;
  }

  /* Filled, borderless fields, as on iPhone. A keyboard still gets app.css's focus ring. */
  .fields { display: flex; flex-direction: column; gap: 12px; }
  .door .field {
    min-height: 50px; padding: 12px 16px; font-size: 16px; border: 0; border-radius: 12px;
    background: color-mix(in srgb, var(--ink-3) 14%, transparent);
  }
  /* One footer style under a field: grey, tucked up to it, and a tick at the end once the rule is met. */
  .note { margin: -6px 16px 0; font-size: var(--fs-sm); line-height: 1.4; color: var(--ink-3); }
  .tick { display: inline-flex; vertical-align: -2px; margin-left: 6px; color: var(--success); }
  .error { display: flex; gap: 6px; align-items: baseline; margin: 12px 16px 0; font-size: var(--fs-sm); line-height: 1.4; color: var(--danger); }
  .error :global(svg) { flex-shrink: 0; align-self: center; }
  .forgot { align-self: flex-start; margin: 10px 16px 0; padding: 4px 0; font-size: var(--fs-sm); color: var(--lamp); }
  .forgot + .note { margin-top: 2px; }

  .actions { display: flex; flex-direction: column; gap: 6px; padding-top: 32px; }
  .btn { justify-content: center; }
  .btn.lit { min-height: 50px; font-size: 17px; }
  .btn:disabled { cursor: default; }

  @media (prefers-reduced-motion: no-preference) {
    .light { animation: lamp-on 1.1s .15s var(--ease-out) both; }
    .mark::before { animation: lamp-on .9s .1s var(--ease-out) both; }
    .door > :not(.head), .head > :not(.mark) { animation: rise .5s .15s var(--ease-out) both; }
    .fields.shake { animation: shake .4s; }
  }
  @keyframes lamp-on { 0% { opacity: 0; } 35% { opacity: .45; } 45% { opacity: .2; } 100% { } }
  @keyframes rise { from { opacity: 0; transform: translateY(10px); } }
  @keyframes shake { 20% { transform: translateX(-8px); } 40% { transform: translateX(7px); } 60% { transform: translateX(-5px); } 80% { transform: translateX(3px); } }
</style>
