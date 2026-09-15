<script lang="ts">
  import type { ObjectProps } from '..'
  import { openObject } from '../../lib/objects.svelte'
  import { load, terminals } from './state.svelte'
  import Avatar from '../../ui/Avatar.svelte'
  let { object }: ObjectProps = $props()
  let id = $state('')
  const session = $derived(terminals.sessions[id])
  $effect(() => {void object.version; load(object).then(t => {if (t) id = t.id}).catch(() => {})})
</script>
<button class="terminal-card" onclick={() => openObject(object)} title="Open terminal" data-object-id={object.id}>
  <span class="preview">{#if terminals.previews[id]}<pre>{terminals.previews[id]}</pre>{:else}<span class="glyph">›_</span>{/if}</span>
  <span class="caption"><span class="title"><span class="eyebrow">terminal</span><span class="name">{object.name}</span>{#if session?.ended_at}<span class="ended">ended · {Math.max(1, Math.round((session.ended_at - session.started_at) / 60))} min{#if session.recording_upload_id} ▷{/if}</span>{/if}</span>
    <span class="people">{#if session?.active_controller_id}<span class="controller"><Avatar userId={session.active_controller_id} size={24} /><i></i></span>{/if}{#each session?.viewer_ids.filter(id => id !== session.active_controller_id) || [] as userId}<Avatar {userId} size={20} />{/each}</span>
  </span>
</button>
<style>
  .terminal-card { display: block; width: 320px; max-width: 100%; background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r-lg); overflow: hidden; text-align: left; margin: 6px 0; }
  .terminal-card:hover { border-color: var(--ink-3); } .preview { display: grid; place-items: center; height: 150px; overflow: hidden; background: var(--bg); }
  pre { height: 100%; margin: 0; align-self: start; font: 9px/1.3 'Den Terminal Mono', monospace; color: var(--ink-2); opacity: .55; white-space: pre; width: 100%; padding: 10px; overflow: hidden; }
  .glyph { font: 30px var(--mono); color: var(--ink-3); }
  .caption { display: flex; align-items: center; padding: 10px 12px; gap: 10px; } .title { flex: 1; min-width: 0; display: grid; } .name { font: 15px var(--display); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .people { display: flex; align-items: center; gap: 3px; } .controller { position: relative; } .controller i { position: absolute; width: 7px; height: 7px; right: -2px; bottom: -2px; border-radius: 50%; background: var(--lamp); }
  .people :global(.dot) { display: none; } .ended { font: 10px var(--mono); color: var(--ink-3); margin-top: 4px; }
</style>
