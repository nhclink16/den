<script lang="ts">
  import MusicComposer from './MusicComposer.svelte'
  import JamCard from './JamCard.svelte'
  import JamComposer from './JamComposer.svelte'
  import SidebarToggle from './SidebarToggle.svelte'
  import ObjectDock from './ObjectDock.svelte'
  import { objects } from '../lib/objects.svelte'
  import { call } from '../lib/call.svelte'
  import CallView from './CallView.svelte'
  import { store } from '../lib/store.svelte'
  import type { Channel, Message } from '../lib/types'
  import Icon from './Icon.svelte'
  import MessageList, { type Source } from './MessageList.svelte'
  import Composer from './Composer.svelte'
  import ThreadStrip from './ThreadStrip.svelte'
  import ThreadPanel from './ThreadPanel.svelte'
  import { router } from '../lib/router.svelte'
  import { viewing } from '../lib/notify.svelte'
  import { tick } from 'svelte'
  let q = $state('')
  let listening = $state(false)
  function search(e: SubmitEvent) { e.preventDefault(); if (q.trim()) router.go(`/find?q=${encodeURIComponent(q.trim())}&in=${channel.id}`) }

  let { channel, onmenu, narrow }: { channel: Channel; onmenu: () => void; narrow: boolean } = $props()
  let replyTo = $state<Message | null>(null)
  let dragging = $state(0)
  let dropped = $state<File[]>([])

  // A rejected first page is shown and retryable; an empty room and a room that
  // failed to load are not the same thing.
  let roomError = $state('')
  function loadRoom() {
    roomError = ''
    void store.loadLatest(channel.id).catch((e) => { roomError = (e as Error).message })
  }
  $effect(() => { if (channel.kind !== 'voice' && !store.messages.has(channel.id)) loadRoom() })

  // Closing the room's window cancels its in-flight pages when the target goes.
  $effect(() => {
    const key = roomKey
    if (!key) return
    return () => store.closeWindow(key)
  })
  // The room's open conversations, ordered once per visit.
  $effect(() => {
    if (channel.kind === 'voice') return
    const id = channel.id
    void store.loadThreads(id, { resolved: false }).then(() => store.orderThreads(id)).catch(() => {})
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
      const fallback = document.querySelector<HTMLElement>('.view .head h1')
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

  function onDrop(e: DragEvent) {
    e.preventDefault(); e.stopPropagation(); dragging = 0
    if (stowed) return
    const files = [...(e.dataTransfer?.files || [])]
    if (files.length) dropped = files
  }
</script>

<section class="view" aria-label={store.title(channel)}>
  <header class="head">
    {#if !narrow && !store.layout.sidebar}<SidebarToggle />{/if}
    {#if narrow}<button class="btn quiet iconbtn" onclick={onmenu} aria-label="Menu"><Icon name="menu" /></button>{/if}
    <span class="kind">{#if isDm}<Icon name="lock" />{:else}<Icon name={channel.kind === 'voice' ? 'headset' : 'hash'} size={18} />{/if}</span>
    <h1 class="display" tabindex="-1">{store.title(channel)}</h1>
    <span class="spacer"></span>
    {#if isDm}<button class="btn quiet dm-call" onclick={() => call.join(channel)} aria-label="Call"><Icon name="phone" /> <span>Call</span></button>{/if}
    <form class="search" onsubmit={search}><Icon name="search" size={14} /><input bind:value={q} placeholder={narrow ? 'Search' : isDm ? `Search ${store.title(channel)}` : `Search #${channel.name}`} aria-label={isDm ? 'Search this conversation' : 'Search this room'} /></form>
    {#if !narrow}
      <button class="btn quiet iconbtn" title="Toggle people (Ctrl+Shift+M)" onclick={() => store.saveLayout({ members: !store.layout.members })}><Icon name="people" /></button>
    {/if}
  </header>

  <JamCard channelId={channel.id} />

  {#if call.channel && !objects.expanded}<CallView />{/if}
  {#if objects.active}<div class="object-slot" class:hidden={!!call.channel && call.expanded && !objects.expanded}><ObjectDock /></div>{/if}
  {#if (!call.channel || !call.expanded) && !objects.expanded}
  {#if channel.kind === 'voice'}
    <div class="voice-empty"><div class="voice-music">{#if call.channel?.id !== channel.id}<button class="btn lit" onclick={() => call.join(channel)}><Icon name="headset" /> Join {channel.name}</button>{/if}<h2>Bring a song</h2><p>Music plays for everyone in the call.</p><MusicComposer roomId={channel.id} /><div class="or"><span>or</span></div><JamComposer roomId={channel.id} /></div></div>
  {:else}
  {#if isDm && call.channel?.id !== channel.id && call.ids(channel.id).length}
    <button class="call-banner" onclick={() => call.join(channel)}><Icon name="phone" /> {call.ids(channel.id).map((id) => store.name(id)).join(', ')} {call.ids(channel.id).length === 1 ? 'is' : 'are'} in a call · Join</button>
  {/if}
  <div class="content" bind:clientWidth={space}>
    <!-- Files belong to the pane they were dropped on. The room owns this one;
         the panel owns its own, and a stowed room is not a drop target. -->
    <div
      class="room" class:stowed inert={stowed || undefined}
      role="group" aria-label={`Main conversation in ${store.title(channel)}`}
      ondragenter={(e) => { e.preventDefault(); e.stopPropagation(); dragging++ }}
      ondragleave={() => dragging--}
      ondragover={(e) => { e.preventDefault(); e.stopPropagation() }}
      ondrop={onDrop}
    >
      <ThreadStrip {channel} selected={openThread} onopen={(id, root) => open(root, id)} />
      {#if roomError}
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
      <MessageList {channel} source={roomSource} onreply={(m) => open(m.id, m.thread?.id, m.id)} />

      <div class="typing" aria-live="polite">
        {#if listening}<span class="mono">listening</span>{:else if typing.length}{typing.join(', ')} {typing.length === 1 ? 'is' : 'are'} typing{/if}
      </div>

      <Composer {channel} bind:replyTo bind:dropped bind:listening />
      {#if dragging > 0}
        <div class="drop">
          <div class="drop-inner">
            <Icon name="clip" size={28} />
            <span class="display big">Drop it in #{channel.name}</span>
            <span class="muted">Anything up to 1 GB. Clips play right in the chat.</span>
          </div>
        </div>
      {/if}
    </div>
    {#if panel && rootOf}
      <ThreadPanel
        {channel} rootId={rootOf} threadId={openThread} target={target} narrow={!pair}
        onclose={closePanel}
        onjump={() => router.go(openThread ? `/c/${channel.id}/t/${openThread}` : `/c/${channel.id}`, true)}
        onvalid={(ok) => (panelValid = ok)}
      />
    {:else if panel && loadingThread}
      <section class="panel-missing" aria-label="Conversation" aria-busy="true">
        <p class="muted">Opening that conversation…</p>
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
  .windowed { display: flex; align-items: center; gap: 10px; margin: 0; padding: 6px var(--gutter); border-bottom: 1px solid var(--line); font-size: 12px; }
  .panel-missing { flex: 1; display: grid; place-content: center; gap: 10px; justify-items: center; padding: 24px; text-align: center; }
  .panel-missing .row { display: flex; gap: 8px; }
  .object-slot { display: contents; }
  .object-slot.hidden { display: none; }
  .view { flex: 1; min-height: 0; display: flex; flex-direction: column; position: relative; }
  .head {
    display: flex; align-items: center; gap: 10px; padding: 10px var(--gutter); min-height: 52px;
    border-bottom: 1px solid var(--line);
  }
  .kind { color: var(--ink-3); display: grid; }
  h1 { font-size: 19px; margin: 0; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .spacer { flex: 1; }
  .iconbtn { padding: 6px; }
  .search { display: flex; align-items: center; gap: 6px; padding: 4px 10px; border: 1px solid transparent; border-radius: 999px; color: var(--ink-3); background: var(--bg-2); }
  .search:focus-within { border-color: var(--accent); color: var(--ink-2); box-shadow: 0 0 0 3px var(--accent-glow); }
  .search input { background: none; border: 0; outline: 0; width: 120px; font-size: 13px; color: var(--ink); transition: width 0.15s; }
  .search input:focus { width: 200px; }
  .search input::placeholder { color: var(--ink-3); }
  .call-banner { display: flex; align-items: center; gap: 8px; padding: 10px var(--gutter); color: var(--lamp); background: var(--bg-3); border-bottom: 1px solid var(--line); text-align: left; }
  .voice-music { width: min(360px, calc(100% - 32px)); }
  .voice-music h2 { font-size: 22px; margin: 20px 0 4px; }
  .voice-music p { color: var(--ink-2); margin: 0 0 18px; font-size: 13px; }
  .or { display: flex; align-items: center; gap: 10px; margin: 18px 0; color: var(--ink-3); font-size: 11px; text-transform: uppercase; letter-spacing: 0.08em; }
  .or::before, .or::after { content: ''; flex: 1; height: 1px; background: var(--line); }
  .voice-empty { flex: 1; display: grid; place-items: center; }
  @media (max-width: 600px) { .head { gap: 6px; padding-inline: 10px; } .search input { width: 64px; } .search input:focus { width: 90px; } .dm-call { padding: 6px; } .dm-call span { display: none; } }
  .typing { height: 18px; padding: 0 20px; font-size: 12px; color: var(--ink-3); }
  .drop {
    position: absolute; inset: 8px; z-index: 5; border-radius: var(--r-lg);
    background: var(--overlay); border: 2px dashed var(--lamp);
    display: grid; place-items: center; pointer-events: none;
  }
  .drop-inner { display: grid; justify-items: center; gap: 6px; color: var(--lamp); }
  .big { font-size: 26px; }
</style>
