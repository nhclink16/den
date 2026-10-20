<script lang="ts">
  // Den Shell only: Den's own question about notifications, before iOS's one-time prompt. It waits a moment after
  // home appears, so the first thing someone sees is Den, not a question.
  import { push, pushAnswer } from '../lib/shell-push.svelte'
  import { modal, outsideDialog } from '../lib/modal'
  import Icon from './Icon.svelte'

  let shown = $state(false)
  $effect(() => {
    if (!push.asking) { shown = false; return }
    const wait = setTimeout(() => (shown = true), 1500)
    return () => clearTimeout(wait)
  })
</script>

{#if push.asking && shown}
  <dialog class="ask" use:modal tabindex="-1" aria-labelledby="push-ask-title"
    oncancel={(e) => { e.preventDefault(); pushAnswer(false) }} onclick={(e) => { if (outsideDialog(e)) pushAnswer(false) }}>
    <span class="bell" aria-hidden="true"><Icon name="bell" size={28} /></span>
    <h2 id="push-ask-title">Turn on notifications?</h2>
    <p>Den tells you when someone mentions you or sends you a message. Follow a room to hear about everything in it.</p>
    <button class="btn lit" onclick={() => pushAnswer(true)}>Turn On</button>
    <button class="btn quiet" onclick={() => pushAnswer(false)}>Not Now</button>
  </dialog>
{/if}

<style>
  /* A bottom sheet: the page dims behind it, and its background runs under the home indicator. */
  .ask {
    position: fixed; inset: auto 0 0; margin: 0 auto; width: min(100%, 480px); max-width: none; max-height: none;
    display: flex; flex-direction: column; align-items: center; text-align: center;
    padding: 28px 24px max(16px, env(safe-area-inset-bottom)); border: 0; border-radius: max(10px, var(--r-lg)) max(10px, var(--r-lg)) 0 0;
    background: var(--bg-2); color: var(--ink);
  }
  .ask::backdrop { background: rgb(0 0 0 / .35); }
  .bell { display: grid; place-items: center; width: 60px; height: 60px; border-radius: 50%; color: var(--lamp); background: var(--lamp-glow); }
  h2 { margin: 16px 0 0; font-size: 22px; line-height: 1.2; }
  p { margin: 8px 0 24px; color: var(--ink-2); font-size: 15px; line-height: 1.4; max-width: 34ch; }
  .btn { width: 100%; justify-content: center; }
  .btn.lit { min-height: 50px; font-size: 17px; }
  .btn.quiet { margin-top: 6px; }
  @media (prefers-reduced-motion: no-preference) {
    .ask { animation: up .35s var(--ease-out); }
    .ask::backdrop { animation: dim .35s; }
  }
  @keyframes up { from { transform: translateY(100%); } }
  @keyframes dim { from { background: transparent; } }
</style>
