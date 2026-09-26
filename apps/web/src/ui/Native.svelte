<script lang="ts">
  import { onMount } from 'svelte'
  import { native, invoke } from '../lib/native'
  import { desktop } from '../lib/desktop.svelte'
  import { themes } from '../lib/theme.svelte'
  import { activityShare } from '../lib/activity-share.svelte'
  import { call } from '../lib/call.svelte'
  import { instances, store } from '../lib/store.svelte'
  import AddServer from './AddServer.svelte'
  import ActivityAsk from './ActivityAsk.svelte'
  onMount(() => {
    desktop.attach(); activityShare.attach()
    if (native) document.documentElement.classList.add('desktop')
  })
  $effect(() => desktop.syncShortcut(call.room && call.prefs.mode === 'ptt' ? call.prefs.pttKey : ''))
  $effect(() => { if (native) void invoke('badge', { count: instances.totalUnread }).catch(() => {}) })
  $effect(() => { if (native) void invoke('tray_state', { inCall: !!call.room, muted: !call.micOn, deafened: call.outputMuted }).catch(() => {}) })
  $effect(() => {
    if (native) {
      const colors = themes.active[themes.half]
      void invoke('set_titlebar', { color: colors.bg, symbolColor: colors.ink }).catch(() => {})
    }
  })
</script>
{#if native && instances.adding}<AddServer />{/if}
{#if native && store.me}<ActivityAsk />{/if}
