<script lang="ts">
  import { tick } from 'svelte'
  import { store } from '../lib/store.svelte'
  import { conversationKey, type Conversation } from '../lib/conversation'
  import type { Channel, Message } from '../lib/types'
  import Icon from './Icon.svelte'
  import MessageItem from './MessageItem.svelte'
  import MessageList, { type Source } from './MessageList.svelte'
  import Composer from './Composer.svelte'

  // One conversation. Its identity is channel plus ROOT message, which exists
  // before the server has a thread for it: replying opens this panel immediately
  // and the thread is created by the first reply that is actually sent.
  let {
    channel, rootId, threadId, target, narrow, onclose, onjump, onvalid,
  }: {
    channel: Channel
    rootId: string
    threadId?: string
    target?: string
    narrow: boolean
    onclose: () => void
    onjump: () => void
    /// True only once a valid root for THIS room is actually mounted.
    onvalid?: (ok: boolean) => void
  } = $props()

  const here = $derived<Conversation>({ channelId: channel.id, rootId })
  const summary = $derived(threadId ? store.thread(threadId) : store.threadForRoot(rootId))
  const live = $derived(summary?.id ?? threadId)
  const resolved = $derived(!!summary?.resolved_at)
  const position = $derived(live ? store.threadState(live) : undefined)
  const replies = $derived(live ? store.threadMessages.get(live) || [] : [])
  const typing = $derived(store.typingNames(channel.id, live))

  // A CACHE HIT is validated too. A root legitimately fetched in room A must not
  // render under /c/B?reply=thatRoot, and a reply is never an unsaved root.
  const cached = $derived(store.root(rootId))
  const root = $derived(
    cached && cached.channel_id === channel.id && !cached.thread_id ? cached : undefined,
  )
  const rootWrongRoom = $derived(!!cached && (cached.channel_id !== channel.id || !!cached.thread_id))
  let replyTo = $state<Message | null>(null)
  let dropped = $state<File[]>([])
  let listening = $state(false)
  let dragging = $state(0)
  let renaming = $state(false)
  let title = $state('')
  let error = $state('')
  let busy = $state(false)
  let head: HTMLElement

  // The root itself, which stays in the room feed and is also shown here. It is
  // cached in the Store, so edits and reactions reach this copy too, and it is
  // validated against THIS channel: a root from another room must never render
  // under this URL.
  let rootError = $state('')
  let rootRetry = $state(false)
  function loadRootFor(id: string, chan: string) {
    rootError = ''
    rootRetry = false
    void store.loadRoot(id, chan)
      .then((m) => {
        if (rootId !== id) return
        if (!m) rootError = 'That message is not in this room.'
      })
      .catch((e) => {
        if (rootId !== id) return
        // Transient, so it offers a way to try again rather than declaring the
        // conversation unavailable.
        rootError = (e as Error).message
        rootRetry = true
      })
  }
  $effect(() => {
    const id = rootId, chan = channel.id
    rootError = rootWrongRoom ? 'That message is not in this room.' : ''
    rootRetry = false
    if (cached || rootWrongRoom) return
    loadRootFor(id, chan)
  })
  // Published so the view can refuse to treat an unusable panel as displayed.
  $effect(() => { onvalid?.(!!root) })
  // Replies, once a thread exists. A rejected first page is shown and retryable
  // rather than leaving an empty conversation that looks like it has none.
  let repliesError = $state('')
  function loadReplies(id: string) {
    repliesError = ''
    void store.loadThreadMessages(id).catch((e) => { if (live === id) repliesError = (e as Error).message })
  }
  $effect(() => {
    const id = live
    if (id && !store.threadMessages.has(id)) loadReplies(id)
  })

  // Closing a window is what cancels its in-flight requests. Without this the
  // cancellation exists only for a confirmed thread removal, and navigating away
  // or jumping to the newest message leaves its pages running.
  $effect(() => {
    const key = windowKey
    if (!key) return
    return () => store.closeWindow(key)
  })
  // Metadata and this account's position, when only an event has been seen.
  $effect(() => {
    const id = live
    if (id && !store.thread(id)) void store.loadThread(id).catch(() => {})
  })
  // Focus the panel when it opens so the keyboard lands somewhere sensible.
  $effect(() => { void conversationKey(here); void tick().then(() => head?.focus()) })

  // An old target gets a contiguous window, owned by the Store so every update
  // reaches it, kept apart from the latest cache so a gap cannot look like read
  // history, and paged outward on demand.
  const inTail = $derived(!!target && (store.threadMessages.get(live ?? '') || []).some((m) => m.id === target))
  const windowKey = $derived(target && live && !inTail ? store.windowKey(channel.id, live, target) : '')
  $effect(() => {
    const key = windowKey, id = live, want = target
    if (!key || !id || !want) return
    void store.openWindow(channel.id, id, want)
  })
  const windowed = $derived(windowKey ? store.windowAt(windowKey) : undefined)
  const windowStatus = $derived(windowKey ? store.windowStatus(windowKey) : undefined)
  const showingWindow = $derived(!!windowed?.length)

  const source = $derived<Source>({
    list: showingWindow ? windowed! : replies,
    identityExtra: windowKey,
    lastRead: position?.last_read_id ?? rootId,
    // A window is a slice, not the start of the conversation.
    exhausted: showingWindow ? !!windowStatus?.start : !live || store.threadExhausted.has(live),
    loadingOlder: showingWindow ? !!windowStatus?.loading : !!live && store.loadingOlder.has(live),
    older: async () => {
      if (showingWindow && windowKey && live) await store.extendWindow(windowKey, channel.id, live, true)
      else if (live) await store.loadOlderThreadReplies(live)
    },
    // A window is not the tail, so reading it must not acknowledge the tail.
    onread: (displayed) => {
      if (live && !showingWindow && visible()) void store.markThreadRead(live, displayed)
    },
    target,
    prefix: 't',
    watermark: rootId,
    identity: `t:${rootId}`,
  })

  // Only a conversation someone is actually looking at gets acknowledged.
  const visible = () => document.visibilityState === 'visible' && store.active

  async function act(work: () => Promise<unknown>) {
    busy = true; error = ''
    try { await work() } catch (e) { error = (e as Error).message } finally { busy = false }
  }
  const setResolved = (v: boolean) => act(async () => { if (live) await store.updateThread(live, { resolved: v }) })
  const setFollowing = (v: boolean) => act(async () => { if (live) await store.followThread(live, v) })
  async function rename() {
    const next = title.trim()
    if (!live || !next) { renaming = false; return }
    await act(async () => { await store.updateThread(live, { title: next }); renaming = false })
  }
</script>

<!-- The panel owns its own drops: a file meant for this conversation must not
     end up staged in the room behind it. -->
<section
  class="panel" data-thread-panel aria-label={summary ? `Conversation: ${summary.title}` : 'New conversation'}
  ondragenter={(e) => { e.preventDefault(); e.stopPropagation(); dragging++ }}
  ondragleave={() => dragging--}
  ondragover={(e) => { e.preventDefault(); e.stopPropagation() }}
  ondrop={(e) => {
    e.preventDefault(); e.stopPropagation(); dragging = 0
    if (resolved) return
    const files = [...(e.dataTransfer?.files || [])]
    if (files.length) dropped = files
  }}
>
  <header class="head" bind:this={head} tabindex="-1">
    <button class="btn quiet iconbtn" onclick={onclose} aria-label={narrow ? 'Back to room' : 'Close conversation'}>
      {#if narrow}<span>Back to room</span>{:else}<Icon name="x" size={16} />{/if}
    </button>
    {#if renaming}
      <input
        class="field rename" bind:value={title} aria-label="Conversation title"
        onkeydown={(e) => { if (e.key === 'Enter') rename(); if (e.key === 'Escape') renaming = false }}
      />
      <button class="btn quiet" onclick={rename} disabled={busy}>Save</button>
    {:else}
      <h2 class="display">{summary?.title ?? root?.content.split('\n')[0]?.slice(0, 80) ?? 'New conversation'}</h2>
      {#if resolved}<span class="tag mono">Resolved</span>{/if}
    {/if}
    <span class="spacer"></span>
    {#if live && !renaming}
      <button class="btn quiet" onclick={() => { title = summary?.title ?? ''; renaming = true }} disabled={busy}>Rename</button>
      <button class="btn quiet" onclick={() => setFollowing(!position?.following)} disabled={busy}>
        {position?.following ? 'Unfollow' : 'Follow'}
      </button>
      <button class="btn quiet" onclick={() => setResolved(!resolved)} disabled={busy}>
        {resolved ? 'Reopen' : 'Resolve'}
      </button>
    {/if}
  </header>

  {#if error}<p class="err" role="alert">{error}</p>{/if}

  <div class="root">
    {#if root}
      <MessageItem m={root} compact={false} prefix="t" onreply={(m) => (replyTo = m)} />
    {:else if rootError}
      <p class="err" role="alert">
        {rootError}
        {#if rootRetry}<button class="btn quiet" onclick={() => loadRootFor(rootId, channel.id)}>Retry</button>{/if}
      </p>
    {:else}
      <p class="faint" aria-busy="true">Loading the message this conversation is about…</p>
    {/if}
  </div>

  {#if windowKey && windowStatus?.error}
    <p class="windowed" role="alert">
      {windowStatus.error}
      <!-- Resumes the failed edge and direction; openWindow returns at once for
           a window that already exists, so it could never retry an extension. -->
      <button class="btn quiet" onclick={() => store.retryWindow(windowKey, channel.id, live)}>Retry</button>
      <button class="btn quiet" onclick={onjump}>Jump to newest</button>
    </p>
  {:else if windowKey && windowStatus?.loading && !showingWindow}
    <p class="windowed faint" aria-busy="true">Finding that message…</p>
  {:else if showingWindow}
    <p class="windowed faint">
      Showing an older part of this conversation.
      {#if !windowStatus?.end}
        <button class="btn quiet" onclick={() => store.extendWindow(windowKey, channel.id, live, false)} disabled={windowStatus?.loading}>
          {windowStatus?.loading ? 'Loading…' : 'Newer'}
        </button>
      {/if}
      <button class="btn quiet" onclick={onjump}>Jump to newest</button>
    </p>
  {/if}
  {#if repliesError}
    <p class="err" role="alert">
      {repliesError}
      <button class="btn quiet" onclick={() => live && loadReplies(live)}>Retry</button>
    </p>
  {/if}
  {#if live}
    <MessageList {channel} {source} onreply={(m) => (replyTo = m)} />
  {:else}
    <div class="empty faint">No replies yet. The first one starts this conversation.</div>
  {/if}

  <div class="typing" aria-live="polite">
    {#if typing.length}{typing.join(', ')} {typing.length === 1 ? 'is' : 'are'} typing{/if}
  </div>

  <Composer
    {channel} conversation={here} bind:replyTo bind:dropped bind:listening
    locked={!root ? 'This conversation is not available here.'
      : resolved ? 'This conversation is resolved. Reopen it to add anything new.' : undefined}
  />
  {#if dragging > 0}
    <div class="drop">
      <div class="drop-inner">
        <Icon name="clip" size={28} />
        <span class="display big">Drop it in this conversation</span>
      </div>
    </div>
  {/if}
</section>

<style>
  .panel { display: flex; flex-direction: column; min-height: 0; min-width: 0; flex: 1; border-left: 1px solid var(--line); position: relative; }
  .drop { position: absolute; inset: 8px; z-index: 5; border-radius: var(--r-lg); background: var(--overlay); border: 2px dashed var(--lamp); display: grid; place-items: center; pointer-events: none; }
  .drop-inner { display: grid; justify-items: center; gap: 6px; color: var(--lamp); }
  .big { font-size: 22px; }
  /* Wraps, because it has to. At 200% zoom in a 700px window the viewport is
     350px and these controls need about 400px on their own — Resolve ended up
     past the right edge, unreachable, with the title already fully collapsed.
     Shrinking the title cannot fix that; only a second row can. At every width
     where they fit, this changes nothing. */
  .head { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 10px var(--gutter); min-height: 52px; border-bottom: 1px solid var(--line); }
  .head:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  h2 { font-size: 16px; margin: 0; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .spacer { flex: 1; }
  .iconbtn { display: inline-flex; align-items: center; gap: 6px; padding: 6px; }
  .tag { font-size: 11px; color: var(--ink-3); border: 1px solid var(--line); border-radius: var(--r-pill, 999px); padding: 1px 7px; }
  .rename { flex: 1; min-width: 0; }
  .err { margin: 8px var(--gutter) 0; color: var(--danger, var(--lamp)); font-size: 13px; }
  .root { border-bottom: 1px solid var(--line); padding-bottom: 6px; }
  .empty { flex: 1; display: grid; place-items: center; padding: 24px; text-align: center; font-size: 13px; }
  .typing { height: 18px; padding: 0 20px; font-size: 12px; color: var(--ink-3); }
  .windowed { display: flex; align-items: center; gap: 10px; margin: 0; padding: 6px var(--gutter); border-bottom: 1px solid var(--line); font-size: 12px; }
</style>
