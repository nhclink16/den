<script lang="ts">
  import InlineConfirm from './InlineConfirm.svelte'
  import SettingRow from './SettingRow.svelte'
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
  {@const owned = machines.filter(h => h.owner_id === store.me?.id)}
  <h2 class="display">Machines</h2>
  <p class="muted">Open a terminal on a machine you connect to {store.settings.instance_name}.</p>
  <h3 class="list-head">Machines</h3>
  <div class="slist">
    {#each machines as h (h.id)}
      <div class="machine">
        <span class="dot" class:online={h.online} aria-hidden="true"></span>
        <span class="text"><span class="nm">{h.name}</span><span class="sub">{h.online ? 'Online' : 'Offline'}</span></span>
        <InlineConfirm action="Remove" sentence={`Remove ${h.name}? Its terminals end and it must be enrolled again.`} disabled={busy} confirm={() => remove(h)} />
      </div>
    {/each}
    <button class="action-row" data-find="add" disabled={busy} onclick={add}>Add a machine</button>
  </div>
  {#if !machines.length}<p class="list-foot">Add a machine to open your first terminal.</p>{/if}
  {#if enrollment}
    <h3 class="list-head">Add a machine</h3>
    <div class="slist">
      <div class="pad">
        <p>Run this on the machine as your own user. This code is shown once and expires in 10 minutes.</p>
        <pre>{command}</pre>
      </div>
      <SettingRow label="Windows PowerShell" hint="Show the command for Windows instead.">
        {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="Windows PowerShell" bind:checked={windows} />{/snippet}
      </SettingRow>
      <button class="action-row" onclick={() => enrollment = null}>Hide code</button>
    </div>
  {/if}
  {#if owned.length}
    <h3 class="list-head">Record new sessions</h3>
    <div class="slist" data-find="recording">
      {#each owned as h (h.id)}
        <SettingRow label={h.name}>
          {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label={`Record new sessions on ${h.name}`} checked={recording[h.id] || false} aria-disabled={busy} onchange={e => { const enabled = e.currentTarget.checked; e.currentTarget.checked = recording[h.id] || false; void saveRecording(h, enabled) }} />{/snippet}
        </SettingRow>
      {/each}
    </div>
    <p class="list-foot">Recording is off by default. It saves terminal output for replay in the ended card, in this server's uploads folder. The terminal toggle remembers your choice per machine. Turning it off discards the current session's recording; existing recordings stay available.</p>
  {/if}
{:else}
  <h2 class="display">Access</h2>
  <p class="muted">Who can see and use terminals on your machines. Access is asked for and granted per machine.</p>
  {@const standing = grants.filter(g => g.expires_at == null)}
  <h3 class="list-head">Standing grants</h3>
  {#if standing.length}
    <div class="slist" data-find="grants">
      {#each standing as g (g.id)}
        <SettingRow label={`${user(g.grantee_id)} · ${machine(g.host_id)}`} hint={g.capability === 'terminal_control' ? 'Terminal control' : 'Terminal view'}>
          {#snippet control()}<InlineConfirm action="Revoke" sentence={`Revoke ${user(g.grantee_id)}’s access to ${machine(g.host_id)}? They must request access again.`} confirm={() => revoke(g)} />{/snippet}
        </SettingRow>
      {/each}
    </div>
  {:else}<p class="list-foot" data-find="grants">Nobody has standing access. People ask each time, and you approve it here.</p>{/if}
  <h3 class="list-head">Recent access</h3>
  {#if log.length}
    <div class="slist" data-find="log">
      {#each log as entry}<SettingRow label={`${user(entry.actor_id)} · ${entry.action.replaceAll('_', ' ')} · ${machine(entry.host_id)}`} hint={new Date(entry.created_at * 1000).toLocaleString()} />{/each}
    </div>
  {:else}<p class="list-foot" data-find="log">Requests, decisions, and terminal activity appear here.</p>{/if}
{/if}
{#if error}<p role="alert" class="error">{error}</p>{/if}
<style>
  .machine { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 12px; min-height: 56px; padding: 8px 12px 8px 16px; }
  .machine > :global(.confirmation) { margin-inline-start: auto; }
  .text { flex: 1 1 8em; min-width: 0; display: grid; line-height: 1.3; }
  .nm { overflow-wrap: anywhere; }
  .sub { font-size: var(--fs-xs); color: var(--ink-2); }
  .dot { width: 8px; height: 8px; flex: none; background: var(--ink-3); border-radius: 50%; } .online { background: var(--success); }
  .pad { padding: 12px 16px; }
  .pad p { margin: 0; font-size: var(--fs-sm); }
  pre { white-space: pre-wrap; overflow-wrap: anywhere; font: var(--fs-xs)/1.7 var(--mono); margin: 10px 0 0; user-select: all; }
  .error { color: var(--danger); }
</style>
