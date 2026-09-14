<script lang="ts">
  import { onMount } from 'svelte'
  import { native, invoke } from '../lib/native'
  import { desktop } from '../lib/desktop.svelte'
  import { call } from '../lib/call.svelte'
  import { instances } from '../lib/store.svelte'
  import AddServer from './AddServer.svelte'
  onMount(() => desktop.attach())
  $effect(() => desktop.syncShortcut(call.room && call.prefs.mode === 'ptt' ? call.prefs.pttKey : ''))
  $effect(() => { if (native) void invoke('badge', { count: instances.totalUnread }).catch(() => {}) })
  $effect(() => { if (native) void invoke('tray_state', { inCall: !!call.room, muted: !call.micOn, deafened: call.outputMuted }).catch(() => {}) })
</script>
{#if native && instances.adding}<AddServer />{/if}
