<script lang="ts">
  import { onMount } from 'svelte'
  import { Store, instances } from '../lib/store.svelte'
  import { apiFor } from '../lib/api'
  import { originOf } from '../lib/native'
  import type { Instance } from '../lib/types'
  import Mark from './Mark.svelte'
  let dialog: HTMLDialogElement
  let url = $state(instances.url)
  let invite = $state(instances.invite)
  let preview = $state<Instance | null>(null)
  let origin = $state('')
  let username = $state(''), password = $state(''), error = $state(''), busy = $state(false)
  let mode = $state<'login' | 'register'>(instances.invite ? 'register' : 'login')
  onMount(() => { dialog.showModal() })
  async function submit(e: SubmitEvent) {
    e.preventDefault(); busy = true; error = ''
    try {
      if (!preview) {
        const link = new URL(url.includes('://') ? url : `https://${url}`)
        origin = originOf(link.protocol === 'den:' ? link.searchParams.get('url') || '' : url)
        invite = link.searchParams.get('invite') || invite
        if (invite) mode = 'register'
        preview = await apiFor(origin).get<Instance>('/instance')
      } else {
        const s = instances.stores.find(s => s.origin === origin) || new Store(origin)
        if (!s.me) {
          if (mode === 'register') await s.register(username.trim(), password, invite.trim())
          else await s.login(username.trim(), password)
        }
        instances.select(s); instances.adding = false
      }
    } catch (e) { error = (e as Error).message } finally { busy = false }
  }
</script>
<dialog bind:this={dialog} onclose={() => instances.adding = false} aria-labelledby="add-server-title">
  <form onsubmit={submit}>
    <div class="heading"><h2 id="add-server-title" class="display">{preview ? preview.instance_name : 'Add a server'}</h2><button type="button" class="btn quiet" aria-label="Close" onclick={() => dialog.close()}>×</button></div>
    {#if !preview}
      <p class="muted">Enter a Den server URL or an invite link.</p>
      <label>Server URL<input class="field mono" bind:value={url} placeholder="https://denchat.app" required /></label>
    {:else}
      <div class="identity">{#if preview.icon_url}<img src={new URL(preview.icon_url, origin).href} alt="" width="32" height="32" />{:else}<Mark size={32} />{/if}<span class="mono muted">{origin}</span><button type="button" class="btn quiet" onclick={() => preview = null}>Change</button></div>
      <label>Username<input class="field" bind:value={username} autocomplete="username" required pattern="[a-z0-9_]+" /></label>
      <label>Password<input class="field" bind:value={password} type="password" autocomplete={mode === 'login' ? 'current-password' : 'new-password'} required minlength={mode === 'register' ? 12 : 1} /></label>
      {#if mode === 'register'}<label>Invite code<input class="field mono" bind:value={invite} required /></label>{/if}
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <button class="btn lit" disabled={busy}>{busy ? 'Connecting…' : !preview ? 'Continue' : mode === 'register' ? 'Join server' : 'Log in'}</button>
    {#if preview}<button class="btn quiet" type="button" onclick={() => mode = mode === 'login' ? 'register' : 'login'}>{mode === 'login' ? 'Have an invite?' : 'Already a member? Log in'}</button>{/if}
  </form>
</dialog>
<style>
  dialog { width:min(440px, calc(100vw - 40px)); padding:24px; border:1px solid var(--line); border-radius:var(--r-lg); background:var(--bg2); color:var(--ink); }
  dialog::backdrop { background:var(--scrim); }
  form, label { display:grid; gap:10px; } form { gap:16px; }
  h2, p { margin:0; } h2 { flex:1; }
  .heading, .identity { display:flex; align-items:center; gap:10px; }
  .identity { font-size:12px; overflow-wrap:anywhere; }
  .identity span { flex:1; min-width:0; }
  .error { color:var(--danger); }
</style>
