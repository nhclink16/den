<script lang="ts">
  import { onMount } from 'svelte'
  import { native, invoke } from '../lib/native'
  import { desktop } from '../lib/desktop.svelte'
  import { activityShare } from '../lib/activity-share.svelte'
  import { sharePicker } from '../lib/share-picker.svelte'
  import { call } from '../lib/call.svelte'
  import { instances, store } from '../lib/store.svelte'
  import AddServer from './AddServer.svelte'
  import ActivityAsk from './ActivityAsk.svelte'
  import SharePicker from './SharePicker.svelte'
  onMount(() => { desktop.attach(); activityShare.attach(); sharePicker.attach() })
  $effect(() => desktop.syncShortcut(call.room && call.prefs.mode === 'ptt' ? call.prefs.pttKey : ''))
  $effect(() => { if (native) void invoke('badge', { count: instances.totalUnread }).catch(() => {}) })
  $effect(() => { if (native) void invoke('tray_state', { inCall: !!call.room, muted: !call.micOn, deafened: call.outputMuted }).catch(() => {}) })
</script>
{#if native && instances.adding}<AddServer />{/if}
{#if native && store.me}<ActivityAsk />{/if}
{#if native}<SharePicker />{/if}
