<script module lang="ts">
  // The phone list unmounts while a room is open. Coming back should find it
  // where it was, with the room you left in view.
  let savedScroll = 0
  let lastRoom = ''
</script>

<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { call } from '../lib/call.svelte'
  import SidebarToggle from './SidebarToggle.svelte'
  import CallDock from './CallDock.svelte'
  import CallFaces from './CallFaces.svelte'
  import { store, instances } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'
  import RoomIcon from './RoomIcon.svelte'
  import { native } from '../lib/native'
  import { desktop } from '../lib/desktop.svelte'
  import ServerSwitcher from './ServerSwitcher.svelte'
  import { profileCard } from '../lib/people.svelte'
  import { themes } from '../lib/theme.svelte'
  import { dotTone, gameGlyph, gameLabel, stateLabel } from '../lib/servers'
  import { callFaces, callLine, placedIn, previewText } from '../lib/room-list'
  import { liveStatus } from '../lib/status'
  import { dayLabel, sameDay, shortTime } from '../lib/time'
  import type { Channel } from '../lib/types'

  // `phone` is the home screen on phones: the same list, full width, larger.
  let { phone = false }: { phone?: boolean } = $props()
  const inboxCount = $derived(instances.totalUnread)
  const inboxBadge = $derived(inboxCount > 99 ? '99+' : String(inboxCount))
  const iconMode = $derived(themes.appearance.room_icons ?? 'icons')
  const uncategorized = $derived(store.textChannels.filter((c) => c.kind === 'text' && !placedIn(c, store.categories)))
  const voice = $derived(store.textChannels.filter((c) => c.kind === 'voice'))
  const serverActive = (slug: string) => router.route.name === 'server' && router.route.slug === slug
  const active = (id: string) => !phone && router.route.name === 'channel' && router.route.id === id
  // From the phone list, a room records the list as where it came from, so its
  // back button returns here instead of stacking another copy.
  const go = (path: string) => (e: MouseEvent) => { e.preventDefault(); router.go(path, false, phone ? '/' : undefined) }
  const status = $derived(liveStatus(store.me?.status))
  let q = $state('')
  function search(e: SubmitEvent) {
    e.preventDefault()
    router.go(q.trim() ? `/find?q=${encodeURIComponent(q.trim())}` : '/find', false, '/')
  }

  // The sidebar line is the Jam's only announcement, so keep its track current
  // for people who are not in the call and have no strip polling it.
  const jamRooms = $derived(voice.filter((c) => store.jams.has(c.id)).map((c) => c.id))
  $effect(() => {
    const ids = jamRooms
    if (!ids.length) return
    const timer = setInterval(() => {
      if (document.visibilityState === 'visible') for (const id of ids) void store.refreshJam(id, 15000).catch(() => {})
    }, 20000)
    return () => clearInterval(timer)
  })
  const jamLine = (id: string) => {
    const jam = store.jams.get(id)
    return jam ? `Jam · ${jam.now_playing?.track || store.name(jam.host_id)}` : ''
  }

  // --- folding groups (this device only) ---
  const folded = (key: string) => store.layout.collapsed.includes(key)
  const fold = (key: string) => store.saveLayout({ collapsed: folded(key) ? store.layout.collapsed.filter((k) => k !== key) : [...store.layout.collapsed, key] })
  /// A folded group still shows the room you're in, and anything that mentions you.
  const shown = (key: string, list: Channel[]) => folded(key) ? list.filter((c) => active(c.id) || store.unread(c.id).mention) : list
  const groupUnread = (list: Channel[]) => list.reduce((n, c) => n + store.unread(c.id).count, 0)

  // --- one row's words ---
  const when = (iso: string) => (sameDay(iso, new Date().toISOString()) ? shortTime(iso) : dayLabel(iso))
  function line(c: Channel) {
    const typing = store.typingNames(c.id)
    if (typing.length) return { text: typing.length === 1 ? `${typing[0]} is typing…` : 'Several people are typing…', typing: true }
    const last = c.last_message
    if (!last) return { text: c.kind === 'dm' ? 'Say hello' : 'No messages yet', typing: false }
    const pair = c.kind === 'dm' && (c.member_ids || []).length <= 2
    const who = last.author_id === store.me?.id ? 'You: ' : pair ? '' : `${store.name(last.author_id)}: `
    return { text: who + previewText(last), typing: false }
  }
  function who(ids: string[]) {
    const names = ids.filter((id) => id !== store.me?.id).map((id) => store.name(id))
    const me = ids.includes(store.me?.id ?? '')
    if (!names.length) return me ? "You're in" : 'Nobody here yet'
    const list = names.length <= 3 ? names.slice(0, -1).join(', ') + (names.length > 1 ? ' and ' : '') + names.at(-1) : `${names.slice(0, 2).join(', ')} and ${names.length - 2} others`
    return me ? `You, ${list}` : list
  }

  let scroller = $state<HTMLDivElement>()
  onMount(() => {
    if (!phone || !scroller) return
    scroller.scrollTop = savedScroll
    const el = scroller
    void tick().then(() => { if (lastRoom) el.querySelector<HTMLElement>(`[data-room="${lastRoom}"]`)?.focus({ preventScroll: true }) })
    return () => { savedScroll = el.scrollTop }
  })
</script>

{#snippet badge(count: number, mention: boolean)}
  {#if mention}<span class="count at" aria-hidden="true">@</span><span class="sr-only">, mentions you</span>
  {:else if count}<span class="count" aria-hidden="true">{count}</span><span class="sr-only">, {count} unread</span>{/if}
{/snippet}

{#snippet room(c: Channel, last = false)}
  {@const u = store.unread(c.id)}
  {@const l = line(c)}
  <a href="/c/{c.id}" class="row" class:last data-room={c.id} data-hover-animates class:sel={active(c.id)} class:unread={u.count > 0} class:bare={iconMode === 'none' && c.kind !== 'dm'} aria-current={active(c.id) ? 'page' : undefined} onclick={(e) => { lastRoom = c.id; go(`/c/${c.id}`)(e) }}>
    <RoomIcon channel={c} size={phone ? 44 : 34} />
    <span class="mid">
      <span class="top">
        <span class="nm">{store.title(c)}</span>
        {#if c.last_message}<time class="tm" datetime={c.last_message.created_at}>{when(c.last_message.created_at)}</time>{/if}
      </span>
      <span class="pv">
        <span class="pvt" class:typing={l.typing}>{l.text}</span>
        {#if store.uploads.activeInChannel(c.id)}<span class="uploading" title="Uploading files" aria-label="Uploading files">↑</span>{/if}
        {@render badge(u.count, u.mention)}
      </span>
    </span>
  </a>
{/snippet}

{#snippet group(key: string, label: string, list: Channel[])}
  {#if list.length}
    {@const open = !folded(key)}
    {@const rows = shown(key, list)}
    <button class="cat" aria-expanded={open} onclick={() => fold(key)}>
      <span>{label}</span><span class="chev" class:shut={!open}><Icon name="chevron" size={14} /></span>
      {#if !open && groupUnread(list)}<span class="sum">{groupUnread(list)}</span>{/if}
    </button>
    {#each rows as c, i (c.id)}{@render room(c, i === rows.length - 1)}{/each}
  {/if}
{/snippet}

<nav class="side" class:phone class:mac={desktop.platform === 'macos' && !phone} aria-label="Rooms">
  {#if phone}
    <div class="topbar">
      {#if store.me}<a href="/settings" class="me-btn" aria-label="Your profile and settings" onclick={go('/settings')}><Avatar userId={store.me.id} size={34} presence={false} /></a>{/if}
      {#if native}<ServerSwitcher />{/if}
      <span class="conn" class:off={!store.connected} role="img" aria-label={store.connected ? 'Connected' : 'Offline, reconnecting'} title={store.connected ? 'Connected' : 'Reconnecting'}></span>
      <span class="spacer"></span>
      <a href="/inbox" class="icon-btn inbox" aria-label={inboxCount ? `Inbox, ${inboxCount} unread` : 'Inbox'} onclick={go('/inbox')}><Icon name="inbox" size={22} />{#if inboxCount}<span class="count">{inboxBadge}</span>{/if}</a>
    </div>
    <h1 class="display large" title={store.settings.instance_name}>{store.settings.instance_name}</h1>
    <form class="find input-shell" role="search" onsubmit={search}><Icon name="search" size={18} /><input bind:value={q} placeholder="Search" aria-label="Search messages" enterkeyhint="search" /></form>
  {:else}
    <div class="brand">
      {#if native}<ServerSwitcher />{:else}<a href="/" class="display wordmark" onclick={go('/')} title={store.settings.instance_name}>{store.settings.instance_name}</a>{/if}
      <span class="conn" class:off={!store.connected} role="img" aria-label={store.connected ? 'Connected' : 'Offline, reconnecting'} title={store.connected ? 'Connected' : 'Reconnecting'}></span>
      <span class="spacer"></span>
      <a href="/find" class="icon-btn" class:on={router.route.name === 'search'} title="Search" aria-label="Search" onclick={go('/find')}><Icon name="search" /></a>
      <a href="/inbox" class="icon-btn inbox" class:on={router.route.name === 'inbox'} aria-current={router.route.name === 'inbox' ? 'page' : undefined} title="Inbox" aria-label={inboxCount ? `Inbox, ${inboxCount} unread` : 'Inbox'} onclick={go('/inbox')}><Icon name="inbox" />{#if inboxCount}<span class="count">{inboxBadge}</span>{/if}</a>
      <SidebarToggle hide />
    </div>
  {/if}

  {#if !store.connected}
    <p class="offline" role="status">You're offline. Den is trying to reconnect.</p>
  {/if}
  <div class="scroll" bind:this={scroller}>
    {#each voice as c (c.id)}
      {@const here = call.origin === store.origin && call.channel?.id === c.id}
      <!-- The server's list catches up a moment after you join; you're in already. -->
      {@const ids = here && store.me && !call.ids(c.id).includes(store.me.id) ? [store.me.id, ...call.ids(c.id)] : call.ids(c.id)}
      {@const jam = jamLine(c.id)}
      <!-- In the call, the card says what the call pill says: who's talking. -->
      {@const talking = here && !call.reconnecting ? callLine(call.participants).talking : ''}
      <!-- On a phone, the call pill above the list is this card while you're in the
           call, with your mic and Leave besides, so the list doesn't repeat it. -->
      {#if !(phone && here)}
      <div class="hang" class:live={ids.length > 0}>
        {#if ids.length}
          <CallFaces faces={here && call.participants.length ? callFaces(call.participants) : ids.map((userId) => ({ userId }))} size={phone ? 32 : 28} max={phone ? 3 : 2} />
        {:else}
          <RoomIcon channel={c} size={phone ? 44 : 34} />
        {/if}
        <span class="txt">
          <span class="nm">{c.name}{#if ids.length && !talking}<i class="live-dot" aria-hidden="true"></i>{/if}</span>
          <span class="who" class:talking>{#if talking}<span class="talk-bars" aria-hidden="true"><i></i><i></i><i></i></span><span class="say">{talking}</span>{:else}{who(ids)}{/if}</span>
          {#if jam}<span class="jam-line" data-testid="sidebar-jam" title={jam}><span aria-hidden="true">🎧</span> {jam}</span>{/if}
        </span>
        {#if here}
          <button class="btn small" onclick={() => (call.expanded = true)} aria-label={`Open ${c.name}`}>Open</button>
        {:else}
          <button class="btn lit small" disabled={call.joining === c.id} onclick={() => call.join(c)} aria-label={jam ? `Join ${c.name}. ${jam}` : `Join ${c.name}`}>{call.joining === c.id ? 'Joining…' : 'Join'}</button>
        {/if}
      </div>
      {/if}
    {/each}

    {#each uncategorized as c, i (c.id)}{@render room(c, i === uncategorized.length - 1)}{/each}

    {#each store.categories as cat (cat.id)}
      {@render group(cat.id, cat.name, store.textChannels.filter((c) => c.kind === 'text' && placedIn(c, store.categories) === cat.id))}
    {/each}

    {#if store.servers.length}
      {@const open = !folded('servers')}
      <button class="cat" aria-expanded={open} onclick={() => fold('servers')}><span>Servers</span><span class="chev" class:shut={!open}><Icon name="chevron" size={14} /></span></button>
      {#each open ? store.servers : store.servers.filter((sv) => serverActive(sv.slug)) as sv, i (sv.slug)}
        {@const playing = sv.connected && sv.state === 'up' && sv.players.length}
        {@const label = `${gameLabel(sv)}, ${stateLabel(sv).toLowerCase()}${playing ? `, ${sv.players.length} playing` : ''}`}
        <a href="/server/{sv.slug}" class="row server" class:last={i === store.servers.length - 1} class:bare={iconMode === 'none'} class:sel={!phone && serverActive(sv.slug)} aria-current={serverActive(sv.slug) ? 'page' : undefined} aria-label={label} title={label} onclick={go(`/server/${sv.slug}`)} data-testid="sidebar-server">
          {#if iconMode !== 'none'}<span class="tile" aria-hidden="true">{gameGlyph(sv.game)}</span>{/if}
          <span class="mid">
            <span class="top"><span class="nm">{gameLabel(sv)}</span></span>
            <span class="pv"><span class="sdot {dotTone(sv)}" aria-hidden="true"></span><span class="pvt">{stateLabel(sv)}{playing ? ` · ${sv.players.length} playing` : ''}</span></span>
          </span>
        </a>
      {/each}
    {/if}

    {@render group('direct', 'Direct', [...store.dms].sort((a, b) => (b.last_message?.id ?? '').localeCompare(a.last_message?.id ?? '')))}
  </div>

  {#if desktop.updateReady}<button class="btn quiet" onclick={() => desktop.restart()}>Update ready · Restart</button>{/if}
  {#if !phone}
    <CallDock />
    <div class="me">
      {#if store.me}
        <button class="me-card" aria-haspopup="dialog" onclick={(e) => profileCard.open(store.me!.id, e.currentTarget, instances.active)}>
          <Avatar userId={store.me.id} size={34} />
          <span class="t"><b>{store.me.display_name || store.me.username}</b>{#if status?.text || status?.emoji}<span>{status.emoji ?? ''} {status.text ?? ''}</span>{/if}</span>
        </button>
        <a href="/settings" class="icon-btn" class:on={router.route.name === 'settings'} aria-current={router.route.name === 'settings' ? 'page' : undefined} title="Settings" aria-label="Settings" onclick={go('/settings')}><Icon name="gear" /></a>
      {/if}
    </div>
  {/if}
</nav>

<style>
  .side { height: 100%; display: flex; flex-direction: column; }
  .spacer { flex: 1; }
  .brand { display: flex; align-items: center; gap: 4px; padding: 14px 10px 8px 18px; min-height: 58px; }
  .mac .brand { padding-top: 38px; }
  .wordmark { font-size: 24px; font-weight: 700; color: var(--ink); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .wordmark:hover { text-decoration: none; }
  .conn { width: 7px; height: 7px; border-radius: 50%; background: var(--moss); margin: 2px 4px 0; flex: none; }
  .conn.off { background: var(--danger); }
  /* A blink that reaches 0.3 opacity reads as broken rendering; and the global
     reduced-motion reset only shortens the duration, so guard the rule itself. */
  @media (prefers-reduced-motion: no-preference) {
    .conn.off { animation: blink 1.4s infinite alternate; }
  }
  @keyframes blink { to { opacity: 0.62; } }
  .icon-btn { position: relative; color: var(--ink-2); }
  .icon-btn:hover { text-decoration: none; }
  .inbox .count { position: absolute; top: 0; right: -2px; box-shadow: 0 0 0 2px var(--bg-2); }
  .offline {
    margin: 0 10px 6px; padding: 7px 10px; border-radius: var(--r);
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--ink); font-size: var(--fs-xs); line-height: 1.35;
  }
  .scroll { flex: 1; overflow-y: auto; padding: 0 8px 8px; }

  /* The hangout: who's in it, and one button. */
  .hang { --ring: var(--bg-3); display: flex; align-items: center; gap: 12px; margin: 4px 4px 6px; padding: 12px 12px 12px 14px; border-radius: var(--r-tile); background: var(--bg-3); }
  .hang .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.3; }
  .hang .nm { font-weight: 700; display: flex; align-items: center; gap: 6px; }
  .hang .who, .hang .jam-line { font-size: var(--fs-xs); color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hang .jam-line { color: var(--lamp); }
  .hang .who.talking { display: flex; align-items: center; gap: 6px; color: var(--lamp); }
  .hang .who .say { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .live-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--lamp); flex: none; }
  .btn.small { min-height: 32px; padding: 4px 14px; font-size: var(--fs-sm); }

  .cat { display: flex; align-items: center; gap: 4px; width: 100%; padding: 14px 10px 4px 12px; font-size: var(--fs-xs); font-weight: 600; color: var(--ink-3); text-align: left; border-radius: var(--r); }
  @media (hover: hover) { .cat:hover { color: var(--ink-2); } }
  .cat:active { background: var(--press); }
  .chev { display: grid; transition: transform var(--t-fast); }
  .chev.shut { transform: rotate(-90deg); }
  .sum { margin-inline-start: auto; font-variant-numeric: tabular-nums; color: var(--ink-2); }

  .row { display: flex; align-items: center; gap: 10px; padding: calc(7px * var(--density, 1)) 10px; margin: 1px 0; border-radius: var(--r-lg); color: var(--ink); text-align: left; transition: background-color var(--t-fast); }
  .row.bare { padding-inline: 12px; }
  .row:hover { background: var(--hover); text-decoration: none; }
  .row.sel { background: var(--sel); }
  .row.sel :global(.tile:not(.bare)) { background: color-mix(in srgb, var(--lamp) 22%, var(--bg-3)); color: var(--lamp); }
  .mid { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.3; }
  .top, .pv { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .top { align-items: baseline; }
  .nm { flex: 1; min-width: 0; font-size: 14px; font-weight: 500; color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .unread .nm, .sel .nm { font-weight: 700; color: var(--ink); }
  .tm { flex: none; font-size: var(--fs-xs); color: var(--ink-3); font-variant-numeric: tabular-nums; }
  .unread .tm { color: var(--lamp); }
  .pvt { flex: 1; min-width: 0; font-size: var(--fs-sm); color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pvt.typing { color: var(--lamp); }
  .tile { width: 34px; height: 34px; flex: none; display: grid; place-items: center; border-radius: var(--avatar-r, 50%); background: var(--bg-3); font-size: 17px; line-height: 1; }
  .uploading { font-size: var(--fs-sm); color: var(--lamp); }
  .count { flex: none; min-width: 18px; height: 18px; padding: 0 5px; display: inline-grid; place-items: center; border-radius: var(--r-pill, 999px); background: var(--lamp); color: var(--on-lamp); font-size: 11px; font-weight: 700; line-height: 1; font-variant-numeric: tabular-nums; }
  /* Up is green, asleep is a ring, down is red; starting and stopping sit between. */
  .sdot { width: 7px; height: 7px; border-radius: 50%; flex: none; }
  .sdot.up { background: var(--moss); }
  .sdot.busy { background: var(--lamp); }
  .sdot.asleep { box-shadow: inset 0 0 0 1.5px var(--ink-3); }
  .sdot.down { background: var(--danger); }

  .me { display: flex; align-items: center; gap: 6px; padding: 10px 10px 12px 14px; border-top: 1px solid color-mix(in srgb, var(--line) 60%, transparent); }
  .me-card { flex: 1; min-width: 0; display: flex; align-items: center; gap: 10px; margin: -4px; padding: 4px; border-radius: var(--r); text-align: left; transition: background-color var(--t-fast); }
  .me-card:hover { background: var(--hover); }
  .me-card .t { min-width: 0; display: flex; flex-direction: column; line-height: 1.25; }
  .me-card b, .me-card .t span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .me-card b { font-size: var(--fs-sm); }
  .me-card .t span { font-size: var(--fs-xs); color: var(--ink-3); }

  /* The phone home: Messages-sized rows with hairlines between them. */
  .phone .topbar { display: flex; align-items: center; gap: 4px; min-height: 52px; padding: 4px 8px 0 12px; }
  .me-btn { display: grid; place-items: center; width: var(--hit); height: var(--hit); border-radius: 50%; transition: transform var(--t) var(--ease-out); }
  .me-btn:active { transform: scale(.94); }
  .phone .large { margin: 0; padding: 0 20px 12px; font-size: var(--fs-large); font-weight: 700; line-height: 1.1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .find { display: flex; align-items: center; gap: 8px; margin: 0 16px 14px; padding: 0 12px; min-height: 40px; border-radius: var(--r-pill); background: var(--bg-3); color: var(--ink-3); border: 1px solid transparent; }
  .find:focus-within { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-glow); }
  .find input { flex: 1; min-width: 0; background: none; border: 0; outline: 0; color: var(--ink); font-size: 16px; padding: 8px 0; }
  .find input::placeholder { color: var(--ink-3); }
  /* The shell draws the ring; the field inside it draws none. */
  .find input:focus-visible { box-shadow: none; }
  .phone .scroll { padding: 0 0 calc(24px + env(safe-area-inset-bottom, 0px)); }
  .phone .hang { margin: 0 16px 8px; padding: 14px 14px 14px 16px; gap: 14px; background: var(--bg-2); }
  .phone .hang { --ring: var(--bg-2); }
  .phone .hang .nm { font-size: var(--fs-body); }
  .phone .cat { padding: 18px 20px 4px; font-size: var(--fs-xs); }
  .phone .row { position: relative; gap: 12px; padding: 10px 16px; min-height: 68px; margin: 0; border-radius: 0; }
  .phone .row::after { content: ''; position: absolute; left: 72px; right: 0; bottom: 0; height: 1px; background: color-mix(in srgb, var(--line) 70%, transparent); }
  .phone .row.bare::after { left: 16px; }
  .phone .row.last::after { display: none; }
  .phone .row:hover { background: none; }
  .phone .row:active { background: var(--press); }
  .phone .nm { font-size: calc(var(--fs-body) + 1px); font-weight: 600; color: var(--ink); }
  .phone .unread .nm { font-weight: 700; }
  .phone .tm { font-size: var(--fs-sm); }
  .phone .pvt { font-size: var(--fs-sm); }
  .phone .tile { width: 44px; height: 44px; font-size: 20px; }
  .phone .count { min-width: 20px; height: 20px; font-size: 12px; }
</style>
