<script lang="ts">
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import type { Host, HostEnrollment, Grant, AccessLog } from '../lib/types'
  let { section }: { section: string } = $props()
  let machines = $state<Host[]>([])
  let grants = $state<Grant[]>([])
  let log = $state<AccessLog[]>([])
  let enrollment = $state<HostEnrollment | null>(null)
  let error = $state('')
  let busy = $state(false)
  const user = (id: string) => store.users.get(id)?.username || id
  const machine = (id: string) => machines.find(h => h.id === id)?.name || id
  async function refresh() {
    try { [machines, grants, log] = await Promise.all([api.get<Host[]>('/hosts'), api.get<Grant[]>('/grants'), api.get<AccessLog[]>('/access/log')]) } catch (e) { error = (e as Error).message }
  }
  onMount(() => { refresh(); const timer = setInterval(refresh, 5000); return () => clearInterval(timer) })
  async function add() {
    busy = true; error = ''
    try { enrollment = await api.post<HostEnrollment>('/hosts/enroll', {}) } catch (e) { error = (e as Error).message } finally { busy = false }
  }
  async function remove(h: Host) {
    busy = true; error = ''
    try { await api.del(`/hosts/${h.id}`); await refresh() } catch (e) { error = (e as Error).message } finally { busy = false }
  }
  async function revoke(g: Grant) {
    try { await api.del(`/grants/${g.id}`); await refresh() } catch (e) { error = (e as Error).message }
  }
</script>
{#if section === 'machines'}
  <h2 class="display">Machines</h2>
  <p class="muted">Open a terminal on a machine you connect to Den.</p>
  {#each machines as h}
    <div class="row"><span><span class="dot" class:online={h.online}></span>{h.name} <small>{h.online ? 'online' : 'offline'}</small></span><button class="btn quiet" disabled={busy} onclick={() => remove(h)}>Remove</button></div>
  {:else}<p class="muted">Add a machine to open your first terminal.</p>{/each}
  <button class="btn lit" disabled={busy} onclick={add}>Add a machine</button>
  {#if enrollment}
    <div class="enrollment"><p>Run this on the machine. This code is shown once and expires in 10 minutes.</p><pre>cargo install --git https://github.com/nhclink16/den --locked den-host &amp;&amp; den-host login '{enrollment.code}' &amp;&amp; den-host install</pre><button class="btn quiet" onclick={() => enrollment = null}>Hide code</button></div>
  {/if}
{:else}
  <h2 class="display">Access</h2>
  <h3>Standing grants</h3>
  {#each grants.filter(g => g.expires_at == null) as g}
    <div class="row"><span>{user(g.grantee_id)} · {machine(g.host_id)}<small>{g.capability === 'terminal_control' ? 'Terminal control' : 'Terminal view'}</small></span><button class="btn quiet" onclick={() => revoke(g)}>Revoke</button></div>
  {:else}<p class="muted">No standing grants.</p>{/each}
  <h3>Recent access</h3>
  {#each log as entry}<div class="audit"><span>{user(entry.actor_id)} · {entry.action.replaceAll('_', ' ')} · {machine(entry.host_id)}</span><time>{new Date(entry.created_at * 1000).toLocaleString()}</time></div>{:else}<p class="muted">Requests, decisions, and terminal activity appear here.</p>{/each}
{/if}
{#if error}<p role="alert">{error}</p>{/if}
<style>
  h2 { font-size: 24px; margin-bottom: 8px; } h3 { margin: 24px 0 12px; font-size: 14px; }
  .muted, small, time { color: var(--ink-3); } .muted { margin-bottom: 16px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 0; border-bottom: 1px solid var(--line); }
  .row:last-of-type { margin-bottom: 16px; } small { font: 11px var(--mono); margin-left: 8px; }
  .dot { display: inline-block; width: 7px; height: 7px; background: var(--ink-3); border-radius: 50%; margin-right: 8px; } .online { background: var(--lamp); }
  .enrollment { margin-top: 16px; padding: 16px; border: 1px solid var(--line); border-radius: var(--r); }
  pre { white-space: pre-wrap; overflow-wrap: anywhere; font: 12px/1.7 var(--mono); margin: 12px 0; user-select: all; }
  .audit { display: grid; gap: 4px; padding: 10px 0; border-bottom: 1px solid var(--line); font-size: 13px; } time { font: 10px var(--mono); }
</style>
