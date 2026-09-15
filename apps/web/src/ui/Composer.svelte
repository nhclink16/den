<script lang="ts">
  import { onMount } from 'svelte'
  import { plugins } from '../plugins'
  import { store } from '../lib/store.svelte'
  import type { Channel, Message, User } from '../lib/types'
  import type { PendingUpload } from '../lib/uploads.svelte'
  import { bytes } from '../lib/time'
  import Icon from './Icon.svelte'
  import Avatar from './Avatar.svelte'
  import DictationButton from './DictationButton.svelte'

  let { channel, replyTo = $bindable(null), dropped = $bindable([]), listening = $bindable(false) }: { channel: Channel; replyTo: Message | null; dropped: File[]; listening?: boolean } = $props()

  let text = $state('')
  const pending = $derived(store.uploads.forChannel(channel.id))
  let busy = $state(false)
  let error = $state('')
  let dismissed = $state(false)
  let selected = $state(0)
  const commands = $derived(plugins.flatMap((p) => p.slashCommands))
  const matches = $derived(!dismissed && /^\/\S*$/.test(text) ? commands.filter((c) => c.name.startsWith(text.slice(1))) : [])
  function pickCommand(name: string) { text = `/${name} `; dismissed = true; ta.focus() }

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
    const all = [...store.users.values()]
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
    text = `${text.slice(0, at)}@${u.username} ${text.slice(caret)}`
    dismissed = true
    requestAnimationFrame(() => { ta?.focus(); ta?.setSelectionRange(pos, pos); caret = pos; grow() })
  }

  let ta: HTMLTextAreaElement
  let fileInput: HTMLInputElement

  const placeholder = $derived(channel.kind === 'dm' ? `Message ${store.title(channel)}` : `Say something in #${channel.name}`)
  const uploading = $derived(pending.some((p) => !p.done && !p.error))

  $effect(() => { if (dropped.length) { add(dropped); dropped = [] } })
  $effect(() => { if (replyTo) ta?.focus() })

  onMount(() => {
    let width = ta.clientWidth
    const observer = new ResizeObserver(() => { if (ta.clientWidth !== width) { width = ta.clientWidth; grow() } })
    observer.observe(ta)
    return () => observer.disconnect()
  })

  function add(files: File[]) { store.uploads.add(channel.id, files); ta?.focus() }
  function removePending(p: PendingUpload) { store.uploads.remove(channel.id, p) }

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
    if (e.key === 'Escape' && replyTo) replyTo = null
    if (e.key === 'ArrowUp' && !text) {
      const mine = [...(store.messages.get(channel.id) || [])].reverse().find((m) => m.author_id === store.me?.id)
      if (mine) document.getElementById(`m-${mine.id}`)?.querySelector<HTMLButtonElement>('button[title="Edit"]')?.click()
    }
  }

  // A send can finish after navigation or call expansion removes this composer.
  function grow() { if (!ta) return; dismissed = false; selected = 0; track(); ta.style.height = 'auto'; ta.style.height = Math.min(ta.scrollHeight, 220) + 'px'; if (text.trim()) store.sendTyping(channel.id) }

  async function submit() {
    const channelId = channel.id
    const queue = store.uploads
    const content = text.trim()
    const ready = pending.filter((p) => p.done).map((p) => p.done!.id)
    if ((!content && !ready.length) || uploading || busy) return
    busy = true; error = ''
    try {
      const command = commands.find((c) => content === `/${c.name}` || content.startsWith(`/${c.name} `))
      if (command) {
        await command.run({ channelId: channel.id, args: content.slice(command.name.length + 1).trim(), post: (content) => store.send(channel.id, content) })
      } else await store.send(channel.id, content, { reply_to: replyTo?.id, upload_ids: ready })
      text = ''; replyTo = null
      queue.sent(channelId, ready)
      requestAnimationFrame(grow)
    } catch (err) { error = (err as Error).message } finally {
      busy = false
    }
  }
</script>

<div class="composer">
  {#if error}<p role="alert" class="error">{error}</p>{/if}
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
        <button id={`mention-${i}`} role="option" aria-selected={i === selected % people.length} class:chosen={i === selected % people.length} onmousedown={(e) => e.preventDefault()} onclick={() => pickPerson(u)}>
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
      <span class="snippet faint">{replyTo.content.slice(0, 80)}</span>
      <button class="x" onclick={() => (replyTo = null)} aria-label="Cancel reply"><Icon name="x" size={14} /></button>
    </div>
  {/if}

  {#if pending.length}
    <div class="pending">
      {#each pending as p (p.id)}
        {@const pct = p.progress.total ? Math.round((p.progress.sent / p.progress.total) * 100) : 0}
        <div class="chip" class:err={!!p.error} class:done={!!p.done}>
          <span class="ring" style="--p:{p.done ? 100 : pct}"></span>
          <span class="fname">{p.file.name}</span>
          <span class="faint mono">{p.error || (p.done ? bytes(p.file.size) : `${pct}%`)}</span>
          <button class="x" onclick={() => removePending(p)} aria-label="Remove"><Icon name="x" size={13} /></button>
        </div>
      {/each}
    </div>
  {/if}

  <div class="box" class:uploading>
    <button class="attach" title="Attach a file" onclick={() => fileInput.click()}><Icon name="clip" size={18} /></button>
    <input class="sr-only" type="file" multiple bind:this={fileInput} onchange={(e) => { add([...(e.currentTarget.files || [])]); e.currentTarget.value = '' }} tabindex="-1" />
    <textarea bind:this={ta} bind:value={text} {placeholder} rows="1" oninput={grow} onkeydown={onKey} onkeyup={track} onclick={track} onpaste={onPaste} aria-label={placeholder} aria-controls={matches.length ? "slash-commands" : people.length ? "mention-people" : undefined} aria-activedescendant={matches.length ? `slash-${selected % matches.length}` : people.length ? `mention-${selected % people.length}` : undefined}></textarea>
    <DictationButton channelId={channel.id} textarea={() => ta} text={() => text} bind:listening update={(value, caret) => { text = value; requestAnimationFrame(() => { grow(); ta?.setSelectionRange(caret, caret) }) }} />
    <button class="sendbtn" class:ready={text.trim() || pending.some((p) => p.done)} onclick={submit} disabled={uploading || busy} title="Send (Enter)"><Icon name="send" size={16} /></button>
  </div>
</div>

<style>
  .error { color: var(--ember); }
  .commands { position: absolute; bottom: 100%; left: 16px; right: 16px; background: var(--bg-2); border: 1px solid var(--line); border-radius: 8px; padding: 4px; z-index: 6; }
  .commands button { display: flex; width: 100%; gap: 12px; padding: 8px; text-align: left; border-radius: 4px; }
  .commands span { color: var(--ink-2); }
  .commands .chosen { background: var(--bg-3); }
  .people { max-height: 260px; overflow-y: auto; }
  .people button { align-items: center; gap: 8px; }
  .people b { font-weight: 600; }
  .handle { color: var(--ink-3); font-size: 13px; }
  .composer { position: relative; padding: 0 var(--gutter) 14px; }
  .box {
    display: flex; align-items: flex-end; gap: 6px; padding: calc(6px * var(--density)) calc(6px * var(--density)) calc(6px * var(--density)) calc(8px * var(--density));
    background: var(--bg-3); border: 1px solid var(--line); border-radius: var(--r-lg);
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  .box:focus-within { border-color: var(--lamp); box-shadow: 0 0 0 3px var(--lamp-glow); }
  textarea {
    flex: 1; min-width: 0; resize: none; background: none; border: 0; outline: 0; padding: calc(8px * var(--density)) 4px;
    max-height: 220px; line-height: 1.4; color: var(--ink);
  }
  textarea::placeholder { color: var(--ink-3); }
  .attach, .sendbtn { display: grid; padding: 9px; border-radius: 8px; color: var(--ink-3); flex: none; }
  .attach:hover { color: var(--ink); background: var(--bg-2); }
  .sendbtn.ready { color: var(--lamp); }
  .sendbtn:disabled { opacity: 0.4; }
  .reply-bar, .pending {
    display: flex; align-items: center; gap: 8px; padding: 6px 12px; margin: 0 8px;
    background: var(--bg-2); border: 1px solid var(--line); border-bottom: 0;
    border-radius: var(--r-lg) var(--r-lg) 0 0; font-size: 13px;
  }
  .pending { flex-wrap: wrap; gap: 6px; }
  .snippet { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .x { display: grid; padding: 3px; border-radius: 4px; color: var(--ink-3); }
  .x:hover { color: var(--ink); background: var(--bg-3); }
  .chip {
    display: inline-flex; align-items: center; gap: 8px; padding: 5px 8px 5px 6px;
    background: var(--bg-3); border-radius: 999px; max-width: 100%;
  }
  .chip.err { color: var(--ember); }
  .ring {
    width: 14px; height: 14px; border-radius: 50%; flex: none;
    background: conic-gradient(var(--lamp) calc(var(--p) * 1%), var(--line) 0);
    mask: radial-gradient(circle, transparent 45%, var(--bg) 50%);
  }
  .chip.done .ring { background: var(--lamp); mask: none; }
  .fname { max-width: 220px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
