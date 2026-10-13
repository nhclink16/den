<script lang="ts">
  import { objectKind } from '../plugins'
  import { instances, store } from '../lib/store.svelte'
  import { goToMessage } from '../lib/notify.svelte'
  import type { Message } from '../lib/types'
  import { render, emojiOnly, plainMentions } from '../lib/markdown'
  import { shortTime, clockTime } from '../lib/time'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import Attachment from './Attachment.svelte'
  import InlineConfirm from './InlineConfirm.svelte'
  import EmojiPicker from './EmojiPicker.svelte'
  import { profileCard } from '../lib/people.svelte'
  import { klipyGif, snippet } from '../lib/klipy'

  // `prefix` scopes the DOM id. A thread panel shows the same root message the
  // room does, so an unscoped id would exist twice and a deep link could reveal
  // whichever copy the browser found first.
  let { m, compact, prefix = 'm', onreply, onmediaready }: { m: Message; compact: boolean; prefix?: string; onreply: (m: Message) => void; onmediaready?: () => void } = $props()
  // The badge reads the metadata OWNER, not the copy embedded in this message.
  // A cached root predates its first reply, so its embedded summary is absent
  // then and stale after a rename or a Resolve.
  const conversation = $derived(
    m.thread ? store.thread(m.thread.id) ?? m.thread : store.threadForRoot(m.id),
  )
  const unread = $derived(conversation ? store.threadUnread(conversation.id) : undefined)
  let editing = $state(false)
  let draft = $state('')
  const mine = $derived(m.author_id === store.me?.id)
  const canDelete = $derived(mine || store.me?.role === 'admin')
  const author = $derived(store.user(m.author_id))
  const html = $derived(render(m.content, store.users))
  // A message that is only a GIF link from KLIPY shows the GIF itself.
  const gif = $derived(klipyGif(m.content))
  let parent = $state<Message | undefined>()
  // Every reply in a thread answers its root, which sits at the top of the panel.
  // A first reply, an agent or the CLI still sends reply_to=root: no quote for that.
  const quotesRoot = $derived(!!m.thread_id && store.thread(m.thread_id)?.root_message_id === m.reply_to)
  // Decoration only: a lookup that fails leaves the reference undecorated. It
  // is never a cancellation, and it never touches a draft's quote.
  $effect(() => {
    if (!m.reply_to || quotesRoot) { parent = undefined; return }
    const want = m.reply_to
    void store.fetchMessage(want, m.channel_id)
      .then((p) => { if (m.reply_to === want) parent = p })
      .catch(() => { if (m.reply_to === want) parent = undefined })
  })
  const mentionsMe = $derived(!!store.me && (m.mention_ids || []).includes(store.me.id))
  const QUICK = ['👍', '😂', '❤️', '🔥', '👀', '💀']
  // A message that is nothing but a few emoji is a reaction said out loud: draw it big.
  const jumbo = $derived(!m.attachments?.length && emojiOnly(m.content))
  // Only messages that land while you watch animate in; history loads still.
  // Read once on purpose: an edit must not replay the arrival.
  // svelte-ignore state_referenced_locally
  const fresh = Date.now() - new Date(m.created_at).getTime() < 8000

  // Touch screens have no hover. A tap on the message, or a long press anywhere on
  // it, shows its actions (plus Copy, since the text can't be selected there) until
  // the next tap elsewhere. A scroll is neither: the browser cancels the pointer.
  let held = $state(false)
  let article = $state<HTMLElement>()
  let touch = false
  let press: ReturnType<typeof setTimeout> | undefined
  let pressAt = { x: 0, y: 0 }
  let pressed = false
  // The emoji picker is a popover inside the message; it handles its own taps.
  const own = (e: Event) => !(e.target as Element).closest('dialog, [popover]')
  // A tap that closes a popup (a profile card) is spent on closing it.
  let dismissing = false
  function down(e: PointerEvent) {
    touch = e.pointerType === 'touch'
    if (!touch || !e.isPrimary || !own(e)) return
    dismissing = !!document.querySelector(':popover-open')
    pressed = false
    pressAt = { x: e.clientX, y: e.clientY }
    press = setTimeout(() => { press = undefined; pressed = true; held = true }, 450)
  }
  function move(e: PointerEvent) { if (press && Math.hypot(e.clientX - pressAt.x, e.clientY - pressAt.y) > 10) lift() }
  function lift() { clearTimeout(press); press = undefined }
  // A long press is not also a tap on whatever was under the finger.
  function swallow(e: MouseEvent) { if (pressed) { pressed = false; e.preventDefault(); e.stopPropagation() } }
  function tap(e: MouseEvent) {
    if (!touch || dismissing || !own(e) || (e.target as Element).closest('a, button, video, audio, textarea, .mention')) return
    held = !held
  }
  $effect(() => {
    if (!held) return
    // The bar sits above the message, so on a message at the top edge it would be cut off.
    article?.querySelector('.tools')?.scrollIntoView({ block: 'nearest' })
    const away = (e: PointerEvent) => { if (!article?.contains(e.target as Node)) held = false }
    document.addEventListener('pointerdown', away, true)
    return () => document.removeEventListener('pointerdown', away, true)
  })
  function done() { held = false }

  function startEdit() { draft = m.content; editing = true }
  async function saveEdit(e: KeyboardEvent) {
    if (e.key === 'Escape') { editing = false; return }
    if (e.key !== 'Enter' || e.shiftKey) return
    e.preventDefault()
    const text = draft.trim()
    if (text && text !== m.content) await store.edit(m.id, text)
    editing = false
  }
  // Delete asks first, in the message itself, like every other delete in Den.
  let deleting = $state(false)
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions (touch only; keyboards reach the actions by focus) -->
<article class="msg" data-hover-animates class:compact class:me={mentionsMe} class:fresh class:replying={!!parent} class:held id="{prefix}-{m.id}" bind:this={article}
  onpointerdown={down} onpointermove={move} onpointerup={lift} onpointercancel={lift}
  onclickcapture={swallow} onclick={tap} oncontextmenu={(e) => { if (touch && own(e)) e.preventDefault() }}>
  {#if parent}
    <!-- The quoted message may be outside the loaded page, or in another
         conversation entirely; the shared resolver finds it either way. -->
    <!-- The concrete owner, not the proxy: goToMessage checks that the source is
         a real instance, and the proxy is not one. -->
    <button class="reply-ref" onclick={() => goToMessage(instances.active, parent!.channel_id, parent!.id)}>
      <Icon name="reply" size={12} />
      <span class="who">{store.name(parent.author_id)}</span>
      <span class="snippet">{plainMentions(snippet(parent.content), store.users).slice(0, 90) || 'sent a file'}</span>
    </button>
  {/if}
  <div class="row">
    <div class="gutter">
      {#if compact}<span class="stamp" title={shortTime(m.created_at)}>{clockTime(m.created_at)}</span>{:else}<button class="face" aria-label={`${store.name(m.author_id)}'s profile`} onclick={(e) => profileCard.open(m.author_id, e.currentTarget, instances.active)}><Avatar userId={m.author_id} size={36} /></button>{/if}
    </div>
    <div class="body">
      {#if !compact}
        <div class="meta">
          <button class="author" onclick={(e) => profileCard.open(m.author_id, e.currentTarget, instances.active)}>{author?.display_name || author?.username || 'someone'}</button>
          {#if author?.bot}<span class="bot"><Icon name="bot" size={11} />Agent</span>{/if}
          <span class="time">{shortTime(m.created_at)}</span>
        </div>
      {/if}
      {#if editing}
        <textarea class="field edit" bind:value={draft} onkeydown={saveEdit} rows="2"></textarea>
        <div class="faint hint">Enter to save · Esc to cancel</div>
      {:else if gif}
        <a class="gif" href={gif} target="_blank" rel="noopener noreferrer"><img src={gif} alt="GIF" loading="lazy" onload={onmediaready} /></a>
      {:else}
        <div class="text" class:jumbo>{@html html}{#if m.edited_at}<span class="edited" title={m.edited_at}> (edited)</span>{/if}</div>
      {/if}
      {#each m.objects || [] as object (object.id)}
        {@const Card = objectKind(object.kind)?.card}
        {#if Card}<Card {object} />{/if}
      {/each}
      {#if m.attachments?.length}
        <div class="files">{#each m.attachments as a (a.id)}<Attachment upload={a} author={store.name(m.author_id)} {onmediaready} />{/each}</div>
      {/if}
      {#if conversation && prefix === 'm'}
        {@const last = conversation.last_reply}
        <!-- The newest reply, not the parent's text again: the parent is right above. -->
        <a
          class="thread-link"
          class:lit={!!unread?.count}
          class:resolved={!!conversation.resolved_at}
          href="/c/{m.channel_id}/t/{conversation.id}"
          title={last ? shortTime(last.created_at) : undefined}
          onclick={(e) => { if (!e.metaKey && !e.ctrlKey && !e.shiftKey && e.button === 0) { e.preventDefault(); onreply(m) } }}
        >
          {#if last}
            <Avatar userId={last.author_id} size={18} presence={false} />
            <span class="who">{store.name(last.author_id)}</span>
            <!-- An empty reply carries a file or a card; the preview cannot tell which. -->
            <span class="said">{plainMentions(snippet(last.excerpt), store.users).trim() || 'shared something'}</span>
          {:else}
            <Icon name="reply" size={12} />
          {/if}
          <span class="replies faint">{last ? '· ' : ''}{conversation.reply_count} {conversation.reply_count === 1 ? 'reply' : 'replies'}</span>
          {#if unread?.count}<span class="count" class:at={unread.mention}>{unread.mention ? '@' : unread.count}</span>{/if}
          {#if conversation.resolved_at}<span class="faint state">resolved</span>{/if}
        </a>
      {/if}
      {#if deleting}
        <div class="deleting"><InlineConfirm open action="Delete" sentence="Delete this message? This can't be undone." confirm={() => store.remove(m.id, m.channel_id)} oncancel={() => (deleting = false)} /></div>
      {/if}
      {#if m.reactions?.length}
        <div class="reactions">
          {#each m.reactions as r (r.emoji)}
            <button class="pill rx" aria-pressed={!!store.me && r.user_ids.includes(store.me.id)} onclick={() => store.react(m, r.emoji)} title={r.user_ids.map((id) => store.name(id)).join(', ')}>
              <span>{r.emoji}</span><span class="n">{r.user_ids.length}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
    <div class="tools">
      <EmojiPicker label="React to this message" quick={QUICK} align="end" onpick={(e) => { void store.react(m, e); done() }}><span class="tool" aria-hidden="true">😶</span></EmojiPicker>
      <button title="Reply" aria-label="Reply" onclick={() => { done(); onreply(m) }}><Icon name="reply" /></button>
      {#if held && m.content}<button title="Copy text" aria-label="Copy text" onclick={() => { void navigator.clipboard?.writeText(m.content); done() }}><Icon name="copy" /></button>{/if}
      {#if mine}<button title="Edit" aria-label="Edit message" onclick={() => { done(); startEdit() }}><Icon name="edit" /></button>{/if}
      {#if canDelete}<button title="Delete" aria-label="Delete message" class="danger" onclick={() => { done(); deleting = true }}><Icon name="trash" /></button>{/if}
    </div>
  </div>
</article>

<style>
  /* One line under the parent: who answered last, what they said, how many replies.
     The reply text is what gives way when the line runs out of room. */
  .thread-link {
    display: inline-flex; align-items: center; gap: 6px; margin-top: 6px; padding: 4px 10px 4px 6px;
    border: 1px solid var(--line); border-radius: var(--r-pill, 999px); color: var(--ink-2); font-size: var(--fs-sm); max-width: 100%;
    white-space: nowrap;
  }
  .thread-link:hover { background: var(--hover); color: var(--ink); text-decoration: none; }
  .thread-link.lit { border-color: var(--lamp); color: var(--ink); }
  .thread-link.resolved { opacity: .6; }
  .thread-link.resolved:hover { opacity: 1; }
  /* The text starts from nothing and takes what is left, so a name only shrinks
     once the text is gone (a long name on a narrow phone). */
  .thread-link .who { font-weight: 700; color: var(--ink); max-width: 8em; }
  .thread-link .said { flex: 1 1 0; }
  .thread-link .who, .thread-link .said { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .thread-link .replies, .thread-link .state, .thread-link .count { flex: none; }
  .thread-link .count {
    min-width: 18px; padding: 0 5px; border-radius: var(--r-pill, 999px); background: var(--lamp); color: var(--on-lamp, #000); font-size: var(--fs-xs); text-align: center;
  }
  .msg { position: relative; padding: calc(2px * var(--density)) var(--gutter); margin-top: calc(14px * var(--density)); transition: background-color var(--t-fast); }
  .msg.compact { margin-top: 0; }
  /* Hover only where there is one: iOS spends a tap on any :hover that reveals
     something, so the avatar or a picture needed two taps, and the hover stuck
     to wherever a scroll ended. */
  @media (hover: hover) {
    .msg:hover { background: var(--hover); }
    .msg.me:hover { background: linear-gradient(90deg, color-mix(in srgb, var(--lamp) 24%, transparent), color-mix(in srgb, var(--lamp) 8%, transparent) 70%); }
    .msg:hover .stamp { opacity: 1; }
    .msg:hover .tools { opacity: 1; pointer-events: auto; transform: none; }
    .face:hover { transform: translateY(-1px); }
  }
  .msg.held { background: var(--hover); }
  .msg.held .stamp { opacity: 1; }
  /* On touch, a long press offers the message's actions, not a text selection or the link menu. */
  @media (hover: none) {
    .msg { -webkit-touch-callout: none; }
    .meta, .text, .reply-ref { -webkit-user-select: none; user-select: none; }
  }
  /* A mention is lit from the left edge and fades out, like light through a doorway. */
  .msg.me { box-shadow: inset 3px 0 0 var(--lamp); background: linear-gradient(90deg, var(--lamp-glow), color-mix(in srgb, var(--lamp) 5%, transparent) 70%); }
  @media (prefers-reduced-motion: no-preference) {
    .msg.fresh { animation: arrive .32s var(--ease-out) both; }
  }
  @keyframes arrive { from { opacity: 0; transform: translateY(6px); } }
  .row { display: flex; gap: 12px; }
  .gutter { width: 36px; flex: none; display: flex; justify-content: center; align-items: flex-start; padding-top: 2px; }
  .msg.compact .gutter { height: 22px; }
  .stamp { font-size: var(--fs-xs); line-height: 22px; white-space: nowrap; color: var(--ink-2); opacity: 0; transition: opacity var(--t-fast); }
  .body { flex: 1; min-width: 0; }
  .meta { display: flex; align-items: baseline; gap: 8px; margin-bottom: 1px; }
  /* A very long name stops at one line; the time and agent badge stay whole. */
  .author { font-weight: 700; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bot, .time { flex: none; }
  .author:hover { text-decoration: underline; text-underline-offset: 3px; }
  .face { display: grid; border-radius: var(--avatar-r, 35%); transition: transform var(--t) var(--ease-out); }
  .face:active { transform: scale(.96); }
  .bot {
    display: inline-flex; align-items: center; gap: 3px; align-self: center; padding: 1px 6px 1px 5px;
    border-radius: var(--r-pill, 999px); font: 600 var(--fs-xs)/1.45 var(--body);
    color: var(--lamp); background: var(--lamp-glow);
  }
  .time { font-size: var(--fs-xs); color: var(--ink-3); }
  /* Prose ran 120-260 characters per line on a wide window. Attachments,
     code blocks and embeds stay full width. */
  .text { line-height: var(--leading); overflow-wrap: anywhere; max-width: 68ch; }
  .text :global(pre) { max-width: none; }
  .text.jumbo { font-size: 2.4em; line-height: 1.2; letter-spacing: .04em; }
  .edited { color: var(--ink-3); font-size: var(--fs-xs); }
  .edit { resize: vertical; margin-top: 2px; }
  .hint { font-size: var(--fs-xs); margin-top: 2px; }
  .gif { display: inline-block; margin-top: 4px; border-radius: var(--r); overflow: hidden; line-height: 0; background: var(--bg-3); }
  .gif img { display: block; max-width: min(320px, 100%); max-height: 260px; min-width: 80px; min-height: 60px; object-fit: contain; }
  .files { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 6px; }
  .reply-ref {
    position: relative; display: flex; text-align: left; max-width: calc(100% - 48px); align-items: center; gap: 6px; margin: 0 0 2px 48px;
    font-size: var(--fs-sm); color: var(--ink-3); white-space: nowrap; min-width: 0;
  }
  /* A spine from the avatar up to the quoted line ties the reply to its parent. */
  .reply-ref::before {
    content: ''; position: absolute; left: -30px; top: 50%; width: 24px; height: 10px;
    border: 2px solid var(--line-strong); border-right: 0; border-bottom: 0; border-top-left-radius: 7px;
  }
  .reply-ref :global(svg) { display: none; }
  .reply-ref:hover .who, .reply-ref:hover .snippet { color: var(--ink); }
  .reply-ref .who { color: var(--ink-2); font-weight: 700; }
  .reply-ref .snippet { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .tools {
    opacity: 0; pointer-events: none; transform: translateY(3px); transition: opacity var(--t-fast), transform var(--t) var(--ease-out);
    position: absolute; right: var(--gutter); top: -14px; display: flex; gap: 2px; z-index: 2;
    background: var(--bg-2); border: 1px solid var(--line-strong); border-radius: var(--r); padding: 2px;
    box-shadow: var(--lift);
  }
  .msg.held .tools, .msg:focus-within .tools, .tools:has(:global(details[open])) { opacity: 1; pointer-events: auto; transform: none; }
  .tools button, .tool { padding: 6px; border-radius: var(--r); color: var(--ink-2); display: grid; transition: background-color var(--t-fast), color var(--t-fast); }
  .tools button:hover, .tool:hover { background: var(--bg-3); color: var(--ink); }
  .tool { line-height: 20px; }
  .tools button.danger:hover { color: var(--ember); }
  .reactions { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 5px; }
  /* A pill, kept as short as the old chip so a row of reactions stays compact. */
  .rx { min-height: 26px; gap: 5px; padding: 2px 9px 2px 7px; font-size: var(--fs-body); transition: background-color var(--t-fast), border-color var(--t-fast), transform var(--t) var(--ease-out); }
  .rx:hover { transform: translateY(-1px); }
  .rx:active { transform: scale(.94); }
  .rx .n { font-size: var(--fs-xs); color: var(--ink-2); font-variant-numeric: tabular-nums; }
  .rx[aria-pressed=true] .n { color: var(--lamp); }
  /* The question and its two answers on one line, under the text they are about. */
  .deleting { margin-top: 6px; }
  .deleting :global(.confirmation) { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 12px; }
</style>
