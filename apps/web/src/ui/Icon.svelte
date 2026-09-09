<script lang="ts">
  // A handful of 16px line icons. Stroke inherits color.
  let { name, size = 16 }: { name: 'hash' | 'lock' | 'inbox' | 'gear' | 'search' | 'panel' | 'people' | 'plus' | 'reply' | 'trash' | 'edit' | 'x' | 'file' | 'clip' | 'bot' | 'send' | 'menu' | 'check' | 'copy'; size?: number } = $props()
  const paths: Record<string, string> = {
    hash: 'M4 6h9M3 10h9M7 3l-1 10M11 3l-1 10',
    lock: 'M4 7V5a4 4 0 0 1 8 0v2M3 7h10v7H3z',
    inbox: 'M2 9l2-6h8l2 6v4H2zM2 9h4l1 2h2l1-2h4',
    gear: 'M2 4.5h6M11 4.5h3M2 11.5h3M8 11.5h6M9.5 4.5a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0zM6.5 11.5a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0z',
    search: 'M7 11.5a4.5 4.5 0 1 0 0-9 4.5 4.5 0 0 0 0 9zM10.5 10.5L14 14',
    panel: 'M2 3h12v10H2zM6 3v10',
    people: 'M6 8a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5zM1.5 14a4.5 4.5 0 0 1 9 0M11 8a2 2 0 1 0 0-4M12 10.5a3.5 3.5 0 0 1 2.5 3.5',
    plus: 'M8 3v10M3 8h10',
    reply: 'M6 4L2 8l4 4M2 8h7a5 5 0 0 1 5 5',
    trash: 'M3 4h10M6 4V2h4v2M5 4l.5 9h5l.5-9',
    edit: 'M11 2l3 3-8 8H3v-3z',
    x: 'M4 4l8 8M12 4l-8 8',
    file: 'M4 1h5l4 4v10H4zM9 1v4h4',
    clip: 'M13 7.5L7.5 13a3.5 3.5 0 0 1-5-5L8 2.5a2.5 2.5 0 0 1 3.5 3.5L6 11.5a1.2 1.2 0 0 1-1.7-1.7L9.5 4.5',
    bot: 'M3 6h10v7H3zM8 3v3M5 9.5h.01M11 9.5h.01M6 12h4',
    send: 'M2 8l12-6-3 12-3-5z',
    menu: 'M2 4h12M2 8h12M2 12h12',
    check: 'M3 8.5l3 3 7-7',
    copy: 'M6 6h8v8H6zM2 10V2h8',
  }
</script>

<svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name]} />
</svg>
