<script lang="ts">
  // API keys act as you; agents are members of their own that talk through a key.
  // Both pages list the same tokens, split by whose they are.
  import { api } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import type { BotCreated, Token, TokenSecret } from '../lib/types'
  import Icon from './Icon.svelte'
  import InlineConfirm from './InlineConfirm.svelte'
  import SettingRow from './SettingRow.svelte'
  import { settingsVisit, shownOnce } from '../lib/settings-secrets.svelte'

  let { section }: { section: 'keys' | 'agents' } = $props()
  let tokens = $state<Token[]>([])
  let botName = $state(''), botDisplay = $state(''), tokenName = $state('')
  let copied = $state(false), error = $state('')
  async function load() { try { tokens = await api.get<Token[]>('/tokens') } catch { tokens = [] } }
  $effect(() => { void section; load() })
  async function createBot(e: SubmitEvent) {
    e.preventDefault(); error = ''
    const visit = settingsVisit()
    try {
      const b = await api.post<BotCreated>('/bots', { username: botName.trim(), display_name: botDisplay.trim() || botName.trim() })
      if (visit === settingsVisit()) shownOnce.reveal = { label: `${b.user.display_name} is ready. Its token, shown once:`, token: b.credential.token }
      botName = ''; botDisplay = ''
      await store.resync(); await load()
    } catch (err) { error = (err as Error).message }
  }
  async function createToken(e: SubmitEvent) {
    e.preventDefault(); error = ''
    const visit = settingsVisit()
    try {
      const t = await api.post<TokenSecret>('/tokens', { name: tokenName.trim() || 'token', user_id: null })
      if (visit === settingsVisit()) shownOnce.reveal = { label: `API key "${t.credential.name}" acts as you. Shown once:`, token: t.token }
      tokenName = ''; await load()
    } catch (err) { error = (err as Error).message }
  }
  async function revoke(t: Token) { await api.del(`/tokens/${t.id}`); await load() }
  async function copy(s: string) { await navigator.clipboard.writeText(s); copied = true; setTimeout(() => (copied = false), 1500) }
  const shown = $derived(tokens.filter((t) => (section === 'keys') === (t.user_id === store.me?.id)))
</script>

{#if section === 'keys'}
  <h2 class="display">API keys</h2>
  <p class="muted">An API key acts as you: scripts, the <code>den</code> CLI, or a relay like the Minecraft one. Anyone holding it can do what you can, so keep it private.</p>
{:else}
  <h2 class="display">Agents</h2>
  <p class="muted">An agent is a member of its own, with a name and picture, that talks through its key. No app registration, no OAuth.</p>
{/if}

{#if shownOnce.reveal}
  <div class="reveal" role="status">
    <div>{shownOnce.reveal.label}</div>
    <code class="secret">{shownOnce.reveal.token}</code>
    <div class="buttons">
      <button class="btn" onclick={() => copy(shownOnce.reveal!.token)}><Icon name={copied ? 'check' : 'copy'} /> {copied ? 'Copied' : 'Copy'}</button>
      <button class="btn quiet" onclick={() => (shownOnce.reveal = null)}>I saved it</button>
    </div>
    <div class="faint small">Use it as <code>DEN_TOKEN</code> with the <code>den</code> CLI, or as a bearer token.</div>
  </div>
{/if}

{#if section === 'keys'}
  <h3 class="list-head">New API key</h3>
  <form class="slist" onsubmit={createToken} data-find="new">
    <div class="field-row"><input class="field" bind:value={tokenName} placeholder="What is it for, e.g. minecraft-relay" aria-label="What the key is for" /></div>
    <button class="action-row" type="submit">Create key</button>
  </form>
{:else}
  <h3 class="list-head">New agent</h3>
  <form class="slist" onsubmit={createBot} data-find="new">
    <div class="field-row"><input class="field" bind:value={botName} placeholder="username, e.g. hermes" aria-label="Agent username" pattern={'[a-z0-9_]{3,32}'} required /></div>
    <div class="field-row"><input class="field" bind:value={botDisplay} placeholder="Display name (optional)" aria-label="Agent display name" /></div>
    <button class="action-row" type="submit">Create agent</button>
  </form>
{/if}
{#if error}<p class="error" role="alert">{error}</p>{/if}

<h3 class="list-head">{section === 'keys' ? 'Your keys' : 'Agent keys'}</h3>
{#if !shown.length}<p class="list-foot">None yet.</p>{:else}
  <div class="slist">
    {#each shown as t (t.id)}
      {@const owner = store.user(t.user_id)}
      <SettingRow label={t.name} hint={section === 'agents' ? (owner ? owner.display_name || owner.username : 'agent') : undefined}>
        {#snippet control()}<InlineConfirm action="Revoke" sentence={`Revoke "${t.name}"? Anything using it stops working.`} confirm={() => revoke(t)} />{/snippet}
      </SettingRow>
    {/each}
  </div>
{/if}

<style>
  .reveal { display: grid; gap: 10px; padding: 14px 16px; margin: 18px 0 0; background: var(--sel); border-radius: var(--r-lg); }
  .secret { display: block; padding: 8px 10px; background: var(--bg); border-radius: var(--r); word-break: break-all; user-select: all; }
  .buttons { display: flex; flex-wrap: wrap; gap: 8px; }
  .small { font-size: var(--fs-sm); }
  .field-row { padding: 8px 12px; }
  .error { color: var(--danger); }
</style>
