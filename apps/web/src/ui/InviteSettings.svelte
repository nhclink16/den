<script lang="ts">
  // Admins make invite links. The server keeps only a hash, so a link shows once:
  // the ones made on this visit stay listed until you leave Settings.
  import { api } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import type { Invite } from '../lib/types'
  import Icon from './Icon.svelte'
  import SettingRow from './SettingRow.svelte'
  import { settingsVisit, shownOnce } from '../lib/settings-secrets.svelte'
  import { canShareLink, shareLink } from '../lib/shell-media'

  let uses = $state(1), hours = $state(48)
  let copied = $state<string | null>(null), error = $state('')
  async function makeInvite(e: SubmitEvent) {
    e.preventDefault(); error = ''
    const visit = settingsVisit()
    try { const made = await api.post<Invite>('/invites', { uses, expires_in_hours: hours }); if (visit === settingsVisit()) shownOnce.invites = [made, ...shownOnce.invites] }
    catch (err) { error = (err as Error).message }
  }
  const inviteLink = (code: string) => `${store.origin || location.origin}/login?invite=${encodeURIComponent(code)}`
  async function copy(id: string, s: string) { await navigator.clipboard.writeText(s); copied = id; setTimeout(() => (copied = null), 1500) }
  async function share(s: string, from: EventTarget | null) {
    try { await shareLink({ title: `Join ${store.settings.instance_name}`, url: s }, from as Element) }
    catch (e) { if ((e as Error).name !== 'AbortError') error = (e as Error).message }
  }
  const canShare = canShareLink()
  // Menus of sensible values instead of number boxes.
  const useChoices = [1, 5, 10, 25, 50]
  const expiryChoices: [number, string][] = [[1, '1 hour'], [24, '1 day'], [48, '2 days'], [168, '7 days'], [720, '30 days']]
</script>

<h2 class="display">Invites</h2>

<h3 class="list-head">New invite</h3>
<form class="slist" onsubmit={makeInvite} data-find="new">
  <SettingRow label="People">
    {#snippet control()}<select class="field" bind:value={uses} aria-label="How many people can use it">{#each useChoices as n (n)}<option value={n}>{n === 1 ? '1 person' : `Up to ${n}`}</option>{/each}</select>{/snippet}
  </SettingRow>
  <SettingRow label="Expires after">
    {#snippet control()}<select class="field" bind:value={hours} aria-label="Expires after">{#each expiryChoices as [h, text] (h)}<option value={h}>{text}</option>{/each}</select>{/snippet}
  </SettingRow>
  <button class="action-row" type="submit">Create Invite Link</button>
</form>
<p class="list-foot">The link lets someone make an account here. It shows once, so send it to them straight away.</p>
{#if error}<p class="error" role="alert">{error}</p>{/if}

{#if shownOnce.invites.length}
  <h3 class="list-head">Made just now</h3>
  <div class="slist">
    {#each shownOnce.invites as inv (inv.id)}
      <div class="invite">
        <div class="text"><code class="link">{inviteLink(inv.code)}</code><span class="hint">{inv.uses_left} {inv.uses_left === 1 ? 'use' : 'uses'} left</span></div>
        <button class="icon-btn" onclick={() => copy(inv.id, inviteLink(inv.code))} aria-label="Copy invite link" title="Copy"><Icon name={copied === inv.id ? 'check' : 'copy'} /></button>
        {#if canShare}<button class="text-btn" onclick={(e) => share(inviteLink(inv.code), e.currentTarget)}>Share</button>{/if}
      </div>
    {/each}
  </div>
  <p class="list-foot">Copy these links now. They won't be shown again after you leave Settings.</p>
{/if}

<style>
  .text-btn { min-height: 44px; padding: 0 8px; color: var(--lamp); font-weight: 600; }
  .icon-btn { color: var(--lamp); }
  .invite { display: flex; align-items: center; gap: 12px; min-height: 52px; padding: 8px 8px 8px 16px; }
  .text { flex: 1; min-width: 0; display: grid; gap: 2px; }
  .link { display: block; padding: 0; background: none; border: 0; font-size: var(--fs-sm); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; user-select: all; }
  .hint { font-size: var(--fs-xs); color: var(--ink-2); }
  .error { color: var(--danger); }
</style>
