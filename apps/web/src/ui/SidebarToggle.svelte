<script lang="ts">
  import { store } from '../lib/store.svelte'
  import Icon from './Icon.svelte'
  let { hide = false }: { hide?: boolean } = $props()
  const mac = /Mac|iPhone|iPad/.test(navigator.platform)
  const title = $derived(`${hide ? 'Hide' : 'Show'} room list · ${mac ? '⌘' : 'Ctrl+'}\\`)
</script>
<button class="sidebar-toggle" title={title} aria-label={hide ? 'Hide room list' : 'Show room list'} onclick={() => store.saveLayout({ sidebar: !hide })}><Icon name="panel" /></button>
<style>
  .sidebar-toggle { display:inline-grid; place-items:center; width:28px; height:28px; flex:none; border-radius:var(--r); color:var(--ink3); }
  .sidebar-toggle:hover { background:var(--bg3); color:var(--ink); }
</style>
