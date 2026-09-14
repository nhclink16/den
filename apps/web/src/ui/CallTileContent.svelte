<script lang="ts">
  import CallTile from './CallTile.svelte'
  import { objectKind } from '../plugins'
  import type { CallParticipant } from '../lib/call.svelte'
  import type { Share } from '../lib/call-shares'
  import type { ObjectSummary } from '../lib/types'
  let { participant, share, object }: { participant?: CallParticipant; share?: Share; object?: ObjectSummary } = $props()
</script>
{#if object}
  {@const Tile = objectKind(object.kind)?.tile}
  {#if Tile}<Tile {object} />{/if}
{:else if participant}<CallTile {participant} {share} screen={!!share} />{/if}
