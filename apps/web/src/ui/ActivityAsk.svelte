<script lang="ts">
  // Asked once per app: games share on their own, but "Using Firefox" is the
  // owner's call. Sits in the corner until answered; nothing is shared meanwhile.
  import { activityShare } from '../lib/activity-share.svelte'
  const app = $derived(activityShare.asking)
</script>

{#if app}
  <div class="ask" role="dialog" aria-label="Share activity">
    <p>Show people you're using <b>{app.name}</b>?</p>
    <div class="row">
      <button class="btn lit" onclick={() => activityShare.answer('yes')}>Share</button>
      <button class="btn" onclick={() => activityShare.answer('no')}>Not this app</button>
      <button class="btn quiet" onclick={() => activityShare.answer('never')}>Don't ask</button>
    </div>
  </div>
{/if}

<style>
  .ask {
    position: fixed; right: 16px; bottom: 16px; z-index: 50; width: min(320px, calc(100% - 32px));
    display: grid; gap: 10px; padding: 12px 14px;
    background: var(--bg-2); border: 1px solid var(--line-strong); border-radius: var(--r-lg);
    box-shadow: 0 14px 40px -10px var(--shadow-lg); font-size: 14px;
  }
  @media (prefers-reduced-motion: no-preference) { .ask { animation: ask-in .24s var(--ease-out); } }
  @keyframes ask-in { from { opacity: 0; transform: translateY(8px); } }
  p { margin: 0; overflow-wrap: anywhere; }
  .row { display: flex; flex-wrap: wrap; gap: 6px; }
</style>
