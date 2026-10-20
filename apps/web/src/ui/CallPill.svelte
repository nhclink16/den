<script lang="ts">
  // The call, small, while you chat: who's in, who's talking, your mic and Leave.
  // Tap it to open the call. `full` is the call bar above the sidebar: the same
  // pill with Leave beside it and every other call button on a row under it.
  import { call } from '../lib/call.svelte'
  import { native } from '../lib/native'
  import { callFaces, callLine } from '../lib/room-list'
  import CallControls from './CallControls.svelte'
  import CallFaces from './CallFaces.svelte'
  import Icon from './Icon.svelte'
  let { full = false }: { full?: boolean } = $props()
  const faces = $derived(callFaces(call.participants))
  const line = $derived(callLine(call.participants, call.reconnecting))
</script>

<div class="call-pill" class:full role="group" aria-label="Current call" data-testid={full ? 'call-dock' : 'call-pill'}>
  <button class="open" title={call.expanded ? 'Back to the chat' : 'Open the call'} onclick={() => (call.expanded = !call.expanded)}>
    <CallFaces {faces} size={full ? 26 : 28} instance={call.owner ?? undefined} />
    <span class="tx">
      <b>{call.title}{#if native}<small> · {call.instanceName}</small>{/if}</b>
      <span class="line" class:talking={!!line.talking && !call.reconnecting}>
        {#if line.talking && !call.reconnecting}<span class="talk-bars" aria-hidden="true"><i></i><i></i><i></i></span>{/if}
        <span class="say">{line.talking || line.people}</span>
      </span>
    </span>
    <span class="sr-only">{call.expanded ? ', back to the chat' : ', open the call'}</span>
  </button>
  {#if full}<button class="leave" aria-label="Leave call" title="Leave call" onclick={() => call.leave()}><Icon name="leave" size={18} /></button>{/if}
  <CallControls variant={full ? 'dock' : 'pill'} />
</div>

<style>
  /* One family with the hangout card: the same faces, title and line. Floating
     glass on a phone; on the sidebar it sits flat, like the card above it. */
  .call-pill {
    --ring: var(--bg-2);
    flex: none; display: flex; align-items: center; gap: 8px; margin: 8px 12px; padding: 6px 6px 6px 10px;
    border-radius: var(--r-pill); background: var(--glass); backdrop-filter: blur(16px) saturate(1.2);
    box-shadow: var(--lift), 0 0 0 1px color-mix(in srgb, var(--line) 70%, transparent);
    container: pill / inline-size;
  }
  /* A very small phone with sound off has room for the words or the faces, not both. */
  @container pill (max-width: 330px) { .open :global(.faces) { display: none; } }
  .open { flex: 1 1 160px; min-width: 0; min-height: 44px; display: flex; align-items: center; gap: 10px; text-align: left; border-radius: var(--r-pill); }
  .tx { min-width: 0; display: flex; flex-direction: column; line-height: 1.25; }
  b { font-size: var(--fs-body); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  b small { font-weight: 400; color: var(--ink-2); }
  .line { display: flex; align-items: center; gap: 6px; min-width: 0; font-size: var(--fs-sm); color: var(--ink-2); }
  .line.talking { color: var(--lamp); }
  .say { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .full {
    --ring: var(--bg-3);
    container: dock / inline-size; flex-wrap: wrap; row-gap: 8px; margin: 6px 8px max(6px, env(safe-area-inset-bottom, 0px)); padding: 10px;
    border-radius: calc(var(--r-lg) * 1.4); background: var(--bg-3); backdrop-filter: none; box-shadow: none;
  }
  .full .open { flex: 1 1 0; min-height: 36px; padding-inline-start: 2px; }
  .full b { font-size: var(--fs-sm); }
  .full .line { font-size: var(--fs-xs); }
  /* Red and apart from the toggles, like the open call's. */
  .leave { flex: none; display: grid; place-items: center; width: 34px; height: 34px; border-radius: var(--r-pill); color: var(--bg); background: var(--danger); transition: filter var(--t-fast); }
  @media (hover: hover) { .leave:hover { filter: brightness(1.1); } }
  @media (pointer: coarse) { .leave { width: 44px; height: 44px; } }
  /* Too narrow for one row (the sidebar): who's in and Leave on top, the toggles
     spread across a row of their own, like Discord's voice panel. */
  @container dock (max-width: 520px) { .full :global(.controls) { flex-basis: 100%; justify-content: space-between; gap: 4px; } }
  /* Wide (the bottom of the window with the sidebar hidden): one row, Leave last. */
  @container dock (min-width: 521px) { .full .open { flex-basis: 160px; } .leave { order: 1; margin-inline-start: 4px; } }
</style>
