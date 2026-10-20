<script lang="ts">
  import type { Snippet } from 'svelte'
  /**
   * A small form over the page, the way iOS asks for a name: a title, a field or two, Cancel and one
   * action. Settings uses it to name a sound pack, make an API key or an agent, and add a room or category.
   * `onsubmit` may throw: the message shows in the dialog and it stays open.
   */
  let { open = $bindable(false), title, action, disabled = false, danger = false, onsubmit, children }: {
    open?: boolean; title: string; action: string; disabled?: boolean
    /** A destructive action: its button is red, and the dialog only asks (no fields). */
    danger?: boolean
    onsubmit: () => Promise<void> | void; children?: Snippet
  } = $props()
  let dialog = $state<HTMLDialogElement>()
  let busy = $state(false), error = $state('')
  $effect(() => {
    if (!dialog) return
    if (open && !dialog.open) { error = ''; dialog.showModal() }
    else if (!open && dialog.open) dialog.close()
  })
  async function submit(e: SubmitEvent) {
    e.preventDefault()
    if (busy) return
    busy = true; error = ''
    try { await onsubmit(); open = false }
    catch (err) { error = (err as Error).message || 'That did not work.' }
    finally { busy = false }
  }
</script>

<dialog class="form-dialog" bind:this={dialog} aria-label={title} onclose={() => (open = false)}>
  <form onsubmit={submit}>
    <h2 class="title">{title}</h2>
    {#if children}<div class="fields">{@render children()}</div>{/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="buttons">
      <button type="button" onclick={() => (open = false)}>Cancel</button>
      <button type="submit" class="go" class:danger disabled={disabled || busy}>{action}</button>
    </div>
  </form>
</dialog>

<style>
  .form-dialog { width: min(300px, calc(100vw - 48px)); padding: 0; overflow: hidden; border: 0; border-radius: max(14px, var(--r-lg)); background: var(--bg-2); color: var(--ink); box-shadow: 0 12px 40px rgb(0 0 0 / .25); }
  .form-dialog::backdrop { background: rgb(0 0 0 / .35); }
  /* Outranks Settings' page titles, which style every h2 inside it. */
  .form-dialog h2.title { margin: 0; padding: 20px 16px 4px; font: 600 17px/1.3 var(--body); letter-spacing: 0; text-align: center; color: var(--ink); }
  .fields { display: grid; gap: 8px; padding: 12px 16px 16px; }
  .fields :global(.field) { width: 100%; }
  .error { margin: -8px 16px 12px; font-size: var(--fs-xs); color: var(--danger); text-align: center; }
  /* Two buttons side by side under a hairline, as iOS draws an alert's. */
  .buttons { display: grid; grid-template-columns: 1fr 1fr; border-top: 1px solid var(--hair); }
  .buttons button { min-height: 44px; color: var(--lamp); font-size: 17px; }
  .buttons button + button { border-inline-start: 1px solid var(--hair); }
  .buttons .go { font-weight: 600; }
  .buttons .danger { color: var(--danger); }
  .fields :global(p) { margin: 0; font-size: var(--fs-sm); line-height: 1.4; text-align: center; color: var(--ink-2); }
  .buttons button:disabled { color: var(--ink-3); cursor: default; }
  .buttons button:active:not(:disabled) { background: var(--press); }
</style>
