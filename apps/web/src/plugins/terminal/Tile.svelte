<script lang="ts">
  import type { ObjectProps } from '..'
  import View from './View.svelte'
  let { object }: ObjectProps = $props()
</script>
<div class="tile screen terminal-tile" data-testid="terminal-call-tile"><View {object} compact /></div>
<style>
  .terminal-tile { isolation: isolate; position: relative; flex: none; width: 384px; aspect-ratio: 16/9; border: 1px solid var(--line); border-radius: var(--r-lg); overflow: hidden; }
</style>
