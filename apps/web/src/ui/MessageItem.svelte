<script lang="ts">
  import { objectKind } from '../plugins'
  import { instances, store } from '../lib/store.svelte'
  import { goToMessage } from '../lib/notify.svelte'
  import type { Message } from '../lib/types'
  import { render, emojiOnly, plainMentions } from '../lib/markdown'
  import { shortTime } from '../lib/time'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import Attachment from './Attachment.svelte'
  import InlineConfirm from './InlineConfirm.svelte'
  import EmojiPicker from './EmojiPicker.svelte'
  import { profileCard } from '../lib/people.svelte'
  import { klipyGif, snippet } from '../lib/klipy'
  import { themes } from '../lib/theme.svelte'
  import { resolveAccent } from '../lib/theme-color'
  import { bubbleFill, nameInk, personColor, withContrast } from '../lib/bubbles'
  import { haptic } from '../lib/haptics'

  // `prefix` scopes the DOM id. A thread panel shows the same root message the
  // room does, so an unscoped id would exist twice and a deep link could reveal
  // whichever copy the browser found first.
  // `head` starts one person's run of messages (their name goes above it) and
  // `tail` ends it (their picture goes beside it).
  let { m, head = true, tail = true, prefix = 'm', onreply, onmediaready }: { m: Message; head?: boolean; tail?: boolean; prefix?: string; onreply: (m: Message) => void; onmediaready?: () => void } = $props()
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
  // A plain feed with names, for people who turned bubbles off. Otherwise your
  // own messages sit on the right, everyone else's on the left.
  const feed = $derived(themes.appearance.bubble_shape === 'none')
  const right = $derived(mine && !feed)
  const text = $derived(!gif && !!m.content.trim())
  const code = $derived(m.content.includes('```'))
  const pieces = $derived(editing || !!gif || text || !!m.attachments?.length || !!m.reactions?.length)
  // The bubble's colour, worked out against the theme so text on it stays readable.
  // With no bubbles, the colour goes on the name instead.
  const fill = $derived.by(() => {
    const half = themes.half, c = themes.active[half], person = personColor(m.author_id, author?.accent)
    if (feed) return head ? { name: nameInk(themes.appearance.bubble_colors, mine, person, c) } : {}
    return { bubble: bubbleFill(themes.appearance.bubble_colors, mine, mentionsMe, person, resolveAccent(c, themes.appearance.accent, half), withContrast(c, themes.appearance.contrast)) }
  })
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
    press = setTimeout(() => { press = undefined; pressed = true; held = true; haptic('medium') }, 450)
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
  let deletingBox = $state<HTMLElement>(), deleteButton = $state<HTMLButtonElement>()
  // A keyboard user who says Keep goes back to the Delete button they came from.
  function keep() {
    const keyboard = !!deletingBox?.querySelector(':focus-visible')
    deleting = false
    if (keyboard) requestAnimationFrame(() => deleteButton?.focus())
  }
</script>

{#snippet replyRef(p: Message)}
  <!-- The quoted message may be outside the loaded page, or in another
       conversation entirely; the shared resolver finds it either way. -->
  <!-- The concrete owner, not the proxy: goToMessage checks that the source is
       a real instance, and the proxy is not one. -->
  <button class="reply-ref" onclick={() => goToMessage(instances.active, p.channel_id, p.id)}>
    <Icon name="reply" size={12} />
    <span class="who">{store.name(p.author_id)}</span>
    <span class="snippet">{plainMentions(snippet(p.content), store.users).slice(0, 90) || 'sent a file'}</span>
  </button>
{/snippet}
{#snippet face(size: number)}
  <button class="face" aria-label={`${store.name(m.author_id)}'s profile`} onclick={(e) => profileCard.open(m.author_id, e.currentTarget, instances.active)}><Avatar userId={m.author_id} {size} /></button>
{/snippet}

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions (touch only; keyboards reach the actions by focus) -->
<article class="msg" data-hover-animates class:feed class:right class:head class:tail class:me={mentionsMe} class:fresh class:replying={!!parent} class:held id="{prefix}-{m.id}" bind:this={article}
  style:--fill={fill.bubble} style:--name={fill.name}
  onpointerdown={down} onpointermove={move} onpointerup={lift} onpointercancel={lift}
  onclickcapture={swallow} onclick={tap} oncontextmenu={(e) => { if (touch && own(e)) e.preventDefault() }}>
  {#if parent && feed}{@render replyRef(parent)}{/if}
  <div class="row">
    {#if !right}
      <div class="gutter">{#if feed && head}{@render face(36)}{/if}</div>
    {/if}
    <div class="body">
      {#if head && !right}
        <div class="meta">
          <button class="author" onclick={(e) => profileCard.open(m.author_id, e.currentTarget, instances.active)}>{author?.display_name || author?.username || 'someone'}</button>
          {#if author?.bot}<span class="bot"><Icon name="bot" size={11} />Agent</span>{/if}
        </div>
      {/if}
      <!-- Your own messages show no name; a screen reader still hears who spoke. -->
      {#if head && right}<span class="sr-only">{author?.display_name || author?.username || 'You'}</span>{/if}
      {#if parent && !feed}{@render replyRef(parent)}{/if}
      <div class="content">
        {#if tail && !right && !feed && pieces}<span class="tail-face">{@render face(28)}</span>{/if}
        {#if editing}
          <div class="editing">
            <textarea class="field edit" bind:value={draft} onkeydown={saveEdit} rows="2"></textarea>
            <div class="faint hint">Enter to save · Esc to cancel</div>
          </div>
        {:else if gif}
          <a class="gif" href={gif} target="_blank" rel="noopener noreferrer"><img src={gif} alt="GIF" loading="lazy" onload={onmediaready} /></a>
        {:else if text}
          <div class="bubble text" class:jumbo class:code>{@html html}{#if m.edited_at}<span class="edited" title={shortTime(m.edited_at)}>edited</span>{/if}</div>
        {/if}
        {#if m.attachments?.length}
          <div class="files">{#each m.attachments as a (a.id)}<Attachment upload={a} author={store.name(m.author_id)} {onmediaready} />{/each}</div>
        {/if}
        {#if m.reactions?.length}
          <div class="reactions">
            {#each m.reactions as r (r.emoji)}
              <button class="pill rx" aria-pressed={!!store.me && r.user_ids.includes(store.me.id)} onclick={() => store.react(m, r.emoji)} title={r.user_ids.map((id) => store.name(id)).join(', ')}>
                <span>{r.emoji}</span>{#if feed || r.user_ids.length > 1}<span class="n">{r.user_ids.length}</span>{/if}
              </button>
            {/each}
          </div>
        {/if}
        <div class="tools">
          <time class="when" datetime={m.created_at}>{shortTime(m.created_at)}</time>
          <EmojiPicker label="React to this message" quick={QUICK} align="end" onpick={(e) => { void store.react(m, e); done() }}><span class="tool" aria-hidden="true">😶</span></EmojiPicker>
          <button title="Reply" aria-label="Reply" onclick={() => { done(); onreply(m) }}><Icon name="reply" /></button>
          {#if held && m.content}<button title="Copy text" aria-label="Copy text" onclick={() => { void navigator.clipboard?.writeText(m.content); done() }}><Icon name="copy" /></button>{/if}
          {#if mine}<button title="Edit" aria-label="Edit message" onclick={() => { done(); startEdit() }}><Icon name="edit" /></button>{/if}
          {#if canDelete}<button bind:this={deleteButton} title="Delete" aria-label="Delete message" class="danger" onclick={() => { done(); deleting = true }}><Icon name="trash" /></button>{/if}
        </div>
      </div>
      {#if m.objects?.length}
        <!-- Cards are shared things, not something said: never inside a bubble. -->
        <div class="cards">
          {#each m.objects as object (object.id)}
            {@const Card = objectKind(object.kind)?.card}
            {#if Card}<Card {object} />{/if}
          {/each}
        </div>
      {/if}
      {#if conversation && prefix === 'm'}
        {@const last = conversation.last_reply}
        <!-- How many replies, then the newest one: the parent is right above. -->
        <a
          class="thread-link"
          class:lit={!!unread?.count}
          class:resolved={!!conversation.resolved_at}
          href="/c/{m.channel_id}/t/{conversation.id}"
          title={last ? shortTime(last.created_at) : undefined}
          onclick={(e) => { if (!e.metaKey && !e.ctrlKey && !e.shiftKey && e.button === 0) { e.preventDefault(); onreply(m) } }}
        >
          {#if last}<Avatar userId={last.author_id} size={16} presence={false} />{:else}<Icon name="reply" size={12} />{/if}
          <span class="replies">{conversation.reply_count} {conversation.reply_count === 1 ? 'reply' : 'replies'}</span>
          {#if last}
            <!-- An empty reply carries a file or a card; the preview cannot tell which. -->
            <span class="said"><span class="who">{store.name(last.author_id)}:</span> {plainMentions(snippet(last.excerpt), store.users).trim() || 'shared something'}</span>
          {/if}
          {#if unread?.count}<span class="count" class:at={unread.mention}>{unread.mention ? '@' : unread.count}</span>{/if}
          {#if conversation.resolved_at}<span class="state">resolved</span>{/if}
        </a>
      {/if}
      {#if deleting}
        <div class="deleting" bind:this={deletingBox}><InlineConfirm open action="Delete" sentence="Delete this message? This can't be undone." confirm={() => store.remove(m.id, m.channel_id)} oncancel={keep} /></div>
      {/if}
    </div>
  </div>
</article>

<style>
  /* ---- Shared by bubbles and the plain feed ---- */
  .msg { position: relative; padding: 1px var(--gutter); transition: background-color var(--t-fast); }
  .msg.head { margin-top: calc(12px * var(--density)); }
  .row { display: flex; gap: 8px; }
  .gutter { width: 28px; flex: none; display: flex; justify-content: center; align-items: flex-start; }
  .body { flex: 1; min-width: 0; display: flex; flex-direction: column; align-items: flex-start; }
  .right .body { align-items: flex-end; }
  .content { position: relative; align-self: stretch; display: flex; flex-direction: column; align-items: flex-start; }
  .right .content { align-items: flex-end; }
  .meta { display: flex; align-items: baseline; gap: 6px; max-width: 100%; margin: 0 0 3px 12px; font-size: var(--fs-xs); color: var(--ink-2); }
  /* A very long name stops at one line; the agent badge stays whole. */
  .author { font-weight: 600; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  @media (hover: hover) { .author:hover { color: var(--ink); text-decoration: underline; text-underline-offset: 3px; } }
  .author:active { opacity: .6; }
  .bot {
    flex: none; display: inline-flex; align-items: center; gap: 3px; align-self: center; padding: 0 6px 0 5px;
    border-radius: var(--r-pill, 999px); font: 600 var(--fs-xs)/1.45 var(--body);
    color: var(--lamp); background: var(--lamp-glow);
  }
  .face { display: grid; border-radius: var(--avatar-r, 35%); transition: transform var(--t) var(--ease-out); }
  .face:active { transform: scale(.96); }
  @media (hover: hover) { .face:hover { transform: translateY(-1px); } }
  /* The picture sits beside the last thing in a run, like Messages. */
  .tail-face { position: absolute; left: -36px; bottom: 0; display: grid; }
  .reactions:last-child, .files:last-child { margin-bottom: 0; }

  /* Only messages that land while you watch animate in; history loads still. */
  @media (prefers-reduced-motion: no-preference) {
    .msg.fresh { animation: arrive .32s var(--ease-out) both; }
  }
  @keyframes arrive { from { opacity: 0; transform: translateY(6px); } }
  /* On touch, a long press offers the message's actions, not a text selection or the link menu. */
  @media (hover: none) {
    .msg { -webkit-touch-callout: none; }
    .meta, .text, .reply-ref { -webkit-user-select: none; user-select: none; }
  }

  /* ---- Bubbles ---- */
  /* Sentences keep a readable width; a message with code may use the whole row,
     and the code scrolls sideways inside its own block. */
  .bubble, .gif, .files, .editing, .reply-ref, .deleting { max-width: min(640px, 82%); }
  .bubble.code, .editing { max-width: 100%; }
  .bubble {
    position: relative; min-width: 0; padding: calc(7px * var(--density) + 1px) calc(12px * var(--density) + 1px);
    border-radius: var(--r-bubble); background: var(--fill, var(--bg-3)); color: var(--ink);
    line-height: var(--leading); overflow-wrap: anywhere;
  }
  .bubble :global(.prose) { display: block; max-width: 68ch; }
  /* The corner nearest the speaker is tight, and so are the joins inside a run. */
  .msg:not(.right):not(.head) .bubble { border-top-left-radius: var(--r-tight); }
  .msg:not(.right) .bubble { border-bottom-left-radius: var(--r-tight); }
  .msg.right:not(.head) .bubble { border-top-right-radius: var(--r-tight); }
  .msg.right .bubble { border-bottom-right-radius: var(--r-tight); }
  /* Inside a coloured bubble, links and names are the text colour, underlined or
     bold: the accent can't be read on every person's colour. */
  .bubble :global(a) { color: inherit; text-decoration: underline; text-underline-offset: 2px; }
  .bubble :global(.mention) { color: inherit; background: color-mix(in srgb, var(--ink) 9%, transparent); }
  .bubble :global(code) { background: color-mix(in srgb, var(--ink) 9%, transparent); border-color: transparent; }
  .bubble :global(pre code) { background: none; }
  .bubble :global(pre) { max-width: 100%; margin: 6px 0; background: color-mix(in srgb, var(--ink) 9%, transparent); border-color: transparent; }
  .bubble :global(pre:first-child) { margin-top: 2px; }
  .bubble :global(pre:last-child) { margin-bottom: 2px; }
  /* A message that names you is ringed in the accent as well as lit. */
  .msg.me:not(.feed) .bubble { box-shadow: inset 0 0 0 1.5px var(--lamp); }
  /* A few emoji on their own are a reaction said out loud: big, and no bubble. */
  .bubble.jumbo { background: none; box-shadow: none; padding: 0 2px; font-size: 2.4em; line-height: 1.2; letter-spacing: .04em; }
  .edited { margin-left: 6px; color: var(--ink-2); font-size: var(--fs-xs); white-space: nowrap; }
  .jumbo .edited { font-size: var(--fs-xs); letter-spacing: 0; }
  .editing { width: 100%; }
  .edit { width: 100%; resize: vertical; }
  .hint { font-size: var(--fs-xs); margin-top: 2px; }
  .gif { display: block; margin-top: 2px; border-radius: var(--r-bubble); overflow: hidden; line-height: 0; background: var(--bg-3); }
  .gif img { display: block; max-width: min(320px, 100%); max-height: 260px; min-width: 80px; min-height: 60px; object-fit: contain; }
  .files { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 2px; }
  .right .files { justify-content: flex-end; }
  .msg:not(.feed) .files :global(:is(.image, video.media)) { border-radius: var(--r-bubble); }
  .cards { align-self: stretch; display: flex; flex-direction: column; align-items: flex-start; }
  .right .cards { align-items: flex-end; }
  .msg.held .bubble { filter: brightness(.94); }
  /* Tapbacks hang off the bubble's bottom corner, on the side the bubble starts. */
  .reactions { position: relative; z-index: 1; display: flex; flex-wrap: wrap; gap: 3px; margin: -8px 10px 0; }
  .right .reactions { justify-content: flex-end; }
  .rx { position: relative; min-height: 24px; gap: 3px; padding: 1px 7px; background: var(--bg-2); border-color: transparent; font-size: var(--fs-sm);
    box-shadow: 0 0 0 2px var(--bg), 0 1px 4px var(--shadow); transition: background-color var(--t-fast), transform var(--t) var(--ease-out); }
  /* A fingertip-sized target around a small pill. */
  .rx::after { content: ''; position: absolute; inset: -8px -2px; }
  .rx[aria-pressed=true] { background: var(--sel); box-shadow: 0 0 0 2px var(--bg), inset 0 0 0 1px color-mix(in srgb, var(--lamp) 50%, transparent); }
  @media (hover: hover) { .rx:hover { transform: translateY(-1px); } }
  .rx:active { transform: scale(.94); }
  .rx .n { font-size: var(--fs-xs); font-weight: 700; color: var(--ink-2); font-variant-numeric: tabular-nums; }
  .rx[aria-pressed=true] .n { color: var(--lamp); }
  .reply-ref {
    position: relative; display: flex; text-align: left; align-items: center; gap: 6px; margin: 0 8px 3px; min-width: 0;
    font-size: var(--fs-sm); color: var(--ink-3); white-space: nowrap;
  }
  .reply-ref:hover .who, .reply-ref:hover .snippet { color: var(--ink); }
  .reply-ref .who { color: var(--ink-2); font-weight: 700; }
  .reply-ref .snippet { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  /* A message's thread: a quiet line under it. The reply text gives way first. */
  .thread-link {
    display: inline-flex; align-items: center; gap: 6px; max-width: min(640px, 100%); min-height: 26px; margin: 3px 4px 0; padding: 0 10px 0 4px;
    border-radius: var(--r-pill, 999px); color: var(--ink-2); font-size: var(--fs-xs); white-space: nowrap;
  }
  @media (hover: hover) { .thread-link:hover { background: var(--hover); color: var(--ink); text-decoration: none; } }
  .thread-link:active { background: var(--press); }
  .thread-link .replies { flex: none; color: var(--lamp); font-weight: 700; }
  .thread-link .said { flex: 1 1 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  .thread-link .who { color: var(--ink); font-weight: 600; }
  .thread-link.resolved { opacity: .65; }
  .thread-link.resolved:hover { opacity: 1; }
  .thread-link .state { flex: none; color: var(--ink-3); }
  .thread-link .count {
    flex: none; min-width: 18px; padding: 0 5px; border-radius: var(--r-pill, 999px); background: var(--lamp); color: var(--on-lamp, #000); font-weight: 700; text-align: center;
  }
  @media (pointer: coarse) { .thread-link { min-height: 32px; } }
  .deleting { margin-top: 6px; }
  .deleting :global(.confirmation) { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 12px; }

  /* The actions, and the exact time: on hover with a mouse, on a tap or long
     press on a touch screen. They float over the top edge where the bubble starts. */
  .tools {
    opacity: 0; pointer-events: none; transform: translateY(3px); transition: opacity var(--t-fast), transform var(--t) var(--ease-out);
    position: absolute; bottom: calc(100% - 6px); left: 0; display: flex; align-items: center; gap: 2px; z-index: 2;
    background: var(--bg-2); border: 1px solid var(--line-strong); border-radius: var(--r); padding: 2px;
    box-shadow: var(--lift);
  }
  .right .tools { left: auto; right: 0; }
  .when { padding: 0 6px 0 4px; font-size: var(--fs-xs); color: var(--ink-2); white-space: nowrap; font-variant-numeric: tabular-nums; }
  @media (hover: hover) {
    .msg:hover .tools { opacity: 1; pointer-events: auto; transform: none; }
  }
  .msg.held .tools, .msg:focus-within .tools, .tools:has(:global(details[open])) { opacity: 1; pointer-events: auto; transform: none; }
  .tools button, .tool { padding: 6px; border-radius: var(--r); color: var(--ink-2); display: grid; transition: background-color var(--t-fast), color var(--t-fast); }
  .tools button:hover, .tool:hover { background: var(--bg-3); color: var(--ink); }
  .tools button:active, .tool:active { background: var(--press); }
  .tool { line-height: 20px; }
  .tools button.danger:hover { color: var(--ember); }

  /* ---- The plain feed: names and text, no bubbles ---- */
  .feed.head { margin-top: calc(14px * var(--density)); }
  .feed .row { gap: 12px; }
  .feed .gutter { width: 36px; padding-top: 2px; }
  .feed .meta { margin: 0 0 1px; font-size: var(--fs-body); color: var(--ink); }
  .feed .author { font-weight: 700; color: var(--name, var(--ink)); }
  .feed .content { position: static; }
  .feed .bubble { padding: 0; border-radius: 0; background: none; max-width: 100%; }
  .feed .bubble :global(a) { color: var(--lamp); text-decoration: none; }
  .feed .bubble :global(a:hover) { text-decoration: underline; }
  .feed .bubble :global(.mention) { color: var(--lamp); background: var(--mention-bg); }
  .feed .bubble :global(code), .feed .bubble :global(pre) { background: var(--bg-3); border-color: color-mix(in srgb, var(--line) 70%, transparent); }
  .feed .bubble.jumbo { padding: 0; }
  .feed .gif, .feed .files { margin-top: 4px; max-width: 100%; }
  .feed .gif { border-radius: var(--r); }
  .feed .reactions { margin: 5px 0 0; gap: 4px; }
  .feed .rx { min-height: 26px; padding: 2px 9px 2px 7px; gap: 5px; background: var(--bg-3); box-shadow: none; font-size: var(--fs-body); }
  .feed .rx[aria-pressed=true] { background: var(--sel); border-color: color-mix(in srgb, var(--lamp) 45%, transparent); box-shadow: none; }
  .feed .reply-ref { max-width: calc(100% - 48px); margin: 0 0 2px 48px; }
  /* A spine from the avatar up to the quoted line ties the reply to its parent. */
  .feed .reply-ref::before {
    content: ''; position: absolute; left: -30px; top: 50%; width: 24px; height: 10px;
    border: 2px solid var(--line-strong); border-right: 0; border-bottom: 0; border-top-left-radius: 7px;
  }
  .feed .reply-ref :global(svg) { display: none; }
  .feed .thread-link { margin-left: 0; }
  .feed .tools { bottom: auto; top: -14px; left: auto; right: var(--gutter); }
  @media (hover: hover) {
    .feed:hover { background: var(--hover); }
    .feed.me:hover { background: linear-gradient(90deg, color-mix(in srgb, var(--lamp) 24%, transparent), color-mix(in srgb, var(--lamp) 8%, transparent) 70%); }
  }
  .feed.held { background: var(--hover); }
  /* A mention is lit from the left edge and fades out, like light through a doorway. */
  .feed.me { box-shadow: inset 3px 0 0 var(--lamp); background: linear-gradient(90deg, var(--lamp-glow), color-mix(in srgb, var(--lamp) 5%, transparent) 70%); }
</style>
