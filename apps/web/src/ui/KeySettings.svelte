<script lang="ts">
  // API keys act as you; agents are members of their own that talk through a key.
  // Both pages list the same tokens, split by whose they are.
  import { api } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import type { BotCreated, Token, TokenSecret } from '../lib/types'
  import Icon from './Icon.svelte'
  import InlineConfirm from './InlineConfirm.svelte'
  import SettingRow from './SettingRow.svelte'
  import FormDialog from './FormDialog.svelte'
  import { settingsVisit, shownOnce } from '../lib/settings-secrets.svelte'

  let { section }: { section: 'keys' | 'agents' } = $props()
  let tokens = $state<Token[]>([])
  let botName = $state(''), botDisplay = $state(''), tokenName = $state('')
  let copied = $state(false), adding = $state(false)
  async function load() { try { tokens = await api.get<Token[]>('/tokens') } catch { tokens = [] } }
  $effect(() => { void section; load() })
  // The dialog shows a failure and stays open.
  async function createBot() {
    const visit = settingsVisit()
    const b = await api.post<BotCreated>('/bots', { username: botName.trim(), display_name: botDisplay.trim() || botName.trim() })
    if (visit === settingsVisit()) shownOnce.reveal = { label: `${b.user.display_name} is ready. Its token, shown once:`, token: b.credential.token }
    botName = ''; botDisplay = ''
    await store.resync(); await load()
  }
  async function createToken() {
    const visit = settingsVisit()
    const t = await api.post<TokenSecret>('/tokens', { name: tokenName.trim() || 'token', user_id: null })
    if (visit === settingsVisit()) shownOnce.reveal = { label: `API key "${t.credential.name}" acts as you. Shown once:`, token: t.token }
    tokenName = ''; await load()
  }
  async function revoke(t: Token) { await api.del(`/tokens/${t.id}`); await load() }
  async function copy(s: string) { await navigator.clipboard.writeText(s); copied = true; setTimeout(() => (copied = false), 1500) }
  const shown = $derived(tokens.filter((t) => (section === 'keys') === (t.user_id === store.me?.id)))
</script>

<h2 class="display">{section === 'keys' ? 'API keys' : 'Agents'}</h2>

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

<!-- The add action is the first row; the list follows only when there is something in it. -->
<div class="slist gap-top" data-find="new">
  <button class="action-row add" onclick={() => (adding = true)}><Icon name="plus" size={15} /> {section === 'keys' ? 'New API Key' : 'New Agent'}</button>
  {#each shown as t (t.id)}
    {@const owner = store.user(t.user_id)}
    <SettingRow label={t.name} hint={section === 'agents' ? (owner ? owner.display_name || owner.username : 'agent') : undefined}>
      {#snippet control()}<InlineConfirm action="Revoke" sentence={`Revoke "${t.name}"? Anything using it stops working.`} confirm={() => revoke(t)} />{/snippet}
    </SettingRow>
  {/each}
</div>
<p class="list-foot">
  {#if section === 'keys'}A key acts as you, for scripts, the <code>den</code> CLI or a relay. Anyone holding it can do what you can, so keep it private.
  {:else}An agent is a member of its own, with a name and picture, that talks through its key.{/if}
</p>

{#if section === 'keys'}
  <FormDialog bind:open={adding} title="New API Key" action="Create" onsubmit={createToken}>
    <input class="field" bind:value={tokenName} placeholder="What it's for, e.g. minecraft-relay" aria-label="What the key is for" autocomplete="off" />
  </FormDialog>
{:else}
  <FormDialog bind:open={adding} title="New Agent" action="Create" disabled={!/^[a-z0-9_]{3,32}$/.test(botName.trim())} onsubmit={createBot}>
    <input class="field" bind:value={botName} placeholder="username, e.g. hermes" aria-label="Agent username" autocapitalize="off" autocomplete="off" />
    <input class="field" bind:value={botDisplay} placeholder="Display name (optional)" aria-label="Agent display name" autocomplete="off" />
  </FormDialog>
{/if}
<style>
  .reveal { display: grid; gap: 10px; padding: 14px 16px; margin: 18px 0 0; background: var(--sel); border-radius: var(--r-lg); }
  .secret { display: block; padding: 8px 10px; background: var(--bg); border-radius: var(--r); word-break: break-all; user-select: all; }
  .buttons { display: flex; flex-wrap: wrap; gap: 8px; }
  .small { font-size: var(--fs-sm); }
  .gap-top { margin-top: 18px; }
  button.action-row.add { display: flex; align-items: center; gap: 6px; }
</style>
