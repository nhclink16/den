<script module lang="ts">
  import { untrack } from 'svelte'
  import { call } from '../lib/call.svelte'
  import { store, instances } from '../lib/store.svelte'
  // The phone list unmounts while a room is open. Coming back should find it
  // where it was, with the room you left in view.
  let savedScroll = 0
  let lastRoom = ''
  // A join that failed leaves Retry on the hangout row (the toast only says what happened), until the next
  // try. Watched for the life of the app, so a join that ends while a room is open is still known on return,
  // and kept with the account it happened to.
  let tried = '', owner = '', was = ''
  const join = $state({ failed: '', who: '' })
  const account = () => `${store.origin}:${store.me?.id ?? ''}`
  // New message: only the latest choice opens, whichever visit to the list made it.
  let picks = 0
  $effect.root(() => {
    $effect(() => {
      const id = call.joining
      // Each start of joining is a new attempt, owned by the server and account the call says it's for.
      if (id && id !== was) { tried = id; owner = untrack(() => `${call.origin}:${call.owner?.me?.id ?? ''}`); join.failed = '' }
      else if (call.channel) tried = ''
      else if (!id && tried && call.error) { join.failed = tried; join.who = owner; tried = '' }
      was = id ?? ''
    })
  })
</script>

<script lang="ts">
  import { onMount, tick } from 'svelte'
  import SidebarToggle from './SidebarToggle.svelte'
  import CallDock from './CallDock.svelte'
  import CallFaces from './CallFaces.svelte'
  import { router } from '../lib/router.svelte'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'
  import RoomIcon from './RoomIcon.svelte'
  import { native } from '../lib/native'
  import NavBar, { trouble } from './NavBar.svelte'
  import { disclosurePopover } from '../lib/disclosure-popover'
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
  // The home's search field is the way into Search: a tap or click opens it, focused, where results come as
  // you type. Tabbing through doesn't; typing and Enter search from here.
  let q = $state('')
  const search = () => router.go(q.trim() ? `/find?q=${encodeURIComponent(q.trim())}` : '/find', false, '/')
  const tapped = () => { if (document.documentElement.dataset.input === 'touch') search() }

  // New message: everyone you could start a DM with.
  let composing = $state(false)
  const people = $derived(store.people.filter((u) => u.id !== store.me?.id).sort((a, b) => (a.display_name || a.username).localeCompare(b.display_name || b.username)))
  // Leaving the list cancels a choice still on its way: coming back later shouldn't jump into it.
  $effect(() => () => { picks++ })
  async function message(userId: string) {
    composing = false
    // The concrete server, not the proxy: if someone switches servers meanwhile, this room isn't on the new one.
    // Only the latest choice opens, and only on the server and account it was made on.
    const s = instances.active, me = s.me?.id, mine = ++picks
    const here = () => picks === mine && instances.active === s && s.me?.id === me
    try {
      const c = await s.openDm([userId])
      if (here() && router.route.name === 'home') router.go(`/c/${c.id}`, false, '/')
    } catch (e) { if (here()) s.toast = (e as Error).message }
  }

  const failed = $derived(join.who === account() ? join.failed : '')

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
    if (!last) return { text: store.selfDm(c) ? 'No notes yet' : c.kind === 'dm' ? 'Say hello' : 'No messages yet', typing: false }
    const pair = c.kind === 'dm' && (c.member_ids || []).length <= 2
    const who = last.author_id === store.me?.id ? 'You: ' : pair ? '' : `${store.name(last.author_id)}: `
    return { text: who + previewText(last), typing: false }
  }
  function who(ids: string[]) {
    const names = ids.filter((id) => id !== store.me?.id).map((id) => store.name(id))
    const me = ids.includes(store.me?.id ?? '')
    if (!names.length) return me ? "You're in" : 'Nobody here yet'
    const list = names.length <= 3 ? names.slice(0, -1).join(', ') + (names.length > 1 ? ' and ' : '') + names.at(-1) : `${names.slice(0, 2).join(', ')} and ${names.length - 2} others`
    return me ? `You, ${list}` : `${list} ${names.length === 1 ? 'is' : 'are'} here`
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
  {#if !phone}
    <div class="brand">
      {#if native}<ServerSwitcher />{:else}<a href="/" class="display wordmark" onclick={go('/')} title={store.settings.instance_name}>{store.settings.instance_name}</a>{/if}
      <span class="spacer"></span>
      <a href="/find" class="icon-btn" class:on={router.route.name === 'search'} title="Search" aria-label="Search" onclick={go('/find')}><Icon name="search" /></a>
      <a href="/inbox" class="icon-btn inbox" class:on={router.route.name === 'inbox'} aria-current={router.route.name === 'inbox' ? 'page' : undefined} title="Inbox" aria-label={inboxCount ? `Inbox, ${inboxCount} unread` : 'Inbox'} onclick={go('/inbox')}><Icon name="inbox" />{#if inboxCount}<span class="count">{inboxBadge}</span>{/if}</a>
      <SidebarToggle hide />
    </div>
    {#if !store.connected}
      <p class="offline" role="status">You're offline. Den is trying to reconnect.</p>
    {/if}
  {/if}

  <div class="scroll" bind:this={scroller}>
    {#if phone}
      <!-- The large title and search scroll away; "Den" moves into the bar. Trouble with the connection
           shows there too, and nothing does while it's healthy. -->
      <NavBar title={store.settings.instance_name} large status={trouble(store.connected)}>
        {#snippet leading()}
          {#if store.me}<a href="/settings" class="me-btn" aria-label="Your profile and settings" onclick={go('/settings')}><Avatar userId={store.me.id} size={32} presence={false} /></a>{/if}
          {#if native}<ServerSwitcher />{/if}
        {/snippet}
        {#snippet trailing()}
          <details class="compose" bind:open={composing}>
            <summary class="icon-btn" aria-label="New message"><Icon name="compose" size={22} /></summary>
            <div class="sheet" popover="auto" use:disclosurePopover={'end'}>
              {#if composing}
                <div class="people" role="menu" aria-label="New message">
                  <p class="sheet-title" aria-hidden="true">New message</p>
                  {#each people as u (u.id)}
                    <button role="menuitem" onclick={() => message(u.id)}>
                      <Avatar userId={u.id} size={36} />
                      <span class="who-txt"><b>{u.display_name || u.username}</b><span>@{u.username}</span></span>
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </details>
          <a href="/inbox" class="icon-btn inbox" aria-label={inboxCount ? `Inbox, ${inboxCount} unread` : 'Inbox'} onclick={go('/inbox')}><Icon name="inbox" size={22} />{#if inboxCount}<span class="count">{inboxBadge}</span>{/if}</a>
        {/snippet}
      </NavBar>
      <form class="find input-shell" role="search" onsubmit={(e) => { e.preventDefault(); search() }}><Icon name="search" size={18} /><input bind:value={q} placeholder="Search" aria-label="Search messages" enterkeyhint="search" onfocus={tapped} onclick={search} /></form>
    {/if}
    {#each voice as c (c.id)}
      {@const here = call.origin === store.origin && call.channel?.id === c.id}
      <!-- The server's list catches up a moment after you join; you're in already. -->
      {@const ids = here && store.me && !call.ids(c.id).includes(store.me.id) ? [store.me.id, ...call.ids(c.id)] : call.ids(c.id)}
      {@const jam = jamLine(c.id)}
      <!-- In the call, the card says what the call pill says: who's talking. -->
      {@const talking = here && !call.reconnecting ? callLine(call.participants).talking : ''}
      {@const joining = call.joining === c.id}
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
          <span class="who" class:talking class:failed={failed === c.id && !joining && !ids.length}>{#if failed === c.id && !joining && !ids.length}Couldn't connect{:else if talking}<span class="talk-bars" aria-hidden="true"><i></i><i></i><i></i></span><span class="say">{talking}</span>{:else}{who(ids)}{/if}</span>
          {#if jam}<span class="jam-line" data-testid="sidebar-jam" title={jam}><span aria-hidden="true">🎧</span> {jam}</span>{/if}
        </span>
        {#if here}
          <button class="btn small" onclick={() => (call.expanded = true)} aria-label={`Open ${c.name}`}>Open</button>
        {:else}
          <!-- Joining stays in full colour with a spinner: it's working, not switched off. -->
          <button class="btn lit small" class:joining disabled={joining} onclick={() => call.join(c)} aria-label={failed === c.id && !joining ? `Retry joining ${c.name}` : jam ? `Join ${c.name}. ${jam}` : `Join ${c.name}`}>{#if joining}<span class="spin" aria-hidden="true"></span>Joining…{:else if failed === c.id}Retry{:else}Join{/if}</button>
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
  @media (hover: hover) { .wordmark:hover { text-decoration: none; } }
  .icon-btn { position: relative; color: var(--ink-2); }
  @media (hover: hover) { .icon-btn:hover { text-decoration: none; } }
  .inbox .count { position: absolute; top: 0; right: -2px; box-shadow: 0 0 0 2px var(--bg-2); }
  .offline {
    margin: 0 8px 6px; padding: 7px 10px; border-radius: var(--r);
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--ink); font-size: var(--fs-xs); line-height: 1.35;
  }
  .scroll { flex: 1; overflow-y: auto; padding: 0 8px 8px; }

  /* The hangout: who's in it, and one button. Same edges, corners and icon column as a room row below it. */
  .hang { --ring: var(--bg-3); display: flex; align-items: center; gap: 10px; margin: 4px 0 6px; padding: 12px 10px; border-radius: var(--r-lg); background: var(--bg-3); }
  .hang .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.3; }
  .hang .nm { font-weight: 700; display: flex; align-items: center; gap: 6px; }
  .hang .who, .hang .jam-line { font-size: var(--fs-xs); color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hang .jam-line { color: var(--lamp); }
  .hang .who.talking { display: flex; align-items: center; gap: 6px; color: var(--lamp); }
  .hang .who .say { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  /* Someone's in the call: live green, not the unread colour. */
  .live-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--moss); flex: none; }
  .btn.small { min-height: 32px; padding: 4px 14px; font-size: var(--fs-sm); }
  .btn.small.joining { opacity: 1; gap: 6px; }
  .hang .who.failed { color: var(--danger); }
  .spin { width: 12px; height: 12px; border-radius: 50%; border: 2px solid currentColor; border-right-color: transparent; }
  @media (prefers-reduced-motion: no-preference) { .spin { animation: spin .8s linear infinite; } }
  @keyframes spin { to { transform: rotate(1turn); } }

  .cat { display: flex; align-items: center; gap: 4px; width: 100%; padding: 14px 10px 4px 12px; font-size: var(--fs-xs); font-weight: 600; color: var(--ink-3); text-align: left; border-radius: var(--r); }
  @media (hover: hover) { .cat:hover { color: var(--ink-2); } }
  .cat:active { background: var(--press); }
  .chev { display: grid; transition: transform var(--t-fast); }
  .chev.shut { transform: rotate(-90deg); }
  .sum { margin-inline-start: auto; font-variant-numeric: tabular-nums; color: var(--ink-2); }

  .row { display: flex; align-items: center; gap: 10px; padding: calc(7px * var(--density, 1)) 10px; margin: 1px 0; border-radius: var(--r-lg); color: var(--ink); text-align: left; transition: background-color var(--t-fast); }
  .row.bare { padding-inline: 12px; }
  @media (hover: hover) { .row:hover { background: var(--hover); text-decoration: none; } }
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
  @media (hover: hover) { .me-card:hover { background: var(--hover); } }
  .me-card .t { min-width: 0; display: flex; flex-direction: column; line-height: 1.25; }
  .me-card b, .me-card .t span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .me-card b { font-size: var(--fs-sm); }
  .me-card .t span { font-size: var(--fs-xs); color: var(--ink-3); }

  /* The phone home: Messages-sized rows with hairlines between them. */
  .me-btn { display: grid; place-items: center; width: 44px; height: 44px; border-radius: 50%; transition: transform var(--t) var(--ease-out); }
  .me-btn:active { transform: scale(.94); }
  /* The badge sits on the tray's top right corner and stays inside the 16pt margin, so the tray moves in a little. */
  .phone .inbox { margin-inline-end: 7px; }
  .phone .inbox .count { top: 1px; right: 1px; box-shadow: 0 0 0 2px var(--bg); }
  .compose summary { list-style: none; cursor: pointer; }
  .compose summary::-webkit-details-marker { display: none; }
  .compose [popover] { position: fixed; inset: auto; margin: 0; padding: 0; width: min(360px, calc(100vw - 16px)); max-height: min(520px, calc(100vh - 120px)); overflow-y: auto; overscroll-behavior: contain; border: 1px solid var(--line-strong); border-radius: var(--r-lg); background: var(--bg-2); color: var(--ink); box-shadow: 0 14px 44px var(--shadow); }
  .people { display: grid; padding: 4px 12px 16px; }
  .sheet-title { margin: 8px 4px 10px; text-align: center; font-weight: 600; font-size: 17px; }
  .people button { display: flex; align-items: center; gap: 12px; min-height: 56px; padding: 6px 12px; background: var(--bg); text-align: start; color: var(--ink); }
  .people button + button { background-image: linear-gradient(var(--hair), var(--hair)); background-size: calc(100% - 60px) 1px; background-position: 60px 0; background-repeat: no-repeat; }
  .people button:first-of-type { border-radius: var(--r-lg) var(--r-lg) 0 0; }
  .people button:last-child { border-radius: 0 0 var(--r-lg) var(--r-lg); }
  .people button:only-of-type { border-radius: var(--r-lg); }
  .people button:active { background: var(--press); }
  .who-txt { min-width: 0; display: flex; flex-direction: column; line-height: 1.25; }
  .who-txt b { font-weight: 600; }
  .who-txt span { font-size: var(--fs-sm); color: var(--ink-3); }
  .find { display: flex; align-items: center; gap: 8px; margin: 0 16px 14px; padding: 0 12px; min-height: 40px; border-radius: var(--r-pill); background: var(--bg-3); color: var(--ink-3); border: 1px solid transparent; }
  :global(:where(:root:not([data-input='touch']))) .find:focus-within { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-glow); }
  .find input { flex: 1; min-width: 0; background: none; border: 0; outline: 0; color: var(--ink); font-size: 16px; padding: 8px 0; }
  .find input::placeholder { color: var(--ink-3); }
  /* The shell draws the ring; the field inside it draws none. */
  .find input:focus-visible { box-shadow: none; }
  .phone .scroll { padding: 0 0 calc(24px + env(safe-area-inset-bottom, 0px)); }
  .phone .hang { margin: 0 16px 8px; padding: 14px 14px 14px 16px; gap: 14px; border-radius: var(--r-tile); background: var(--bg-2); }
  .phone .hang { --ring: var(--bg-2); }
  .phone .hang .nm { font-size: var(--fs-body); }
  .phone .cat { padding: 18px 20px 4px; font-size: var(--fs-xs); }
  .phone .row { position: relative; gap: 12px; padding: 10px 16px; min-height: 68px; margin: 0; border-radius: 0; }
  .phone .row::after { content: ''; position: absolute; left: 72px; right: 0; bottom: 0; height: 1px; background: color-mix(in srgb, var(--line) 70%, transparent); }
  .phone .row.bare::after { left: 16px; }
  .phone .row.last::after { display: none; }
  @media (hover: hover) { .phone .row:hover { background: none; } }
  .phone .row:active { background: var(--press); }
  .phone .nm { font-size: calc(var(--fs-body) + 1px); font-weight: 600; color: var(--ink); }
  .phone .unread .nm { font-weight: 700; }
  .phone .tm { font-size: var(--fs-sm); }
  .phone .pvt { font-size: var(--fs-sm); }
  .phone .tile { width: 44px; height: 44px; font-size: 20px; }
  .phone .count { min-width: 20px; height: 20px; font-size: 12px; }
</style>
