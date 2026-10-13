<script lang="ts">
  import { call } from '../lib/call.svelte'
  import CallJam from './CallJam.svelte'
  import CallPill from './CallPill.svelte'
  // `controls` off drops the call bar: the open call has its own row of buttons,
  // and two identical rows of call buttons is one too many.
  let { controls = true }: { controls?: boolean } = $props()
</script>

{#if call.channel}
  <CallJam />
  {#if controls}<CallPill full />{/if}
{/if}
