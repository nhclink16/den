<script lang="ts">
  import { onMount } from 'svelte'
  import type { ObjectProps } from '..'
  import { store } from '../../lib/store.svelte'
  import { api } from '../../lib/api'
  import { terminals, load, catalog } from './state.svelte'
  import type { AccessRequest } from '../../lib/types'
  let { object }: ObjectProps = $props()
  let now = $state(Date.now())
  let error = $state('')
  let busy = $state(false)
  const r = $derived(terminals.requests[object.id])
  const requester = $derived(r && store.user(r.requester_id))
  const expired = $derived(r?.status === 'pending' && r.expires_at * 1000 <= now)
  const duration = $derived(r?.standing ? 'Standing access' : r?.duration_minutes === 60 ? '1 hour' : `${r?.duration_minutes} minutes`)
  $effect(() => {void object.version; load(object).catch(e => error = e.message)})
  onMount(() => {const tick = setInterval(() => now = Date.now(), 1000); return () => clearInterval(tick)})
  async function decide(allow: boolean) {
    busy = true
    try {terminals.requests[object.id] = await api.post<AccessRequest>(`/requests/${object.id}/decide`, { allow }); await catalog()} catch (e) {error = (e as Error).message} finally {busy = false}
  }
</script>
<div class="request-card" data-testid="access-request-card" data-object-id={object.id}>
  {#if r}<p>{requester?.username || 'Someone'} wants a terminal on {r.host_name} {#if requester?.bot}<span class="badge">bot</span>{/if}</p><div class="detail">{r.capability === 'terminal_control' ? 'Terminal control' : 'Terminal view'} · {duration}</div>
    {#if expired}<div class="decision">Expired</div>
    {:else if r.status === 'pending' && r.owner_id === store.me?.id}<div class="buttons"><button class="btn lit" disabled={busy} onclick={() => decide(true)}>Allow</button><button class="btn quiet" disabled={busy} onclick={() => decide(false)}>Deny</button></div>
    {:else}<div class="decision">{r.status === 'allowed' ? r.standing ? 'Standing access allowed' : `Allowed for ${duration}` : r.status === 'denied' ? 'Denied' : 'Waiting for a decision'}</div>{/if}
  {/if}
  {#if error}<p role="alert">{error}</p>{/if}
</div>
<style>
  .request-card { width: 320px; max-width: 100%; padding: 14px; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-2); margin: 6px 0; }
  p { font-size: 14px; margin: 0 0 8px; } .detail { color: var(--ink-2); font-size: 12px; } .buttons { display: flex; gap: 8px; margin-top: 14px; } .decision { font: 11px var(--mono); margin-top: 12px; color: var(--ink-2); } .badge { font: 9px var(--mono); border: 1px solid var(--line); border-radius: 3px; padding: 1px 3px; }
</style>
