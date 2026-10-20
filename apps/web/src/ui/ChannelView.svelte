<script lang="ts">
  import MusicComposer from './MusicComposer.svelte'
  import JamCard from './JamCard.svelte'
  import SidebarToggle from './SidebarToggle.svelte'
  import HomeButton from './HomeButton.svelte'
  import NavBar, { trouble } from './NavBar.svelte'
  import Avatar from './Avatar.svelte'
  import { profileCard } from '../lib/people.svelte'
  import RoomIcon from './RoomIcon.svelte'
  import { roomName } from '../lib/room-list'
  import { themes } from '../lib/theme.svelte'
  import ObjectDock from './ObjectDock.svelte'
  import { objects } from '../lib/objects.svelte'
  import { call } from '../lib/call.svelte'
  import CallView from './CallView.svelte'
  import { store, instances } from '../lib/store.svelte'
  import type { Channel, Message } from '../lib/types'
  import Icon from './Icon.svelte'
  import MessageList, { type Source } from './MessageList.svelte'
  import Composer from './Composer.svelte'
  import ThreadStrip from './ThreadStrip.svelte'
  import ThreadPanel from './ThreadPanel.svelte'
  import { router } from '../lib/router.svelte'
  import { viewing } from '../lib/notify.svelte'
  import { native } from '../lib/native'
  import { tick, untrack } from 'svelte'
  let q = $state('')
  let listening = $state(false)
  // Searches everywhere: a miss in this room reads as "the message is gone".
  function search(e: SubmitEvent) { e.preventDefault(); if (q.trim()) router.go(`/find?q=${encodeURIComponent(q.trim())}`) }

  let { channel, narrow }: { channel: Channel; narrow: boolean } = $props()
  let replyTo = $state<Message | null>(null)
  // The message box's height, and whether messages are out of sight under it: on a phone the list runs under it.
  let dockHeight = $state(0), below = $state(false)
  let dragging = $state(0)
  let dropped = $state<File[]>([])

  // A rejected first page is shown and retryable; an empty room and a room that
  // failed to load are not the same thing.
  let roomError = $state('')
  // Only the newest load may report a failure: a reconnect's retry and the resync's reload can overlap.
  let loads = 0
  function loadRoom() {
    const mine = ++loads
    roomError = ''
    void store.loadLatest(channel.id).catch((e) => { if (mine === loads) roomError = (e as Error).message })
  }
  $effect(() => { if (channel.kind !== 'voice' && !store.messages.has(channel.id)) loadRoom() })
  // Once Den is back, a room that couldn't load tries again.
  let wasConnected = store.connected
  $effect(() => {
    const now = store.connected
    if (now && !wasConnected && untrack(() => roomError)) loadRoom()
    wasConnected = now
  })

  // Closing the room's window cancels its in-flight pages when the target goes.
  $effect(() => {
    const key = roomKey
    if (!key) return
    return () => store.closeWindow(key)
  })
  // The room's open conversations, ordered once per visit. With none open, one
  // resolved conversation is enough to show the strip and its Resolved history.
  // A failed load shows the strip anyway, so its Retry is there.
  let threadsFailed = $state(false)
  $effect(() => {
    threadsFailed = false
    if (channel.kind === 'voice') return
    const id = channel.id
    void store.loadThreads(id, { resolved: false })
      .then(async () => { if (!store.threadsIn(id).length) await store.loadThreads(id, { resolved: true, limit: 1 }) })
      .then(() => store.orderThreads(id)).catch(() => { if (channel.id === id) threadsFailed = true })
  })

  // Which conversation is open comes from the URL, so back, forward, reload and
  // a pasted link all agree. `reply` is a conversation the server has not made a
  // thread for yet, identified by its root.
  const openThread = $derived(router.route.name === 'channel' ? router.route.thread : undefined)
  const openReply = $derived(router.route.name === 'channel' ? router.route.reply : undefined)
  const target = $derived(router.route.name === 'channel' ? router.route.message : undefined)
  // A conversation only belongs to THIS room if the server says so. A known
  // thread from another channel, reached under this URL, must not render its
  // content here, suppress its notifications, or be acknowledged as if it were.
  const known = $derived(openThread ? store.thread(openThread) : undefined)
  const wrongRoom = $derived(!!known && known.channel_id !== channel.id)
  const rootOf = $derived(
    openThread ? (wrongRoom ? undefined : known?.root_message_id) : openReply,
  )
  const panel = $derived(!!(openThread || openReply))

  // A saved URL can be opened cold: pasted, reloaded, or followed to a resolved
  // conversation that no open list would ever have mentioned. Nothing else
  // fetches that metadata, so this does, and says which of the three states it
  // is in rather than showing an empty panel.
  let loadingThread = $state(false)
  let threadGone = $state('')
  let retryable = $state(false)
  // Which route the in-flight load belongs to. A failure for the conversation
  // someone has already navigated away from must not overwrite the one they are
  // looking at now, and must not leave its spinner running.
  let loadingFor = $state('')
  function loadRoute(id: string) {
    const mine = `${channel.id}/${id}`
    loadingFor = mine
    loadingThread = true
    threadGone = ''
    retryable = false
    void store.loadThread(id)
      .then((view) => {
        if (loadingFor !== mine) return
        if (view && view.thread.channel_id !== channel.id) {
          threadGone = 'That conversation is not in this room.'
        }
      })
      .catch((e) => {
        if (loadingFor !== mine) return
        const status = (e as { status?: number }).status
        // Only confirmed missing or denied is unavailable; anything else is
        // transient and gets an explicit Retry rather than a dead end.
        threadGone = status === 404 ? 'That conversation no longer exists.'
          : status === 403 ? "You don't have access to that conversation."
          : (e as Error).message
        retryable = status !== 404 && status !== 403
      })
      .finally(() => { if (loadingFor === mine) loadingThread = false })
  }
  $effect(() => {
    const id = openThread
    // Switching routes cancels whatever the previous one was doing.
    loadingFor = id ? `${channel.id}/${id}` : ''
    loadingThread = false
    threadGone = wrongRoom ? 'That conversation is not in this room.' : ''
    retryable = false
    if (!id || known) return
    loadRoute(id)
  })

  // Wide enough for both, measured on the actual chat container rather than the
  // window: the sidebar, the member list and browser zoom all take from it.
  //
  // The two panes flex equally, so this threshold is simply the container width
  // below which each half is too narrow to work in — not an allocation, and not
  // an orientation test. A real Electron window at 820 showed why the old 820
  // was wrong: the Shell is already narrow there, so the container gets the full
  // 820, the comparison passed on its exact boundary, and a portrait window
  // rendered two ~410px columns with no way back to the room.
  let space = $state(0)
  const PAIR = 920
  const pair = $derived(space >= PAIR)
  // The room is still mounted when the panel takes the width — that is what
  // keeps its draft, its uploads and its scroll position — but it is inert and
  // it stops acknowledging, because nobody is reading it.
  const stowed = $derived(panel && !pair)

  // Mark read while we're looking at it and the tab is visible, and again when it becomes visible.
  let roomTail = $state('')
  const readRoom = () => {
    if (!roomShown || document.visibilityState !== 'visible' || !roomTail) return
    void store.markRead(channel.id, roomTail)
  }
  $effect(() => { void roomTail; void roomShown; readRoom() })
  $effect(() => {
    const fn = () => readRoom()
    document.addEventListener('visibilitychange', fn)
    return () => document.removeEventListener('visibilitychange', fn)
  })

  // An old ROOT target gets a contiguous window, owned by the Store for the same
  // reason the panel's is: one update path, and never mistaken for the tail.
  const roomTarget = $derived(panel ? undefined : target)
  const roomInTail = $derived(!!roomTarget && (store.messages.get(channel.id) || []).some((m) => m.id === roomTarget))
  const roomKey = $derived(roomTarget && !roomInTail ? store.windowKey(channel.id, undefined, roomTarget) : '')
  $effect(() => {
    const want = roomTarget
    if (!roomKey || !want) return
    void store.openWindow(channel.id, undefined, want)
  })
  const roomWindow = $derived(roomKey ? store.windowAt(roomKey) : undefined)
  const roomWindowStatus = $derived(roomKey ? store.windowStatus(roomKey) : undefined)
  const roomWindowed = $derived(!!roomWindow?.length)
  // Messages already here (the latest, or an older part opened from a link) stay on screen; with none,
  // the room says it couldn't load.
  const cached = $derived(!!store.messages.get(channel.id)?.length || roomWindowed)

  const roomSource = $derived<Source>({
    list: roomWindowed ? roomWindow! : store.messages.get(channel.id) || [],
    lastRead: store.unread(channel.id).lastRead,
    exhausted: roomWindowed ? !!roomWindowStatus?.start : store.exhausted.has(channel.id),
    loadingOlder: roomWindowed ? !!roomWindowStatus?.loading : store.loadingOlder.has(channel.id),
    older: () => (roomWindowed
      ? store.extendWindow(roomKey, channel.id, undefined, true)
      : store.loadOlder(channel.id)),
    // A window is not the tail: reading it must not acknowledge the tail.
    onread: (displayed) => { if (!roomWindowed) roomTail = displayed },
    target: roomTarget,
    prefix: 'm',
    identity: `room:${channel.id}`,
    identityExtra: roomKey,
    hidden: stowed,
  })

  // Opening a conversation from the room PUSHES, recording the room as its
  // parent. Switching conversations while one is already open REPLACES that
  // entry and keeps the same parent, so Back to room still means the room.
  let opener = $state('')
  let restoring = $state('')
  const roomUrl = $derived(`/c/${channel.id}`)
  function open(rootId: string, threadId?: string, from?: string) {
    opener = from ?? rootId
    const to = threadId ? `/c/${channel.id}/t/${threadId}` : `/c/${channel.id}?reply=${rootId}`
    if (panel) router.go(to, true)
    else router.go(to, false, roomUrl)
  }
  function closePanel() {
    restoring = opener
    // Real back only when the recorded parent IS this room. A parent pointing
    // somewhere else is not something this control should walk into, and the
    // replacement below also clears it so a stale one cannot be inherited.
    if (router.parent === roomUrl) router.back()
    else router.go(roomUrl, true, '')
  }
  // Focus returns only once the route has really closed the panel and the room
  // is no longer inert; popstate is asynchronous, so doing it at click time
  // would put focus into a hidden pane.
  $effect(() => {
    if (panel || !restoring || stowed) return
    const back = restoring
    restoring = ''
    void tick().then(() => {
      const opener = document.querySelector<HTMLButtonElement>(`#m-${CSS.escape(back)} button[title="Reply"]`)
      const fallback = document.querySelector<HTMLElement>('.view h1')
      ;(opener ?? fallback)?.focus()
    })
  })
  // The first reply saves the thread: swap the pre-creation URL for the canonical
  // one in place, so Back does not return to a conversation that now exists.
  $effect(() => {
    if (!openReply) return
    const made = store.threadForRoot(openReply)
    if (made) router.go(`/c/${channel.id}/t/${made.id}`, true)
  })

  // Publish what is actually on screen, so an alert for a conversation nobody
  // can see still arrives.
  // What is actually on screen. An expanded call or object covers both
  // timelines, and a conversation that has not loaded is not being read either,
  // so neither may certify anything as seen.
  const covered = $derived(!!(call.channel && call.expanded) || objects.expanded)
  // Not merely "the route names a root", but "the panel actually mounted a valid
  // one". A panel showing "not in this room" is not displayed content and must
  // not suppress alerts or certify a read.
  let panelValid = $state(false)
  const panelShown = $derived(panel && !!rootOf && panelValid && !covered)
  const roomShown = $derived(!stowed && !covered)
  $effect(() => {
    viewing.channelId = channel.id
    viewing.threadId = panelShown ? openThread : undefined
    viewing.roomVisible = roomShown
    return () => { viewing.channelId = ''; viewing.threadId = undefined; viewing.roomVisible = false }
  })

  const typing = $derived(store.typingNames(channel.id))
  const isDm = $derived(channel.kind === 'dm')
  // An expanded call hides this room completely, so the header names the call.
  const callCovers = $derived(!!call.channel && call.expanded && !objects.expanded)
  // On a phone the open call and a thread each take the whole screen, the way a
  // room covers the list: their own header row, with its own way back, replaces
  // the room's. Not in the desktop app: there the room header is what drags the
  // window, and its narrowest window is no phone.
  const pushed = $derived(narrow && !native && (callCovers || stowed))
  // A phone gets the iPhone bar. The desktop app keeps its header even in a narrow window: that header is
  // what drags the window.
  const nav = $derived(narrow && !native)
  // The person on the other end of a two-person DM, whose face and profile the bar offers.
  const other = $derived(isDm && (channel.member_ids || []).length <= 2 ? (channel.member_ids || []).find((id) => id !== store.me?.id) : undefined)
  // The messages, with nothing above them in the room: no error in their place, no note about an older part.
  const listed = $derived(!(roomError && !cached))
  const under = $derived(nav && listed && !roomKey)
  const showThreads = $derived(channel.kind === 'text' && !callCovers && (store.threadsIn(channel.id, false).length || threadsFailed))

  function onDrop(e: DragEvent) {
    e.preventDefault(); e.stopPropagation(); dragging = 0
    if (stowed) return
    const files = [...(e.dataTransfer?.files || [])]
    if (files.length) dropped = files
  }
</script>

{#snippet person()}
  <h1 class="person">
    <button onclick={(e) => other && profileCard.open(other, e.currentTarget, instances.active)} disabled={!other} aria-label={other ? `${store.title(channel)}'s profile` : undefined}>
      {#if other}<Avatar userId={other} size={26} presence={false} />{:else}<RoomIcon {channel} size={26} />{/if}
      <span>{store.title(channel)}</span>
    </button>
  </h1>
{/snippet}
{#snippet threadLine()}<ThreadStrip {channel} selected={openThread} onopen={(id, root) => open(root, id)} subtitle />{/snippet}
{#snippet roomActions()}
  {#if isDm}<button class="icon-btn" onclick={() => call.join(channel)} aria-label="Call"><Icon name="phone" size={22} /></button>{/if}
  <a href="/find" class="icon-btn" aria-label="Search messages" onclick={(e) => { e.preventDefault(); router.go('/find', false, roomUrl) }}><Icon name="search" size={22} /></a>
{/snippet}

<!-- `edge`: the bar is the first thing on a phone screen, so it reaches up under the status bar (Shell). -->
<section class="view" class:pushed class:edge={nav && (!pushed || !call.channel)} aria-label={store.title(channel)}>
  {#if nav && !pushed}
  <NavBar
    title={store.title(channel)} status={trouble(store.connected)} subtitle={roomError && cached ? "Couldn't load new messages" : ''}
    heading={isDm ? person : undefined} subline={showThreads && !(roomError && cached) ? threadLine : undefined}
  >
    {#snippet leading()}<HomeButton />{/snippet}
    {#snippet trailing()}{@render roomActions()}{/snippet}
  </NavBar>
  {:else if !pushed}
  <header class="head">
    {#if !narrow && !store.layout.sidebar}<SidebarToggle />{/if}
    {#if narrow}<HomeButton />{/if}
    <span class="kind">{#if callCovers}<Icon name="headset" size={18} />{:else if isDm}<Icon name="lock" />{:else}<RoomIcon {channel} size={18} bare />{/if}</span>
    <div class="titles">
      <h1 class="display" tabindex="-1">{callCovers ? call.title : store.title(channel)}</h1>
      <!-- Only where there is something to show, and DMs open their threads from
           the line under each message. -->
      {#if showThreads}
        <ThreadStrip {channel} selected={openThread} onopen={(id, root) => open(root, id)} />
      {/if}
    </div>
    <span class="spacer"></span>
    {#if isDm && !callCovers}<button class="btn quiet dm-call" onclick={() => call.join(channel)} aria-label="Call"><Icon name="phone" /> <span>Call</span></button>{/if}
    <form class="search input-shell" onsubmit={search}><Icon name="search" size={14} /><input bind:value={q} placeholder="Search" aria-label="Search messages" /></form>
    <!-- A phone has no room for a field beside the title: one tap opens Search. -->
    <a href="/find" class="icon-btn find" aria-label="Search messages" onclick={(e) => { e.preventDefault(); router.go('/find') }}><Icon name="search" size={20} /></a>
    {#if !narrow}
      <button class="icon-btn" title="Toggle people (Ctrl+Shift+M)" aria-label="People" onclick={() => store.saveLayout({ members: !store.layout.members })}><Icon name="people" /></button>
    {/if}
  </header>
  {/if}

  {#if channel.kind !== 'text'}<JamCard channelId={channel.id} />{/if}
  {#if call.channel && call.owner && call.channel.id !== channel.id && !objects.expanded}<JamCard channelId={call.channel.id} owner={call.owner} compact />{/if}

  {#if call.channel && !objects.expanded}<CallView {narrow} back={store.title(channel)} titled={pushed && callCovers} />{/if}
  {#if objects.active}<div class="object-slot" class:hidden={!!call.channel && call.expanded && !objects.expanded}><ObjectDock /></div>{/if}
  {#if (!call.channel || !call.expanded) && !objects.expanded}
  {#if channel.kind === 'voice'}
    <div class="voice-empty"><div class="voice-music">{#if call.channel?.id !== channel.id}<button class="btn lit" onclick={() => call.join(channel)}><Icon name="headset" /> Join {channel.name}</button>{/if}<h2>Bring a song</h2><p>Music plays for everyone in the call.</p><MusicComposer roomId={channel.id} /><p class="jam-hint">{call.channel?.id === channel.id ? 'For a Spotify Jam, open Music in the call controls.' : 'Spotify Jams start from inside the call.'}</p></div></div>
  {:else}
  {#if isDm && call.channel?.id !== channel.id && call.ids(channel.id).length}
    <button class="call-banner" onclick={() => call.join(channel)}><Icon name="phone" /> {call.ids(channel.id).map((id) => store.name(id)).join(', ')} {call.ids(channel.id).length === 1 ? 'is' : 'are'} in a call · Join</button>
  {/if}
  <div class="content" class:under bind:clientWidth={space}>
    <!-- Files belong to the pane they were dropped on. The room owns this one;
         the panel owns its own, and a stowed room is not a drop target. -->
    <div
      class="room" class:stowed inert={stowed || undefined} style:--list-bottom={nav ? `${dockHeight}px` : undefined}
      role="group" aria-label={`Main conversation in ${store.title(channel)}`}
      ondragenter={(e) => { e.preventDefault(); e.stopPropagation(); dragging++ }}
      ondragleave={() => dragging--}
      ondragover={(e) => { e.preventDefault(); e.stopPropagation() }}
      ondrop={onDrop}
    >
      {#if roomError && !cached}
        <!-- Nothing to show: say so plainly, with one way to try again. -->
        <div class="failed" role="alert">
          <Icon name="offline" size={40} />
          <h2>Can't load {roomName(channel, themes.appearance.room_icons)}</h2>
          <p>{store.connected ? 'Something went wrong on the way. Try again in a moment.' : "You're offline. Den tries again as soon as you're back."}</p>
          <button class="btn lit" onclick={loadRoom}>Try Again</button>
        </div>
      {:else if roomError && !nav}
        <p class="windowed" role="alert">
          {roomError}
          <button class="btn quiet" onclick={loadRoom}>Retry</button>
        </p>
      {/if}
      <!-- The room's own old-target states, with the way back to the present. -->
      {#if roomKey && roomWindowStatus?.error}
        <p class="windowed" role="alert">
          {roomWindowStatus.error}
          <button class="btn quiet" onclick={() => store.retryWindow(roomKey, channel.id, undefined)}>Retry</button>
          <button class="btn quiet" onclick={() => router.go(roomUrl, true)}>Jump to newest</button>
        </p>
      {:else if roomKey && roomWindowStatus?.loading && !roomWindowed}
        <p class="windowed faint" aria-busy="true">Finding that message…</p>
      {:else if roomWindowed}
        <p class="windowed faint">
          Showing an older part of this room.
          {#if !roomWindowStatus?.end}
            <button class="btn quiet" onclick={() => store.extendWindow(roomKey, channel.id, undefined, false)} disabled={roomWindowStatus?.loading}>
              {roomWindowStatus?.loading ? 'Loading…' : 'Newer'}
            </button>
          {/if}
          <button class="btn quiet" onclick={() => router.go(roomUrl, true)}>Jump to newest</button>
        </p>
      {/if}
      {#if listed}<MessageList {channel} source={roomSource} onreply={(m) => open(m.id, m.thread?.id, m.id)} bind:below />{/if}

      <div class="dock" class:over={nav && listed} class:frost={nav && listed && below} bind:offsetHeight={dockHeight}>
        <div class="typing" aria-live="polite">
          {#if listening}<span>listening</span>{:else if typing.length}<span class="dots" aria-hidden="true"><i></i><i></i><i></i></span><b>{typing.join(', ')}</b> {typing.length === 1 ? 'is' : 'are'} typing{/if}
        </div>

        <Composer {channel} bind:replyTo bind:dropped bind:listening />
      </div>
      {#if dragging > 0}
        <div class="drop">
          <div class="drop-inner">
            <Icon name="clip" size={28} />
            <span class="display big">Drop it in {roomName(channel, themes.appearance.room_icons)}</span>
            <span class="muted">Anything up to 1 GB. Clips play right in the chat.</span>
          </div>
        </div>
      {/if}
    </div>
    {#if panel && rootOf}
      <ThreadPanel
        {channel} rootId={rootOf} threadId={openThread} target={target} narrow={!pair} phone={nav}
        onclose={closePanel}
        onjump={() => router.go(openThread ? `/c/${channel.id}/t/${openThread}` : `/c/${channel.id}`, true)}
        onvalid={(ok) => (panelValid = ok)}
      />
    {:else if panel && loadingThread}
      <section class="panel-missing" aria-label="Conversation" aria-busy="true">
        <p class="muted">Opening that conversation…</p>
        <!-- On a phone the room's header has given way, so the way back is here. -->
        <button class="btn quiet" onclick={closePanel}>Back to room</button>
      </section>
    {:else if panel}
      <section class="panel-missing" aria-label="Conversation">
        <p class="muted" role="alert">{threadGone || "That conversation isn't available here."}</p>
        <div class="row">
          {#if retryable && openThread}
            <button class="btn quiet" onclick={() => loadRoute(openThread)}>Retry</button>
          {/if}
          <button class="btn quiet" onclick={closePanel}>Back to room</button>
        </div>
      </section>
    {/if}
  </div>
  {/if}
  {/if}

</section>

<style>
  .content { flex: 1; min-height: 0; display: flex; position: relative; }
  .room { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; position: relative; }
  /* Kept in the layout so its scroll, draft and uploads survive, but out of
     sight, out of the tab order and no longer being read. */
  .room.stowed { visibility: hidden; position: absolute; inset: 0; }
  .windowed { display: flex; align-items: center; gap: 10px; margin: 0; padding: 6px var(--gutter); border-bottom: 1px solid var(--line); font-size: var(--fs-xs); }
  .failed { flex: 1; display: grid; place-content: center; justify-items: center; gap: 8px; padding: 24px 32px; text-align: center; color: var(--ink-3); }
  .failed h2 { margin: 8px 0 0; font-size: var(--fs-title); font-weight: 600; color: var(--ink); }
  .failed p { margin: 0 0 10px; max-width: 30ch; font-size: var(--fs-sm); color: var(--ink-2); text-wrap: pretty; }
  /* A DM's bar title: the person's face and name, which open their profile. */
  .person { margin: 0; max-width: 100%; font-size: inherit; }
  .person button { display: flex; align-items: center; gap: 7px; max-width: 100%; padding: 2px 4px; color: var(--ink); font: 600 17px/1.25 var(--display); }
  .person button:active:not(:disabled) { opacity: .6; }
  .person span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .panel-missing { flex: 1; display: grid; place-content: center; gap: 10px; justify-items: center; padding: 24px; text-align: center; }
  .panel-missing .row { display: flex; gap: 8px; }
  .object-slot { display: contents; }
  .object-slot.hidden { display: none; }
  /* --nav-h is the NavBar's height: its 44pt row, under the status bar when it's first on screen. */
  .view { flex: 1; min-height: 0; display: flex; flex-direction: column; position: relative; --nav-h: calc(var(--nav-inset, 0px) + 44px); }
  /* A phone: the messages run up under the bar when they're the first thing below it (no call, note or error
     between), and down under the message box, the iPhone way. MessageList pads itself by both. */
  .view > :global(.navbar + .content.under) { margin-top: calc(-1 * var(--nav-h)); --list-top: var(--nav-h); }
  .dock.over { position: relative; z-index: 3; margin-top: calc(-1 * var(--list-bottom)); transition: background-color var(--t-fast), box-shadow var(--t-fast); }
  /* The same material as the bar, once there's something under it. */
  .dock.frost {
    background: color-mix(in srgb, var(--nav-bg, var(--bg)) var(--bar-alpha), transparent);
    -webkit-backdrop-filter: var(--bar-blur); backdrop-filter: var(--bar-blur);
    box-shadow: 0 -.5px 0 var(--line);
  }
  .head {
    display: flex; align-items: center; gap: 10px; padding: 10px var(--gutter); min-height: 52px;
    border-bottom: 1px solid var(--line);
  }
  .kind { color: var(--ink-3); display: grid; }
  .kind:empty { display: none; }
  h1 { font-size: var(--fs-title); margin: 0; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* The open-threads pill sits beside the title, and under it on a phone. */
  .titles { display: flex; align-items: center; gap: 10px; min-width: 0; }
  .spacer { flex: 1; }
  /* The same search field as Settings and the Search page; app.css draws its focus ring. */
  .search { display: flex; align-items: center; gap: 6px; padding: 4px 10px; border: 1px solid transparent; border-radius: var(--r-pill); color: var(--ink-3); background: var(--bg-3); }
  /* The transparent border above outranks app.css's shared ring, so restate it. */
  :global(:where(:root:not([data-input='touch']))) .search:focus-within { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-glow); }
  .search input { background: none; border: 0; width: 120px; font-size: var(--fs-sm); color: var(--ink); transition: width 0.15s; }
  .search input:focus { width: 200px; }
  .search input::placeholder { color: var(--ink-3); }
  .call-banner { display: flex; align-items: center; gap: 8px; padding: 10px var(--gutter); color: var(--lamp); background: var(--bg-3); border-bottom: 1px solid var(--line); text-align: left; }
  .voice-music { width: min(360px, calc(100% - 32px)); }
  .voice-music h2 { font-size: 22px; margin: 20px 0 4px; }
  .voice-music p { color: var(--ink-2); margin: 0 0 18px; font-size: var(--fs-sm); }
  .voice-music .jam-hint { margin: 14px 0 0; font-size: var(--fs-xs); }
  .voice-empty { flex: 1; display: grid; place-items: center; }
  .find { display: none; }
  @media (max-width: 600px) { .head { gap: 6px; padding-inline: 10px; } .titles { flex-direction: column; align-items: flex-start; gap: 2px; } .search { display: none; } .find { display: grid; } .dm-call { padding: 6px; } .dm-call span { display: none; } }
  .typing { height: 20px; padding: 0 var(--gutter); display: flex; align-items: center; gap: 4px; font-size: var(--fs-xs); color: var(--ink-2); }
  .typing b { font-weight: 700; color: var(--ink); }
  .dots { display: inline-flex; gap: 3px; margin-right: 4px; }
  .dots i { width: 5px; height: 5px; border-radius: 50%; background: var(--lamp); }
  @media (prefers-reduced-motion: no-preference) {
    .dots i { animation: bob 1.1s ease-in-out infinite; }
    .dots i:nth-child(2) { animation-delay: .15s; }
    .dots i:nth-child(3) { animation-delay: .3s; }
  }
  @keyframes bob { 0%, 60%, 100% { transform: none; opacity: .5; } 30% { transform: translateY(-3px); opacity: 1; } }
  .drop {
    position: absolute; inset: 8px; z-index: 5; border-radius: var(--r-lg);
    background: var(--overlay); border: 2px dashed var(--lamp);
    display: grid; place-items: center; pointer-events: none;
  }
  .drop-inner { display: grid; justify-items: center; gap: 6px; color: var(--lamp); }
  .big { font-size: 26px; }
</style>
