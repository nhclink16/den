<script module lang="ts">
  /// Puts the cursor in the reply box inside `root`, for a conversation someone
  /// just opened. A touch screen gets it only straight after their own tap, so a
  /// reload or a Back never pops the keyboard up. Returns whether it did.
  export function focusComposer(root: ParentNode | null | undefined): boolean {
    if (matchMedia('(pointer: coarse)').matches && !navigator.userActivation?.isActive) return false
    const box = root?.querySelector<HTMLTextAreaElement>('textarea[data-composer]')
    box?.focus()
    return !!box && document.activeElement === box
  }
</script>

<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { plugins } from '../plugins'
  import { planComposerSubmit, shouldClearDraft, type DraftIdentity } from '../lib/composer-submit'
  import { conversationKey, room, sameConversation, type Conversation } from '../lib/conversation'
  import type { Drafts } from '../lib/drafts'
  import { store } from '../lib/store.svelte'
  import type { Channel, Message, User } from '../lib/types'
  import type { PendingUpload } from '../lib/uploads.svelte'
  import { bytes } from '../lib/time'
  import Icon from './Icon.svelte'
  import { roomName } from '../lib/room-list'
  import { themes } from '../lib/theme.svelte'
  import Avatar from './Avatar.svelte'
  import DictationButton from './DictationButton.svelte'
  import GifPicker from './GifPicker.svelte'
  import EmojiPicker from './EmojiPicker.svelte'
  import { messageUrl, snippet, type Gif } from '../lib/klipy'
  import { disclosurePopover } from '../lib/disclosure-popover'

  // `conversation` defaults to the room, so every existing caller keeps its
  // current behaviour untouched while a thread composer can pass its own root.
  let { channel, conversation, locked, replyTo = $bindable(null), dropped = $bindable([]), listening = $bindable(false) }: { channel: Channel; conversation?: Conversation; locked?: string; replyTo: Message | null; dropped: File[]; listening?: boolean } = $props()
  const here = $derived<Conversation>(conversation ?? room(channel.id))
  // The conversation's identity is channel plus ROOT. The server's thread ID
  // appears once the first reply is saved and is placement context only, so
  // acquiring it never moves the draft or its files to a different owner.
  const threadId = $derived(here.rootId ? store.threadForRoot(here.rootId)?.id : undefined)
  const unsaved = $derived(!!here.rootId && !threadId)

  // The draft belongs to the conversation, not to this component: navigating away
  // and back, or a call expanding over the view, must not lose what was typed.
  // Every text change goes through setText so the edit count stays honest: a send
  // that resolves later compares this, not the string, before clearing the box.
  const text = $derived(store.drafts.for(here).text)
  function setText(value: string) {
    if (value === text) return
    store.drafts.setText(here, value)
  }
  // Read through a captured owner, never through a derived and never through the
  // `store` proxy after an await: the proxy follows the active instance, and a
  // derived on a destroyed component hands back its last cached value.
  const draft = (owner: Drafts, c: Conversation): DraftIdentity =>
    ({ conversation: c, replyToId: owner.for(c).replyToId, revision: owner.for(c).revision })

  // True until this composer is destroyed. A send that finishes afterwards may
  // still clear the stored draft, but must not write to bindings nobody is
  // showing any more.
  let alive = true

  // The quote belongs to the DRAFT, which outlives the message list: a reply
  // parent can scroll out of the loaded page while the draft answering it is
  // still open. This effect only presents it. Failing to resolve the message is
  // not a cancel, so it never writes back a null; only cancelReply and a
  // successful send clear the target.
  $effect(() => {
    const key = conversationKey(here)
    untrack(() => {
      const c = here
      const owner = store.drafts
      const token = owner.token
      const saved = owner.for(c).replyToId
      if (saved === (replyTo?.id ?? null)) return
      if (!saved) { replyTo = null; return }
      const cached = store.messages.get(c.channelId)?.find((m) => m.id === saved)
      if (cached) { replyTo = cached; return }
      const load = store.fetchMessage
      void load(saved, c.channelId).then((m) => {
        // A late arrival may only fill in the banner for the same composer, the
        // same owner and the same quote it was asked about.
        if (!m || !alive || conversationKey(here) !== key) return
        if (store.drafts !== owner || !owner.holds(token) || owner.for(c).replyToId !== saved) return
        replyTo = m
      })
    })
  })
  // Picking a target stores it. Clearing is never inferred here.
  $effect(() => {
    const id = replyTo?.id ?? null
    if (id === null) return
    untrack(() => store.drafts.setReplyTo(here, id))
  })
  function cancelReply() {
    store.drafts.setReplyTo(here, null)
    replyTo = null
  }
  const pending = $derived(store.uploads.forConversation(here))
  let busy = $state(false)
  let error = $state('')
  let dismissed = $state(false)
  let selected = $state(0)
  const commands = $derived(plugins.flatMap((p) => p.slashCommands))
  const matches = $derived(!dismissed && /^\/\S*$/.test(text) ? commands.filter((c) => c.name.startsWith(text.slice(1))) : [])
  function pickCommand(name: string) { setText(`/${name} `); dismissed = true; ta.focus() }

  // Mentions. The caret matters, so this cannot key off `text` alone: typing `@a`
  // in the middle of a sentence should offer people, and moving away should stop.
  let caret = $state(0)
  function track() { caret = ta?.selectionStart ?? 0 }

  // Mirrors the `@` rule in mention_names(), crates/den-server/src/activity.rs, so
  // the list never offers a name the server would not have linked.
  const mentioning = $derived.by(() => {
    if (dismissed) return null
    const m = /(?:^|[^A-Za-z0-9_@])@([A-Za-z0-9_.-]*)$/.exec(text.slice(0, caret))
    return m ? m[1]! : null
  })

  const people = $derived.by(() => {
    const query = mentioning
    if (query === null) return []
    const q = query.toLowerCase()
    const all = store.people
    const here = channel.kind === 'dm' ? all.filter((u) => channel.member_ids?.includes(u.id)) : all
    const shown = (u: User) => u.display_name || u.username
    const starts = (u: User) => u.username.toLowerCase().startsWith(q) || shown(u).toLowerCase().startsWith(q)
    return here
      .filter((u) => u.username.toLowerCase().includes(q) || shown(u).toLowerCase().includes(q))
      .sort((a, b) =>
        Number(starts(b)) - Number(starts(a)) ||
        Number(store.online.has(b.id)) - Number(store.online.has(a.id)) ||
        a.username.localeCompare(b.username))
      .slice(0, 8)
  })

  function pickPerson(u: User) {
    const at = text.slice(0, caret).lastIndexOf('@')
    const pos = at + u.username.length + 2
    setText(`${text.slice(0, at)}@${u.username} ${text.slice(caret)}`)
    dismissed = true
    requestAnimationFrame(() => { ta?.focus(); ta?.setSelectionRange(pos, pos); caret = pos; grow() })
  }

  let ta: HTMLTextAreaElement
  let fileInput: HTMLInputElement

  const placeholder = $derived(here.rootId ? 'Reply in thread…' : channel.kind === 'dm' ? `Message ${store.title(channel)}` : `Message ${roomName(channel, themes.appearance.room_icons)}`)
  const uploading = $derived(pending.some((p) => !p.done && !p.error))
  const sendable = $derived(!locked && (!!text.trim() || pending.some((p) => p.done)))

  // A phone gets the Quiet box: a round + holds files, GIFs, emoji and dictation,
  // so the text gets the width, and the mic turns into Send once there is something
  // to send. Wider screens keep every button in the box.
  const narrow = matchMedia('(max-width: 600px)')
  let phone = $state(narrow.matches)
  $effect(() => {
    const change = () => (phone = narrow.matches)
    narrow.addEventListener('change', change)
    return () => narrow.removeEventListener('change', change)
  })
  let menu = $state(false), gifOpen = $state(false), emojiOpen = $state(false)
  let canDictate = $state(false)
  let dictation = $state<ReturnType<typeof DictationButton>>()
  // The mic stays while it is listening, so it can always be stopped.
  const showMic = $derived(canDictate && (!phone || !sendable || listening))
  const showSend = $derived(!phone || !showMic)
  function fromMenu(run: () => void) { menu = false; run() }
  // A picker opened from the menu hands focus back to its own button when it
  // closes, and that button is hidden here: give it to the + instead.
  // iOS before 17.4 has no checkVisibility; a hidden button has no boxes there.
  let plus = $state<HTMLElement>(), pickerWasOpen = false
  $effect(() => {
    const open = gifOpen || emojiOpen
    if (open || !pickerWasOpen) { pickerWasOpen = open; return }
    pickerWasOpen = false
    requestAnimationFrame(() => { const at = document.activeElement as HTMLElement | null; if (phone && (!at || at === document.body || !(at.checkVisibility?.() ?? at.getClientRects().length > 0))) plus?.focus() })
  })

  $effect(() => { if (dropped.length) { add(dropped); dropped = [] } })
  $effect(() => { if (replyTo) ta?.focus() })

  onMount(() => {
    let width = ta.clientWidth
    const observer = new ResizeObserver(() => { if (ta.clientWidth !== width) { width = ta.clientWidth; grow() } })
    observer.observe(ta)
    return () => { alive = false; observer.disconnect() }
  })

  function add(files: File[]) { store.uploads.add(here, files); ta?.focus() }
  // A staged picture shows itself, so you can tell which one you're about to send.
  function preview(img: HTMLImageElement, file: File) {
    const url = URL.createObjectURL(file)
    img.src = url
    return { destroy: () => URL.revokeObjectURL(url) }
  }
  function removePending(p: PendingUpload) { store.uploads.remove(here, p) }

  function onPaste(e: ClipboardEvent) {
    const files = [...(e.clipboardData?.files || [])]
    if (files.length) { e.preventDefault(); add(files) }
  }

  function onKey(e: KeyboardEvent) {
    // Only one of the two popups can be open: a slash command owns the whole box,
    // a mention needs an `@` before the caret.
    const open = matches.length || people.length
    if (open) {
      if (e.key === 'Escape') { e.preventDefault(); dismissed = true; return }
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); selected = (selected + (e.key === 'ArrowDown' ? 1 : open - 1)) % open; return }
      if ((e.key === 'Enter' || e.key === 'Tab') && !e.isComposing) {
        e.preventDefault()
        if (matches.length) pickCommand(matches[selected % matches.length]!.name)
        else pickPerson(people[selected % people.length]!)
        return
      }
    }
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) { e.preventDefault(); submit() }
    if (e.key === 'Escape' && replyTo) cancelReply()
    if (e.key === 'ArrowUp' && !text) {
      const mine = [...(store.messages.get(channel.id) || [])].reverse().find((m) => m.author_id === store.me?.id)
      if (mine) document.getElementById(`m-${mine.id}`)?.querySelector<HTMLButtonElement>('button[title="Edit"]')?.click()
    }
  }

  // A send can finish after navigation or call expansion removes this composer.
  // A destroyed one must not measure a detached box or announce typing to
  // whichever instance is active by then.
  function grow() { if (!alive || !ta) return; dismissed = false; selected = 0; track(); ta.style.height = 'auto'; ta.style.height = Math.min(ta.scrollHeight, 220) + 'px'; if (text.trim()) store.sendTyping(channel.id, threadId, unsaved) }

  // An emoji goes where the cursor was (the box keeps its selection while the
  // picker has focus). A phone gets the keyboard back only when it taps the box.
  function insertEmoji(emoji: string) {
    const start = ta.selectionStart, end = ta.selectionEnd
    setText(text.slice(0, start) + emoji + text.slice(end))
    const pos = start + emoji.length
    requestAnimationFrame(() => {
      if (!matchMedia('(pointer: coarse)').matches) ta?.focus()
      ta?.setSelectionRange(pos, pos); caret = pos; grow()
    })
  }

  // A GIF goes out as its own message, in the same place and quoting the same
  // message a typed one would. Whatever is in the box stays there.
  async function sendGif(g: Gif) {
    const url = messageUrl(g)
    if (!url || locked) return
    const conversation = here
    const owner = store.drafts
    const token = owner.token
    const send = store.send
    const replyToId = owner.for(conversation).replyToId
    const placement: { thread_id?: string; reply_to?: string } =
      threadId ? { thread_id: threadId } : here.rootId ? { reply_to: here.rootId } : {}
    await send(channel.id, url, { ...placement, reply_to: replyToId ?? placement.reply_to })
    if (owner.holds(token) && owner.for(conversation).replyToId === replyToId) {
      owner.setReplyTo(conversation, null)
      if (alive && sameConversation(conversation, here)) replyTo = null
    }
    if (alive) ta?.focus()
  }

  async function submit() {
    const channelId = channel.id
    // Everything this send will still need afterwards is captured NOW, off the
    // proxy: which conversation, which draft owner, which lifetime of it, which
    // upload queue, and a send bound to this instance. After the await `store`
    // may be a different account entirely.
    const conversation = here
    const owner = store.drafts
    const token = owner.token
    const queue = store.uploads
    const send = store.send
    const content = text.trim()
    const ready = pending.filter((p) => p.done).map((p) => p.done!.id)
    const plan = planComposerSubmit(content, ready, commands.map((c) => c.name))
    if (!plan || uploading || busy || locked) return
    // Placement is captured with everything else. The ROOT places a first reply
    // and is not the quote: cancelling the quote in an unsaved panel must not
    // take the placement with it.
    const placement: { thread_id?: string; reply_to?: string } =
      threadId ? { thread_id: threadId } : here.rootId ? { reply_to: here.rootId } : {}
    const submitted = draft(owner, conversation)
    // The account this send belongs to. The Store object alone is not enough:
    // the same one is reused by whoever signs in next, so the owner's lifetime
    // token is what says this is still the same account.
    const sameAccount = () => store.drafts === owner && owner.holds(token)
    const live = () => alive && sameAccount()
    busy = true; error = ''
    try {
      if (plan.kind === 'command') {
        const command = commands.find((c) => c.name === plan.name)!
        // A command can post long after it was invoked. By then the account may
        // have changed, and that post would be written as the new one.
        await command.run({
          channelId,
          conversation,
          replyToId: submitted.replyToId,
          args: plan.args,
          post: async (content) => {
            if (sameAccount()) await send(channelId, content, { ...placement, reply_to: submitted.replyToId ?? placement.reply_to })
          },
        })
        // Commands never receive upload IDs, so completed attachments stay queued.
      } else {
        await send(channelId, content, {
          ...placement,
          // An explicit quote wins over the root for reply_to; placement still
          // carries the thread when there is one.
          reply_to: submitted.replyToId ?? placement.reply_to,
          upload_ids: plan.uploadIds,
        })
        // Consume exactly what was transmitted, whatever the box holds by now.
        // The conversation was captured at submit, so a newer draft staged
        // elsewhere keeps its own files and this one loses only what it sent.
        queue.sent(conversation, plan.uploadIds)
      }
      // Clear only the draft that was actually sent, in the conversation it was
      // sent from, on the owner it was read from. A cleared and refilled owner
      // can hand out the same revision again, so the token is checked too.
      if (owner.holds(token) && shouldClearDraft(submitted, draft(owner, conversation))) {
        owner.setText(conversation, '')
        owner.setReplyTo(conversation, null)
        // Only touch the binding if this composer is still showing that draft.
        if (live() && sameConversation(conversation, here)) replyTo = null
      }
      // Post-completion UI work belongs to the composer that started it, and only
      // while it is still showing this account.
      if (live()) requestAnimationFrame(grow)
    } catch (err) {
      if (live()) error = (err as Error).message
    } finally {
      if (live()) busy = false
    }
  }
</script>

<div class="composer">
  {#if error}<p role="alert" class="error">{error}</p>{/if}
  <!-- A resolved conversation takes nothing new, but what you already typed is
       still yours: the draft and its files stay exactly where they are. -->
  {#if locked}<p class="locked" role="status">{locked}</p>{/if}
  {#if matches.length}
    <div class="commands" role="listbox" id="slash-commands" aria-label="Commands">
      {#each matches as command, i}
        <button id={`slash-${i}`} role="option" aria-selected={i === selected % matches.length} class:chosen={i === selected % matches.length} onmousedown={(e) => e.preventDefault()} onclick={() => pickCommand(command.name)}><b>/{command.name}</b><span>{command.hint}</span></button>
      {/each}
    </div>
  {/if}
  {#if people.length}
    <div class="commands people" role="listbox" id="mention-people" aria-label="People">
      {#each people as u, i}
        <button id={`mention-${i}`} data-hover-animates role="option" aria-selected={i === selected % people.length} class:chosen={i === selected % people.length} onmousedown={(e) => e.preventDefault()} onclick={() => pickPerson(u)}>
          <Avatar userId={u.id} size={22} />
          <b>{u.display_name || u.username}</b>
          <span class="handle">@{u.username}</span>
        </button>
      {/each}
    </div>
  {/if}
  {#if replyTo}
    <div class="reply-bar">
      <Icon name="reply" size={13} />
      <span>Replying to <b>{store.name(replyTo.author_id)}</b></span>
      <span class="snippet faint">{snippet(replyTo.content).slice(0, 80)}</span>
      <button class="x" onclick={cancelReply} aria-label="Cancel reply"><Icon name="x" size={14} /></button>
    </div>
  {/if}

  {#if pending.length}
    <div class="pending">
      {#each pending as p (p.id)}
        {@const pct = p.progress.total ? Math.round((p.progress.sent / p.progress.total) * 100) : 0}
        <div class="chip" class:err={!!p.error} class:done={!!p.done}>
          {#if p.file.type.startsWith('image/')}<img class="thumb" alt="" use:preview={p.file} onerror={(e) => ((e.currentTarget as HTMLImageElement).hidden = true)} />{:else}<span class="ring" style="--p:{p.done ? 100 : pct}"></span>{/if}
          <span class="fname">{p.file.name}</span>
          <span class="faint">{p.error || (p.done ? bytes(p.file.size) : `${pct}%`)}</span>
          <button class="x" onclick={() => removePending(p)} aria-label="Remove"><Icon name="x" size={13} /></button>
        </div>
      {/each}
    </div>
  {/if}

  <div class="row">
    {#if phone}
      <details class="more" bind:open={menu}>
        <summary bind:this={plus} class="icon-btn plus" title="Add a file, GIF, emoji or dictation" aria-label="More"><Icon name="plus" size={20} /></summary>
        <div class="sheet" popover="auto" use:disclosurePopover={'start'}>
          {#if menu}
            <div class="menu" role="menu" aria-label="Add to your message">
              <button role="menuitem" onclick={() => fromMenu(() => fileInput.click())}><Icon name="image" size={20} />Photo or file</button>
              <button role="menuitem" disabled={!!locked} onclick={() => fromMenu(() => (gifOpen = true))}><span class="badge">GIF</span>GIF</button>
              <button role="menuitem" onclick={() => fromMenu(() => (emojiOpen = true))}><Icon name="smile" size={20} />Emoji</button>
              {#if canDictate}<button role="menuitem" onclick={() => fromMenu(() => void dictation?.toggle())}><Icon name="mic" size={20} />Dictate</button>{/if}
            </div>
          {/if}
        </div>
      </details>
    {/if}
    <div class="box" class:uploading>
      <input class="sr-only" type="file" multiple bind:this={fileInput} onchange={(e) => { add([...(e.currentTarget.files || [])]); e.currentTarget.value = '' }} tabindex="-1" />
      <!-- On a phone these three live in the + menu. The pickers stay mounted, with
           their own buttons hidden, so the menu can open them. -->
      <button class="attach icon-btn" class:hidden={phone} title="Attach a file" aria-label="Attach a file" onclick={() => fileInput.click()}><Icon name="clip" size={18} /></button>
      <span class="picker" class:tucked={phone}><GifPicker label="Send a GIF" onpick={sendGif} disabled={!!locked} bind:open={gifOpen}><span class="icon-btn"><span class="badge">GIF</span></span></GifPicker></span>
      <span class="picker" class:tucked={phone}><EmojiPicker label="Add an emoji" onpick={insertEmoji} bind:open={emojiOpen}><span class="icon-btn"><Icon name="smile" size={18} /></span></EmojiPicker></span>
      <textarea data-composer bind:this={ta} bind:value={() => text, (value) => setText(value)} {placeholder} rows="1" oninput={grow} onkeydown={onKey} onkeyup={track} onclick={track} onpaste={onPaste} aria-label={placeholder} aria-controls={matches.length ? "slash-commands" : people.length ? "mention-people" : undefined} aria-activedescendant={matches.length ? `slash-${selected % matches.length}` : people.length ? `mention-${selected % people.length}` : undefined}></textarea>
      <DictationButton bind:this={dictation} channelId={channel.id} textarea={() => ta} text={() => text} bind:listening bind:available={canDictate} hidden={!showMic} update={(value, caret) => { setText(value); requestAnimationFrame(() => { grow(); ta?.setSelectionRange(caret, caret) }) }} />
      {#if showSend}<button class="sendbtn icon-btn" class:ready={sendable} onclick={submit} disabled={!!locked || uploading || busy} title={locked ?? 'Send (Enter)'} aria-label="Send"><Icon name="up" size={18} /></button>{/if}
    </div>
  </div>
</div>

<style>
  .error { color: var(--ember); }
  .commands { position: absolute; bottom: calc(100% - 6px); left: var(--gutter); right: var(--gutter); background: var(--bg-2); border: 1px solid var(--line-strong); border-radius: var(--r-lg); padding: 4px; z-index: 6; box-shadow: var(--lift); }
  .locked { margin: 0 0 6px; padding: 6px 10px; border: 1px solid var(--line); border-radius: var(--r); color: var(--ink-2); font-size: 12px; }
  .commands button { display: flex; width: 100%; gap: 12px; padding: 8px; text-align: left; border-radius: var(--r); }
  .commands span { color: var(--ink-2); }
  .commands .chosen { background: var(--bg-3); box-shadow: inset 2px 0 0 var(--lamp); }
  .people { max-height: 260px; overflow-y: auto; }
  .people button { align-items: center; gap: 8px; }
  .people b { font-weight: 600; }
  .handle { color: var(--ink-3); font-size: 13px; }
  .composer { position: relative; padding: 0 var(--gutter) 14px; }
  .row { display: flex; align-items: flex-end; gap: var(--s2); }
  /* A pill while it is one line, a rounded card once it grows. Sharp corners stay sharp. */
  .box {
    flex: 1; min-width: 0; display: flex; align-items: flex-end; gap: 2px; padding: 3px;
    background: var(--bg); border: 1px solid var(--line-strong); border-radius: min(var(--r-pill), calc(var(--hit) / 2 + 4px));
    transition: border-color var(--t), box-shadow var(--t);
  }
  .box:focus-within { border-color: var(--lamp); box-shadow: 0 0 0 3px var(--lamp-glow); }
  textarea {
    flex: 1; min-width: 0; resize: none; background: none; border: 0; outline: 0;
    padding: calc((var(--hit) - 1.4em) / 2) var(--s2) calc((var(--hit) - 1.4em) / 2) var(--s3);
    max-height: 220px; line-height: 1.4; color: var(--ink);
  }
  /* A long room name must not wrap the placeholder onto a hidden second line. */
  textarea::placeholder { color: var(--ink-3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .picker { display: contents; }
  .hidden { display: none; }
  /* Only the picker's own button goes: a popover inside a hidden box can't show. */
  .tucked :global(summary) { display: none; }
  .badge { font: 800 9.5px/1 var(--body); letter-spacing: .04em; padding: 3px 3.5px 2.5px; border: 1.5px solid currentColor; border-radius: 4px; }
  /* Send: quiet until there is something to send, then a lit circle. */
  .sendbtn { color: var(--ink-3); transition: color var(--t-fast), background-color var(--t-fast), transform var(--t) var(--ease-out); }
  .sendbtn.ready { color: var(--on-lamp); background: var(--lamp); }
  @media (hover: hover) { .sendbtn.ready:hover { color: var(--on-lamp); background: var(--lamp); filter: brightness(1.06); } }
  .sendbtn.ready:active { transform: scale(.94); }
  .sendbtn:disabled { opacity: .4; }
  .plus { background: var(--bg-3); color: var(--ink-2); list-style: none; cursor: pointer; }
  .plus::-webkit-details-marker { display: none; }
  .more[open] > .plus { color: var(--lamp); }
  .more { display: flex; }
  .more [popover] { position: fixed; margin: 0; padding: 0; border: 1px solid var(--line-strong); border-radius: var(--r-lg); background: var(--bg-2); color: var(--ink); box-shadow: 0 14px 44px var(--shadow); overflow: hidden; }
  .menu { display: grid; padding: var(--s3) var(--s3) var(--s4); gap: 0; }
  .menu button { display: flex; align-items: center; gap: var(--s3); min-height: 52px; padding: 0 var(--s4); background: var(--bg); color: var(--ink); font-size: var(--fs-body); text-align: start; border-bottom: 1px solid var(--line); }
  .menu button:first-child { border-radius: var(--r-lg) var(--r-lg) 0 0; }
  .menu button:last-child { border-radius: 0 0 var(--r-lg) var(--r-lg); border-bottom: 0; }
  .menu button:only-child { border-radius: var(--r-lg); }
  .menu button:active { background: var(--press); }
  .menu button:disabled { opacity: .5; }
  .menu :global(svg), .menu .badge { color: var(--lamp); flex: none; }
  .menu .badge { width: 20px; text-align: center; padding: 3px 0 2.5px; }
  .reply-bar, .pending {
    display: flex; align-items: center; gap: 8px; padding: 6px 12px; margin: 0 8px;
    background: var(--bg-2); border: 1px solid var(--line); border-bottom: 0;
    border-radius: var(--r-lg) var(--r-lg) 0 0; font-size: 13px;
  }
  .pending { flex-wrap: wrap; gap: 6px; }
  .snippet { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .x { display: grid; place-items: center; min-width: 24px; min-height: 24px; border-radius: var(--r-pill); color: var(--ink-3); }
  @media (pointer: coarse) { .x { min-width: 36px; min-height: 36px; margin: -6px; } }
  .x:hover { color: var(--ink); background: var(--bg-3); }
  .chip {
    display: inline-flex; align-items: center; gap: 8px; padding: 5px 8px 5px 6px;
    background: var(--bg-3); border-radius: var(--r); max-width: 100%;
  }
  .chip.err { color: var(--ember); }
  .ring {
    width: 14px; height: 14px; border-radius: 50%; flex: none;
    background: conic-gradient(var(--lamp) calc(var(--p) * 1%), var(--line) 0);
    mask: radial-gradient(circle, transparent 45%, var(--bg) 50%);
  }
  .chip.done .ring { background: var(--lamp); mask: none; }
  .thumb { width: 32px; height: 32px; margin: -3px 0; flex: none; object-fit: cover; border-radius: calc(var(--r) * .75); }
  .chip:not(.done) .thumb { opacity: .6; }
  .fname { max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
