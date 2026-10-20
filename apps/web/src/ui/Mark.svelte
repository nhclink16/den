<script lang="ts">
  // The Den mark, Astra's Doorway d. The letter takes the surrounding text colour,
  // the doorway takes the active theme's accent, so the mark changes with the theme.
  // The letter is one outline that runs round the doorway. Cutting the doorway out with
  // even-odd left a hairline where the bowl curves up off the baseline (x 22–28).
  let { size = 24, accent = 'var(--accent)' }: { size?: number; accent?: string } = $props()
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
  <path fill="currentColor" d="M44 4h12v52H42V36a10 10 0 0 0-20 0v18.98C13 52.47 8 45.48 8 36c0-12 9-22 22-22c6 0 10 2 14 5V4Z" />
  <path fill={accent} d="M26 56V36a6 6 0 0 1 12 0v20H26Z" />
</svg>
