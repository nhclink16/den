<script lang="ts">
  import { instances, store } from '../lib/store.svelte'
  import Mark from './Mark.svelte'
  let open = $state(false)
  let trigger: HTMLButtonElement
  function close() { open = false; trigger?.focus() }
</script>
<div class="switcher">
  <button bind:this={trigger} class="wordmark display" aria-label="Switch server" aria-expanded={open} onclick={() => open = !open}>
    <Mark size={22} /><span>{store.settings.instance_name}</span><span class="chevron" class:attention={instances.attention} aria-label={instances.attention ? 'Unread mentions or DMs on another server' : undefined}>⌄</span>
  </button>
  {#if open}
    <button class="outside" aria-label="Close server switcher" onclick={close}></button>
    <div class="popover" role="dialog" aria-label="Servers" onkeydown={e => { if (e.key === 'Escape') close() }} tabindex="-1">
      {#each instances.all as s (s.origin)}
        <div class="server" class:active={s === instances.active}>
          <button class="choose" onclick={() => { instances.select(s); close() }}>
            <Mark size={22} /><span class="identity"><strong>{s.settings.instance_name}</strong>{#if instances.all.filter(x => x.settings.instance_name === s.settings.instance_name).length > 1}<small class="mono">{s.origin}</small>{/if}</span>
            {#if s.attention}<span class="at mono">@</span>{/if}{#if s.totalUnread}<span class="mono">{s.totalUnread}</span>{/if}
          </button>
          <button class="remove" aria-label={`Remove ${s.settings.instance_name}`} onclick={() => { void instances.remove(s); close() }}>Remove</button>
        </div>
      {/each}
      <button class="add" onclick={() => { close(); instances.add() }}>+ Add a server</button>
    </div>
  {/if}
</div>
<style>
  .switcher { position: relative; min-width: 0; flex: 1; }
  .wordmark { display:flex; align-items:center; gap:8px; width:100%; font-size:22px; text-align:left; }
  .wordmark > span:first-of-type { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; flex:1; }
  .chevron { position:relative; font-size:18px; }
  .attention::after { content:''; position:absolute; width:6px; height:6px; background:var(--lamp); border-radius:50%; right:-3px; top:1px; }
  .outside { position:fixed; inset:0; z-index:40; cursor:default; }
  .popover { position:absolute; z-index:41; top:calc(100% + 12px); left:-8px; width:320px; max-width:calc(100vw - 32px); padding:6px; border:1px solid var(--line); background:var(--bg2); border-radius:var(--r-lg); box-shadow:0 12px 40px var(--scrim); }
  .server { display:flex; align-items:center; border-radius:var(--r); }
  .server:hover, .server.active { background:var(--bg3); }
  .choose { display:flex; flex:1; min-width:0; align-items:center; gap:10px; text-align:left; padding:12px 8px; }
  .identity { display:grid; min-width:0; flex:1; gap:3px; }
  small { font-size:10px; overflow:hidden; text-overflow:ellipsis; color:var(--ink3); }
  .at { color:var(--lamp); }
  .remove { opacity:0; font-size:11px; padding:10px 6px; color:var(--danger); }
  .server:hover .remove, .server:focus-within .remove { opacity:1; }
  .add { padding:12px; width:100%; text-align:left; border-top:1px solid var(--line); margin-top:6px; color:var(--accent); }
</style>
