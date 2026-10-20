<script lang="ts">
  import { instances, store } from '../lib/store.svelte'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import Mark from './Mark.svelte'
  // In the top layer, so the main pane never draws over it. Escape and an outside click close it there.
  let open = $state(false)
  let trigger: HTMLElement, list: HTMLElement
  function close() { open = false; trigger?.focus() }
  // Focus that was in the list (or fell to the page) goes back to the name, as it does after a choice.
  function closed(e: ToggleEvent) {
    if (e.newState === 'closed' && (list.contains(document.activeElement) || document.activeElement === document.body)) trigger.focus()
  }
</script>
<details class="switcher" bind:open>
  <!-- The mark and the name are the button. A dot on the mark: unread mentions or DMs on another server. -->
  <summary bind:this={trigger} class="wordmark display" aria-label={instances.attention ? 'Switch server, unread on another server' : 'Switch server'}>
    <span class="mark" class:attention={instances.attention}><Mark size={22} /></span><span class="name">{store.settings.instance_name}</span>
  </summary>
  <div bind:this={list} class="popover" popover="auto" use:disclosurePopover={'start'} ontoggle={closed} role="dialog" aria-label="Servers">
    {#if open}
      {#each instances.all as s (s.origin)}
        <div class="server" class:active={s === instances.active}>
          <button class="choose" onclick={() => { instances.select(s); close() }}>
            <Mark size={22} /><span class="identity"><strong>{s.settings.instance_name}</strong>{#if instances.all.filter(x => x.settings.instance_name === s.settings.instance_name).length > 1}<small>{s.origin}</small>{/if}</span>
            {#if s.attention}<span class="at">@</span>{/if}{#if s.totalUnread}<span>{s.totalUnread}</span>{/if}
          </button>
          <button class="remove" aria-label={`Remove ${s.settings.instance_name}`} onclick={() => { void instances.remove(s); close() }}>Remove</button>
        </div>
      {/each}
      <button class="add" onclick={() => { close(); instances.add() }}>+ Add a server</button>
    {/if}
  </div>
</details>
<style>
  .switcher { min-width: 0; flex: 0 1 auto; }
  /* Padding with a matching negative margin: the hover and open fill reach past the mark and name without moving them. */
  .wordmark { display:flex; align-items:center; gap:8px; margin:-4px -8px; padding:4px 8px; border-radius:var(--r-pill); font-size:22px; text-align:left; list-style:none; cursor:pointer; transition:background-color var(--t-fast); }
  .wordmark::-webkit-details-marker { display:none; }
  @media (hover: hover) { .wordmark:hover { background:var(--bg-3); } }
  .switcher[open] > .wordmark { background:var(--bg-3); }
  .wordmark:active { background:var(--press); }
  .mark { position:relative; display:flex; flex:none; }
  .attention::after { content:''; position:absolute; width:6px; height:6px; background:var(--lamp); border-radius:50%; right:-3px; top:-2px; }
  /* A gap round the dot cut out of the mark, so it reads on any background (hover, open, wallpaper). */
  .attention > :global(svg) { mask:radial-gradient(circle at 100% 1px, transparent 5px, #000 5.5px); }
  .name { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; flex:1; }
  .popover { position:fixed; inset:auto; margin:0; width:320px; max-width:calc(100vw - 16px); padding:6px; border:1px solid var(--line); background:var(--bg2); color:var(--ink); border-radius:var(--r-lg); box-shadow:0 12px 40px var(--scrim); }
  .server { display:flex; align-items:center; border-radius:var(--r); }
  .server.active { background:var(--bg3); }
  @media (hover: hover) { .server:hover { background:var(--bg3); } }
  .choose { display:flex; flex:1; min-width:0; align-items:center; gap:10px; text-align:left; padding:12px 8px; }
  .identity { display:grid; min-width:0; flex:1; gap:3px; }
  small { font-size: var(--fs-xs); overflow:hidden; text-overflow:ellipsis; color:var(--ink3); }
  .at { color:var(--lamp); }
  .remove { opacity:0; font-size: var(--fs-xs); padding:10px 6px; color:var(--danger); }
  .server:focus-within .remove { opacity:1; }
  @media (hover: hover) { .server:hover .remove { opacity:1; } }
  .add { padding:12px; width:100%; text-align:left; border-top:1px solid var(--line); margin-top:6px; color:var(--accent); }
</style>
