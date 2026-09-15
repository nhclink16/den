<script lang="ts">
  import { call } from '../lib/call.svelte'
  import Icon from './Icon.svelte'
  let { userId, name }: { userId: string; name: string } = $props()
  const level = $derived(call.level(userId))
</script>

<div class="volume-control">
  <label><span>Volume <output>{Math.round(call.volume(userId) * 100)}%</output></span>
    <input type="range" min="0" max="100" step="1" value={Math.round(call.volume(userId) * 100)} aria-label={`${name} volume`} oninput={(e) => call.setVolume(userId, Number(e.currentTarget.value) / 100)} />
  </label>
  <button aria-pressed={level.muted} onclick={() => call.muteForMe(userId)}><Icon name={call.volume(userId) === 0 ? 'sound-off' : 'sound'} size={14} />Mute for me</button>
</div>
<style>
  .volume-control { width: 176px; padding: 8px; color: var(--ink); font-size: 12px; }
  label, label > span { display: block; }
  label > span { display: flex; justify-content: space-between; }
  output { font: 11px var(--mono); color: var(--ink-2); }
  input { width: 100%; height: 24px; margin: 4px 0; accent-color: var(--lamp); }
  button { display: flex; align-items: center; gap: 8px; width: 100%; min-height: 28px; padding: 4px; border-radius: var(--r); text-align: left; }
  button:hover, button[aria-pressed='true'] { background: var(--bg-3); }
</style>
