<script lang="ts">
  import { onMount } from 'svelte'
  import SettingRow from './SettingRow.svelte'
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
<h3 class="list-head">Dictation</h3>
<div class="slist" data-find="dictation">
  <SettingRow label="Punctuation" hint="Adds full stops and commas as you speak.">
    {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="Punctuation" bind:checked={punctuation} onchange={() => localStorage.setItem('den.dictation.punctuation', String(punctuation))} />{/snippet}
  </SettingRow>
  <SettingRow label="Voice model" hint={`40 MB, ${downloaded ? 'downloaded to this device' : 'not downloaded'}. It downloads the first time you dictate.`}>
    {#snippet control()}{#if downloaded}<button class="btn quiet" disabled={removing} onclick={remove}>Remove</button>{/if}{/snippet}
  </SettingRow>
</div>
<p class="list-foot">English, recognised on this device.</p>
{#if error}<p role="alert" class="error">{error}</p>{/if}
<style>
  .error { color: var(--danger); font-size: var(--fs-sm); }
</style>
