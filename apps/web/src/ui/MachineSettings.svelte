<script lang="ts">
  import InlineConfirm from './InlineConfirm.svelte'
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { store } from '../lib/store.svelte'
  import type { Host, HostEnrollment, Grant, AccessLog, TerminalRecording } from '../lib/types'
  let { section }: { section: string } = $props()
  let machines = $state<Host[]>([])
  let grants = $state<Grant[]>([])
  let log = $state<AccessLog[]>([])
  let enrollment = $state<HostEnrollment | null>(null)
  let error = $state('')
  let busy = $state(false)
  let recording = $state<Record<string, boolean>>({})
  let windows = $state(false)
  const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`
  const psQuote = (value: string) => `'${value.replaceAll("'", "''")}'`
  const command = $derived(enrollment ? windows
    ? `& ([scriptblock]::Create((Invoke-WebRequest -UseBasicParsing ${psQuote(location.origin + '/install-host.ps1')}).Content)) -Code ${psQuote(enrollment.code)}`
    : `curl -fsSL ${quote(location.origin + '/install-host.sh')} | sh -s -- ${quote(enrollment.code)}` : '')
  const user = (id: string) => store.users.get(id)?.username || id
  const machine = (id: string) => machines.find(h => h.id === id)?.name || id
  async function refresh() {
    try { [machines, grants, log] = await Promise.all([api.get<Host[]>('/hosts'), api.get<Grant[]>('/grants'), api.get<AccessLog[]>('/access/log')]); recording = Object.fromEntries(await Promise.all(machines.filter(h => h.owner_id === store.me?.id).map(async h => [h.id, (await api.get<TerminalRecording>(`/users/me/hosts/${h.id}/recording`)).enabled]))) } catch (e) { error = (e as Error).message }
  }
  onMount(() => { refresh(); const timer = setInterval(refresh, 5000); return () => clearInterval(timer) })
  async function saveRecording(h: Host, enabled: boolean) {
    if (busy) return
    busy = true; error = ''
    try { recording[h.id] = (await api.put<TerminalRecording>(`/users/me/hosts/${h.id}/recording`, { enabled })).enabled }
    catch (e) { error = (e as Error).message }
    finally { busy = false }
  }
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
  <p class="muted">Open a terminal on a machine you connect to {store.settings.instance_name}.</p>
  <p class="muted">Recording is off by default. It saves terminal output for replay in the ended card, in this server's uploads folder. The terminal toggle remembers your choice per machine. Turning it off discards the current session's recording; existing recordings stay available.</p>
  {#each machines as h (h.id)}
    <div class="row"><span><span class="dot" class:online={h.online}></span>{h.name} <small>{h.online ? 'online' : 'offline'}</small></span>{#if h.owner_id === store.me?.id}<label class="record"><input type="checkbox" checked={recording[h.id] || false} aria-disabled={busy} onchange={e => { const enabled = e.currentTarget.checked; e.currentTarget.checked = recording[h.id] || false; void saveRecording(h, enabled) }} /> Record new sessions<span class="sr-only"> on {h.name}</span></label>{/if}<InlineConfirm action="Remove" sentence={`Remove ${h.name}? Its terminals end and it must be enrolled again.`} disabled={busy} confirm={() => remove(h)} /></div>
  {:else}<p class="muted">Add a machine to open your first terminal.</p>{/each}
  <button class="btn lit" disabled={busy} onclick={add}>Add a machine</button>
  {#if enrollment}
    <div class="enrollment"><p>Run this on the machine as your own user. This code is shown once and expires in 10 minutes.</p><label><input type="checkbox" bind:checked={windows} /> Windows PowerShell</label><pre>{command}</pre><button class="btn quiet" onclick={() => enrollment = null}>Hide code</button></div>
  {/if}
{:else}
  <h2 class="display">Access</h2>
  <h3>Standing grants</h3>
  {#each grants.filter(g => g.expires_at == null) as g (g.id)}
    <div class="row"><span>{user(g.grantee_id)} · {machine(g.host_id)}<small>{g.capability === 'terminal_control' ? 'Terminal control' : 'Terminal view'}</small></span><InlineConfirm action="Revoke" sentence={`Revoke ${user(g.grantee_id)}’s access to ${machine(g.host_id)}? They must request access again.`} confirm={() => revoke(g)} /></div>
  {:else}<p class="muted">No standing grants.</p>{/each}
  <h3>Recent access</h3>
  {#each log as entry}<div class="audit"><span>{user(entry.actor_id)} · {entry.action.replaceAll('_', ' ')} · {machine(entry.host_id)}</span><time>{new Date(entry.created_at * 1000).toLocaleString()}</time></div>{:else}<p class="muted">Requests, decisions, and terminal activity appear here.</p>{/each}
{/if}
{#if error}<p role="alert">{error}</p>{/if}
<style>
  h2 { font-size: 24px; margin-bottom: 8px; } h3 { margin: 24px 0 12px; font-size: 14px; }
  .muted, small, time { color: var(--ink-3); } .muted { margin-bottom: 16px; }
  .record { display: inline-flex; align-items: center; gap: 6px; min-height: 24px; font-size: 13px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 0; border-bottom: 1px solid var(--line); }
  @media (max-width: 600px) { .row { flex-wrap: wrap; gap: 8px; } }
  .row:last-of-type { margin-bottom: 16px; } small { font: 11px var(--mono); margin-left: 8px; }
  .dot { display: inline-block; width: 7px; height: 7px; background: var(--ink-3); border-radius: 50%; margin-right: 8px; } .online { background: var(--lamp); }
  .enrollment { margin-top: 16px; padding: 16px; border: 1px solid var(--line); border-radius: var(--r); }
  pre { white-space: pre-wrap; overflow-wrap: anywhere; font: 12px/1.7 var(--mono); margin: 12px 0; user-select: all; }
  .audit { display: grid; gap: 4px; padding: 10px 0; border-bottom: 1px solid var(--line); font-size: 13px; } time { font: 10px var(--mono); }
</style>
