<script lang="ts">
  // Admins make invite links. The server keeps only a hash, so a link shows once:
  // the ones made on this visit stay listed until you leave Settings.
  import { api } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import type { Invite } from '../lib/types'
  import Icon from './Icon.svelte'
  import SettingRow from './SettingRow.svelte'
  import { shownOnce } from '../lib/settings-secrets.svelte'

  let uses = $state(1), hours = $state(48)
  let copied = $state<string | null>(null), error = $state('')
  async function makeInvite(e: SubmitEvent) {
    e.preventDefault(); error = ''
    try { const made = await api.post<Invite>('/invites', { uses, expires_in_hours: hours }); shownOnce.invites = [made, ...shownOnce.invites] }
    catch (err) { error = (err as Error).message }
  }
  const inviteLink = (code: string) => `${store.origin || location.origin}/login?invite=${encodeURIComponent(code)}`
  async function copy(id: string, s: string) { await navigator.clipboard.writeText(s); copied = id; setTimeout(() => (copied = null), 1500) }
</script>

<h2 class="display">Invites</h2>
<p class="muted">An invite link lets someone make an account here. Send it to them directly.</p>

<h3 class="list-head">New invite</h3>
<form class="slist" onsubmit={makeInvite} data-find="new">
  <SettingRow label="Uses" hint="How many people can sign up with it.">
    {#snippet control()}<input class="field num" type="number" min="1" max="50" bind:value={uses} aria-label="Uses" />{/snippet}
  </SettingRow>
  <SettingRow label="Expires in hours" hint="Up to 720, which is 30 days.">
    {#snippet control()}<input class="field num" type="number" min="1" max="720" bind:value={hours} aria-label="Expires in hours" />{/snippet}
  </SettingRow>
  <button class="action-row" type="submit">Create invite link</button>
</form>
{#if error}<p class="error" role="alert">{error}</p>{/if}

{#if shownOnce.invites.length}
  <h3 class="list-head">Made just now</h3>
  <div class="slist">
    {#each shownOnce.invites as inv (inv.id)}
      <div class="invite">
        <div class="text"><code class="link">{inviteLink(inv.code)}</code><span class="hint">{inv.uses_left} {inv.uses_left === 1 ? 'use' : 'uses'} left</span></div>
        <button class="icon-btn" onclick={() => copy(inv.id, inviteLink(inv.code))} aria-label="Copy invite link" title="Copy"><Icon name={copied === inv.id ? 'check' : 'copy'} /></button>
      </div>
    {/each}
  </div>
  <p class="list-foot">Copy these links now. They won't be shown again after you leave Settings.</p>
{:else}
  <p class="list-foot">No invites made on this visit yet. Links show once, right after you make them.</p>
{/if}

<style>
  .num { width: 88px; min-width: 0; text-align: end; }
  .invite { display: flex; align-items: center; gap: 12px; min-height: 52px; padding: 8px 8px 8px 16px; }
  .text { flex: 1; min-width: 0; display: grid; gap: 2px; }
  .link { display: block; padding: 0; background: none; border: 0; font-size: var(--fs-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; user-select: all; }
  .hint { font-size: var(--fs-xs); color: var(--ink-2); }
  .error { color: var(--danger); }
</style>
