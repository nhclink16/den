<script lang="ts">
  import { objectKind } from '../plugins'
  import { instances, store } from '../lib/store.svelte'
  import { goToMessage } from '../lib/notify.svelte'
  import type { Message } from '../lib/types'
  import { render } from '../lib/markdown'
  import { shortTime } from '../lib/time'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import Attachment from './Attachment.svelte'

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
  let parent = $state<Message | undefined>()
  // Decoration only: a lookup that fails leaves the reference undecorated. It
  // is never a cancellation, and it never touches a draft's quote.
  $effect(() => {
    if (!m.reply_to) { parent = undefined; return }
    const want = m.reply_to
    void store.fetchMessage(want, m.channel_id)
      .then((p) => { if (m.reply_to === want) parent = p })
      .catch(() => { if (m.reply_to === want) parent = undefined })
  })
  const mentionsMe = $derived(!!store.me && (m.mention_ids || []).includes(store.me.id))
  const QUICK = ['👍', '😂', '❤️', '🔥', '👀', '💀']
  let picker = $state(false)

  function startEdit() { draft = m.content; editing = true }
  async function saveEdit(e: KeyboardEvent) {
    if (e.key === 'Escape') { editing = false; return }
    if (e.key !== 'Enter' || e.shiftKey) return
    e.preventDefault()
    const text = draft.trim()
    if (text && text !== m.content) await store.edit(m.id, text)
    editing = false
  }
  async function del() {
    if (confirm('Delete this message?')) await store.remove(m.id, m.channel_id)
  }
</script>

<article class="msg" class:compact class:me={mentionsMe} id="{prefix}-{m.id}">
  {#if parent}
    <!-- The quoted message may be outside the loaded page, or in another
         conversation entirely; the shared resolver finds it either way. -->
    <!-- The concrete owner, not the proxy: goToMessage checks that the source is
         a real instance, and the proxy is not one. -->
    <button class="reply-ref" onclick={() => goToMessage(instances.active, parent!.channel_id, parent!.id)}>
      <Icon name="reply" size={12} />
      <span class="who">{store.name(parent.author_id)}</span>
      <span class="snippet">{parent.content.slice(0, 90) || 'sent a file'}</span>
    </button>
  {/if}
  <div class="row">
    <div class="gutter">
      {#if compact}<span class="stamp">{shortTime(m.created_at)}</span>{:else}<Avatar userId={m.author_id} size={36} />{/if}
    </div>
    <div class="body">
      {#if !compact}
        <div class="meta">
          <span class="author">{author?.display_name || author?.username || 'someone'}</span>
          {#if author?.bot}<span class="bot" title="Agent"><Icon name="bot" size={11} /></span>{/if}
          <span class="time">{shortTime(m.created_at)}</span>
        </div>
      {/if}
      {#if editing}
        <textarea class="field edit" bind:value={draft} onkeydown={saveEdit} rows="2"></textarea>
        <div class="faint hint">Enter to save · Esc to cancel</div>
      {:else}
        <div class="text">{@html html}{#if m.edited_at}<span class="edited" title={m.edited_at}> (edited)</span>{/if}</div>
      {/if}
      {#each m.objects || [] as object (object.id)}
        {@const Card = objectKind(object.kind)?.card}
        {#if Card}<Card {object} />{/if}
      {/each}
      {#if m.attachments?.length}
        <div class="files">{#each m.attachments as a (a.id)}<Attachment upload={a} author={store.name(m.author_id)} {onmediaready} />{/each}</div>
      {/if}
      {#if conversation && prefix === 'm'}
        <a
          class="thread-link"
          class:lit={!!unread?.count}
          href="/c/{m.channel_id}/t/{conversation.id}"
          onclick={(e) => { if (!e.metaKey && !e.ctrlKey && !e.shiftKey && e.button === 0) { e.preventDefault(); onreply(m) } }}
        >
          <Icon name="reply" size={12} />
          <span class="thread-title">{conversation.title}</span>
          <span class="faint">{conversation.reply_count} {conversation.reply_count === 1 ? 'reply' : 'replies'}</span>
          {#if unread?.count}<span class="count" class:at={unread.mention}>{unread.mention ? '@' : unread.count}</span>{/if}
          {#if conversation.resolved_at}<span class="faint mono">resolved</span>{/if}
        </a>
      {/if}
      {#if m.reactions?.length}
        <div class="reactions">
          {#each m.reactions as r (r.emoji)}
            <button class="rx" class:mine={!!store.me && r.user_ids.includes(store.me.id)} onclick={() => store.react(m, r.emoji)} title={r.user_ids.map((id) => store.name(id)).join(', ')}>
              <span>{r.emoji}</span><span class="n">{r.user_ids.length}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
    <div class="tools">
      <div class="pick-wrap">
        <button title="React" aria-label="React to this message" onclick={() => (picker = !picker)}><span aria-hidden="true">😶</span></button>
        {#if picker}
          <div class="picker" role="menu">
            {#each QUICK as e (e)}<button role="menuitem" onclick={() => { store.react(m, e); picker = false }}>{e}</button>{/each}
          </div>
        {/if}
      </div>
      <button title="Reply" onclick={() => onreply(m)}><Icon name="reply" /></button>
      {#if mine}<button title="Edit" onclick={startEdit}><Icon name="edit" /></button>{/if}
      {#if canDelete}<button title="Delete" class="danger" onclick={del}><Icon name="trash" /></button>{/if}
    </div>
  </div>
</article>

<style>
  .thread-link {
    display: inline-flex; align-items: center; gap: 8px; margin-top: 6px; padding: 4px 10px;
    border: 1px solid var(--line); border-radius: 999px; color: var(--ink-2); font-size: 12px; max-width: 100%;
  }
  .thread-link:hover { background: var(--hover); color: var(--ink); }
  .thread-link.lit { border-color: var(--lamp); color: var(--ink); }
  .thread-title { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .thread-link .count {
    min-width: 18px; padding: 0 5px; border-radius: 999px; background: var(--lamp); color: var(--on-lamp, #000);
    font-family: var(--mono); font-size: 11px; text-align: center;
  }
  .msg { position: relative; padding: calc(2px * var(--density)) 20px; margin-top: calc(14px * var(--density)); }
  .msg.compact { margin-top: 0; }
  .msg:hover { background: var(--hover); }
  .msg.me { box-shadow: inset 3px 0 0 var(--lamp); background: var(--lamp-glow); }
  .row { display: flex; gap: 12px; }
  .gutter { width: 36px; flex: none; display: flex; justify-content: center; align-items: flex-start; padding-top: 2px; }
  .msg.compact .gutter { height: 22px; }
  .stamp { font-family: var(--mono); font-size: 11px; line-height: 22px; color: var(--ink-2); opacity: 0; }
  .msg:hover .stamp { opacity: 1; }
  .body { flex: 1; min-width: 0; }
  .meta { display: flex; align-items: baseline; gap: 8px; margin-bottom: 1px; }
  .author { font-weight: 700; }
  .bot { color: var(--lamp); display: inline-grid; align-self: center; }
  .time { font-family: var(--mono); font-size: 11px; color: var(--ink-3); }
  /* Prose ran 120-260 characters per line on a wide window. Attachments,
     code blocks and embeds stay full width. */
  .text { line-height: var(--leading); overflow-wrap: anywhere; max-width: 68ch; }
  .text :global(pre) { max-width: none; }
  .edited { color: var(--ink-3); font-size: 12px; }
  .edit { resize: vertical; margin-top: 2px; }
  .hint { font-size: 12px; margin-top: 2px; }
  .files { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 6px; }
  .reply-ref {
    display: flex; align-items: center; gap: 6px; margin: 0 0 2px 48px;
    font-size: 12.5px; color: var(--ink-3); overflow: hidden; white-space: nowrap;
  }
  .reply-ref .who { color: var(--ink-2); font-weight: 700; }
  .reply-ref .snippet { overflow: hidden; text-overflow: ellipsis; }
  .tools {
    opacity: 0; pointer-events: none; transition: opacity .12s;
    position: absolute; right: var(--gutter); top: -12px; display: flex; gap: 2px;
    background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r); padding: 2px;
  }
  .msg:hover .tools, .msg:focus-within .tools, .msg:has(.picker) .tools { opacity: 1; pointer-events: auto; }
  .tools button { padding: 5px; border-radius: var(--r); color: var(--ink-2); display: grid; }
  .tools button:hover { background: var(--bg-3); color: var(--ink); }
  .tools button.danger:hover { color: var(--ember); }
  .reactions { display: flex; flex-wrap: wrap; gap: 4px; margin-top: 5px; }
  .rx { display: inline-flex; align-items: center; gap: 5px; padding: 2px 8px; border-radius: var(--r); border: 1px solid var(--line-strong); background: var(--bg-2); font-size: 13px; }
  .rx:hover { border-color: var(--ink-3); }
  .rx.mine { border-color: var(--lamp-dim); background: var(--lamp-glow); }
  .rx .n { font-family: var(--mono); font-size: 11px; color: var(--ink-2); }
  .pick-wrap { position: relative; }
  .picker { position: absolute; right: 0; top: 100%; margin-top: 4px; display: flex; gap: 2px; padding: 4px; background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r); z-index: 3; }
  .picker button { font-size: 17px; padding: 4px 6px; }
</style>
