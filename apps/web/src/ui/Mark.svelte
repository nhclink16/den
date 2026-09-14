<script lang="ts">
  // The Den mark, Astra's Doorway d. currentColor for the letter, amber for the door.
  let { size = 24 }: { size?: number } = $props()
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
  <path fill="currentColor" fill-rule="evenodd" d="M44 4h12v52H30C16 56 8 48 8 36s9-22 22-22c6 0 10 2 14 5V4ZM22 56V36a10 10 0 0 1 20 0v20H22Z"/>
  <path fill="currentColor" d="M26 56V36a6 6 0 0 1 12 0v20H26Z"/>
</svg>
