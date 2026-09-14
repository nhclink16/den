<script lang="ts">
  import { onMount } from 'svelte'
  let downloaded = $state(false), removing = $state(false), error = $state('')
  let punctuation = $state(localStorage.getItem('den.dictation.punctuation') !== 'false')
  onMount(() => { void import('../lib/dictation-cache').then(async cache => { downloaded = await cache.hasModel() }).catch(() => {}) })
  async function remove() {
    removing = true; error = ''
    try { await (await import('../lib/dictation')).remove(); downloaded = false }
    catch { error = 'The voice model could not be removed. Try again.' }
    finally { removing = false }
  }
</script>
<fieldset>
  <legend class="eyebrow">Dictation</legend>
  <label class="device">Language<select class="field" aria-label="Dictation language"><option value="en">English</option></select></label>
  <p class="muted small">The on-device voice model supports English.</p>
  <label class="switch"><input type="checkbox" bind:checked={punctuation} onchange={() => localStorage.setItem('den.dictation.punctuation', String(punctuation))} /> Punctuation</label>
  <div class="model"><span class="muted small">Voice model: 40 MB, {downloaded ? 'downloaded' : 'not downloaded'}</span>{#if downloaded}<button class="btn quiet" disabled={removing} onclick={remove}>Remove</button>{/if}</div>
  {#if error}<p role="alert">{error}</p>{/if}
</fieldset>
<style>
  fieldset { border: 0; padding: 0; margin: 24px 0 16px; }
  .device { display: grid; gap: 6px; margin: 12px 0 10px; }
  .switch, .model { display: flex; align-items: center; gap: 10px; padding: 8px 0; }
  .switch input { accent-color: var(--lamp); width: 16px; height: 16px; }
  .small { font-size: 13px; }
</style>
