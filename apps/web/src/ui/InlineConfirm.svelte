<script lang="ts">
  import { onMount, tick } from 'svelte'
  let { action, sentence, confirm, disabled = false }: { action: string; sentence: string; confirm: () => Promise<void>; disabled?: boolean } = $props()
  let armed = $state(false), pending = $state(false)
  let group: HTMLDivElement, trigger: HTMLButtonElement, keep: HTMLButtonElement
  let timer: ReturnType<typeof setTimeout> | undefined
  function cancel() { clearTimeout(timer); armed = false }
  async function arm() {
    armed = true; clearTimeout(timer); timer = setTimeout(cancel, 8000)
    await tick(); keep?.focus({ preventScroll: true })
  }
  async function accept() {
    if (pending || disabled) return
    if (!armed) { await arm(); return }
    clearTimeout(timer); pending = true
    try { await confirm() } finally { pending = false; cancel() }
  }
  function blur(event: FocusEvent) { if (!group.contains(event.relatedTarget as Node | null)) cancel() }
  onMount(() => {
    const outside = (event: PointerEvent) => { if (armed && !group.contains(event.target as Node)) cancel() }
    document.addEventListener('pointerdown', outside, true)
    return () => { clearTimeout(timer); document.removeEventListener('pointerdown', outside, true) }
  })
</script>
<div bind:this={group} class="confirmation" class:armed role="group" aria-label={armed ? sentence : action} onfocusout={blur}>
  {#if armed}<span class="sentence">{sentence}</span>{/if}
  <div class="actions">
    <button bind:this={trigger} class="btn" class:danger={armed} class:quiet={!armed} disabled={disabled || pending} onclick={accept}>{action}</button>
    {#if armed}<button bind:this={keep} class="btn quiet" disabled={pending} onclick={() => { cancel(); trigger.focus() }}>Keep</button>{/if}
  </div>
</div>
<style>
  .confirmation { flex: none; }
  .confirmation.armed { flex: 0 1 360px; display: grid; gap: 6px; }
  .sentence { font-size: 13px; line-height: 1.4; }
  .actions { display: flex; gap: 6px; justify-content: flex-end; }
</style>
