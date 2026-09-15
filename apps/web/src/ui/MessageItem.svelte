<script lang="ts">
  import { objectKind } from '../plugins'
  import { store } from '../lib/store.svelte'
  import type { Message } from '../lib/types'
  import { render } from '../lib/markdown'
  import { shortTime } from '../lib/time'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import Attachment from './Attachment.svelte'

  let { m, compact, onreply, onmediaready }: { m: Message; compact: boolean; onreply: (m: Message) => void; onmediaready?: () => void } = $props()
  let editing = $state(false)
  let draft = $state('')
  const mine = $derived(m.author_id === store.me?.id)
  const canDelete = $derived(mine || store.me?.role === 'admin')
  const author = $derived(store.user(m.author_id))
  const html = $derived(render(m.content, store.users))
  let parent = $state<Message | undefined>()
  $effect(() => { if (m.reply_to) store.fetchMessage(m.reply_to, m.channel_id).then((p) => (parent = p)); else parent = undefined })
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

<article class="msg" class:compact class:me={mentionsMe} id="m-{m.id}">
  {#if parent}
    <div class="reply-ref">
      <Icon name="reply" size={12} />
      <span class="who">{store.name(parent.author_id)}</span>
      <span class="snippet">{parent.content.slice(0, 90) || 'sent a file'}</span>
    </div>
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
        <div class="files">{#each m.attachments as a (a.id)}<Attachment upload={a} {onmediaready} />{/each}</div>
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
