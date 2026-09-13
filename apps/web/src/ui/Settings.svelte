<script lang="ts">
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { api } from '../lib/api'
  import { notify } from '../lib/notify.svelte'
  import type { BotCreated, Category, Channel, Invite, Token, TokenSecret } from '../lib/types'
  import MachineSettings from './MachineSettings.svelte'
  import VoiceSettings from './VoiceSettings.svelte'
  import Icon from './Icon.svelte'

  let { section = 'notifications', onmenu, narrow }: { section?: string; onmenu: () => void; narrow: boolean } = $props()
  const admin = $derived(store.me?.role === 'admin')
  const sections = $derived([
    ['notifications', 'Notifications'], ['voice', 'Voice'], ['machines', 'Machines'], ['access', 'Access'], ...(admin ? [['plugins', 'Plugins']] : []), ['layout', 'Layout'], ['agents', 'Agents'],
    ...(admin ? [['invites', 'Invites'], ['rooms', 'Rooms']] : []), ['account', 'Account'],
  ] as [string, string][])
  let q = $state('')
  const visible = $derived(sections.filter(([, l]) => !q || l.toLowerCase().includes(q.toLowerCase())))
  const go = (s: string) => (e: MouseEvent) => { e.preventDefault(); router.go(`/settings/${s}`) }

  // --- notifications ---
  let perm = $state(typeof Notification !== 'undefined' ? Notification.permission : 'denied')
  async function enable() { await notify.ask(); perm = Notification.permission }
  function toggleSub(id: string) {
    const s = new Set(store.notif.subscribed_channel_ids); s.has(id) ? s.delete(id) : s.add(id)
    store.saveNotif({ subscribed_channel_ids: [...s] })
  }

  // --- agents ---
  let tokens = $state<Token[]>([])
  let botName = $state('')
  let botDisplay = $state('')
  let tokenName = $state('')
  let reveal = $state<{ label: string; token: string } | null>(null)
  let copied = $state(false)
  let agentErr = $state('')
  async function loadTokens() { try { tokens = await api.get<Token[]>('/tokens') } catch { tokens = [] } }
  $effect(() => { if (section === 'agents') loadTokens() })
  async function createBot(e: SubmitEvent) {
    e.preventDefault(); agentErr = ''
    try {
      const b = await api.post<BotCreated>('/bots', { username: botName.trim(), display_name: botDisplay.trim() || botName.trim() })
      reveal = { label: `${b.user.display_name} is ready. Its token, shown once:`, token: b.credential.token }
      botName = ''; botDisplay = ''
      await store.resync(); await loadTokens()
    } catch (err) { agentErr = (err as Error).message }
  }
  async function createToken(e: SubmitEvent) {
    e.preventDefault(); agentErr = ''
    try {
      const t = await api.post<TokenSecret>('/tokens', { name: tokenName.trim() || 'token', user_id: null })
      reveal = { label: `Token "${t.credential.name}" posts as you. Shown once:`, token: t.token }
      tokenName = ''; await loadTokens()
    } catch (err) { agentErr = (err as Error).message }
  }
  async function revoke(t: Token) {
    if (!confirm(`Revoke "${t.name}"? Anything using it stops working.`)) return
    await api.del(`/tokens/${t.id}`); await loadTokens()
  }
  async function copy(s: string) { await navigator.clipboard.writeText(s); copied = true; setTimeout(() => (copied = false), 1500) }

  // --- invites ---
  let invites = $state<Invite[]>([])
  let uses = $state(1)
  let hours = $state(48)
  async function makeInvite(e: SubmitEvent) {
    e.preventDefault()
    invites = [await api.post<Invite>('/invites', { uses, expires_in_hours: hours }), ...invites]
  }
  const inviteLink = (code: string) => `${location.origin}/login?invite=${encodeURIComponent(code)}`

  // --- rooms ---
  let newCat = $state('')
  let newChan = $state('')
  let newChanCat = $state('')
  let roomErr = $state('')
  async function addCategory(e: SubmitEvent) {
    e.preventDefault(); roomErr = ''
    try { await api.post<Category>('/categories', { name: newCat.trim(), position: store.categories.length }); newCat = ''; await store.resync() } catch (err) { roomErr = (err as Error).message }
  }
  async function addChannel(e: SubmitEvent) {
    e.preventDefault(); roomErr = ''
    try { await api.post<Channel>('/channels', { name: newChan.trim().toLowerCase(), category_id: newChanCat || null, position: store.textChannels.length }); newChan = ''; await store.resync() } catch (err) { roomErr = (err as Error).message }
  }
  async function delChannel(c: Channel) {
    if (!confirm(`Delete #${c.name} and everything in it?`)) return
    await api.del(`/channels/${c.id}`); await store.resync()
  }
  async function delCategory(c: Category) {
    if (!confirm(`Delete the "${c.name}" category? Its rooms stay, uncategorized.`)) return
    await api.del(`/categories/${c.id}`); await store.resync()
  }

  async function logout() { await store.logout(); router.go('/login') }
</script>

<section class="settings">
  <header class="head">
    {#if narrow}<button class="btn quiet iconbtn" onclick={onmenu} aria-label="Menu"><Icon name="menu" /></button>{/if}
    <span class="kind"><Icon name="gear" size={18} /></span>
    <h1 class="display">Settings</h1>
  </header>

  <div class="body">
    <nav class="toc">
      <div class="search"><Icon name="search" size={14} /><input bind:value={q} placeholder="Find a setting" aria-label="Find a setting" /></div>
      {#each visible as [id, label] (id)}
        <a href="/settings/{id}" class:active={section === id} onclick={go(id)}>{label}</a>
      {/each}
    </nav>

    <div class="pane">
      {#if section === 'machines' || section === 'access'}
      <MachineSettings {section} />
    {:else if section === 'notifications'}
        <h2 class="display">Notifications</h2>
        <p class="muted">Quiet by default. You get told about mentions and direct messages. Follow a room to hear about everything in it.</p>
        {#if perm !== 'granted'}
          <div class="callout">
            <span>{perm === 'denied' ? 'Notifications are blocked in your browser settings.' : 'Desktop notifications are off.'}</span>
            {#if perm === 'default'}<button class="btn lit" onclick={enable}>Turn on</button>{/if}
          </div>
        {/if}
        <label class="switch"><input type="checkbox" checked={store.notif.mentions} onchange={(e) => store.saveNotif({ mentions: e.currentTarget.checked })} /> When someone mentions me</label>
        <label class="switch"><input type="checkbox" checked={store.notif.dms} onchange={(e) => store.saveNotif({ dms: e.currentTarget.checked })} /> Direct messages</label>
        <label class="switch"><input type="checkbox" checked={store.layout.sounds} onchange={(e) => store.saveLayout({ sounds: e.currentTarget.checked })} /> Play a sound</label>
        <h3 class="eyebrow">Rooms you follow</h3>
        <p class="faint small">Every message in a followed room notifies you. Follow sparingly.</p>
        {#each store.textChannels as c (c.id)}
          <label class="switch"><input type="checkbox" checked={store.notif.subscribed_channel_ids.includes(c.id)} onchange={() => toggleSub(c.id)} /> #{c.name}</label>
        {/each}

      {:else if section === 'voice'}
        <VoiceSettings />
      {:else if section === 'plugins' && admin}
        <h2 class="display">Plugins</h2>
        <label class="switch"><input type="checkbox" checked={store.settings.canvas_enabled} onchange={async (e) => { try { store.settings = await api.put('/settings', { canvas_enabled: e.currentTarget.checked }); agentErr = '' } catch (err) { agentErr = (err as Error).message } }} /> Canvas</label>
        <p class="muted">A shared drawing board anyone in a room can open. Uses tldraw.</p>
        {#if agentErr}<p role="alert" class="error">{agentErr}</p>{/if}
      {:else if section === 'layout'}
        <h2 class="display">Layout</h2>
        <label class="switch"><input type="checkbox" checked={store.layout.sidebar} onchange={(e) => store.saveLayout({ sidebar: e.currentTarget.checked })} /> Show the room list <kbd>Ctrl+\</kbd></label>
        <label class="switch"><input type="checkbox" checked={store.layout.members} onchange={(e) => store.saveLayout({ members: e.currentTarget.checked })} /> Show people <kbd>Ctrl+Shift+M</kbd></label>
        <p class="muted small">Jump anywhere with <kbd>Ctrl+K</kbd>. Edit your last message with <kbd>↑</kbd> in an empty composer.</p>

      {:else if section === 'agents'}
        <h2 class="display">Agents</h2>
        <p class="muted">An agent is just a member with a token. No app registration, no OAuth, no portal. Give the token to the thing that should talk here.</p>
        {#if reveal}
          <div class="reveal">
            <div>{reveal.label}</div>
            <code class="secret">{reveal.token}</code>
            <div class="row">
              <button class="btn" onclick={() => copy(reveal!.token)}><Icon name={copied ? 'check' : 'copy'} /> {copied ? 'Copied' : 'Copy'}</button>
              <button class="btn quiet" onclick={() => (reveal = null)}>I saved it</button>
            </div>
            <div class="faint small">Use it as <code>DEN_TOKEN</code> with the <code>den</code> CLI, or as a bearer token.</div>
          </div>
        {/if}
        <form class="inline" onsubmit={createBot}>
          <h3 class="eyebrow">New agent</h3>
          <input class="field" bind:value={botName} placeholder="username, e.g. clanker" pattern={'[a-z0-9_]{3,32}'} required />
          <input class="field" bind:value={botDisplay} placeholder="Display name (optional)" />
          <button class="btn lit" type="submit"><Icon name="bot" /> Create agent</button>
        </form>
        <form class="inline" onsubmit={createToken}>
          <h3 class="eyebrow">Token that posts as you</h3>
          <input class="field" bind:value={tokenName} placeholder="What is it for, e.g. laptop script" />
          <button class="btn" type="submit"><Icon name="plus" /> Create token</button>
        </form>
        {#if agentErr}<p class="error">{agentErr}</p>{/if}
        <h3 class="eyebrow">Active tokens</h3>
        {#if !tokens.length}<p class="faint">None yet.</p>{/if}
        {#each tokens as t (t.id)}
          {@const owner = store.user(t.user_id)}
          <div class="tok">
            <span class="tname">{t.name}</span>
            <span class="muted">{owner?.bot ? `agent ${owner.display_name || owner.username}` : 'you'}</span>
            <span class="spacer"></span>
            <button class="btn quiet danger" onclick={() => revoke(t)}>Revoke</button>
          </div>
        {/each}

      {:else if section === 'invites' && admin}
        <h2 class="display">Invites</h2>
        <form class="inline" onsubmit={makeInvite}>
          <label><span class="eyebrow">Uses</span><input class="field" type="number" min="1" max="50" bind:value={uses} /></label>
          <label><span class="eyebrow">Expires in hours</span><input class="field" type="number" min="1" max="720" bind:value={hours} /></label>
          <button class="btn lit" type="submit"><Icon name="plus" /> New invite</button>
        </form>
        {#each invites as inv (inv.id)}
          <div class="tok">
            <code class="secret small">{inviteLink(inv.code)}</code>
            <span class="faint mono">{inv.uses_left} left</span>
            <button class="btn" onclick={() => copy(inviteLink(inv.code))}><Icon name={copied ? 'check' : 'copy'} /></button>
          </div>
        {/each}
        {#if !invites.length}<p class="faint">Invites you create this session show up here.</p>{/if}

      {:else if section === 'rooms' && admin}
        <h2 class="display">Rooms</h2>
        <form class="inline" onsubmit={addCategory}>
          <input class="field" bind:value={newCat} placeholder="New category" required />
          <button class="btn" type="submit"><Icon name="plus" /> Add category</button>
        </form>
        <form class="inline" onsubmit={addChannel}>
          <input class="field" bind:value={newChan} placeholder="new-room" pattern={'[a-z0-9_-]{1,40}'} required />
          <select class="field" bind:value={newChanCat}>
            <option value="">No category</option>
            {#each store.categories as c (c.id)}<option value={c.id}>{c.name}</option>{/each}
          </select>
          <button class="btn lit" type="submit"><Icon name="plus" /> Add room</button>
        </form>
        {#if roomErr}<p class="error">{roomErr}</p>{/if}
        {#each [null, ...store.categories] as cat (cat?.id ?? 'none')}
          {@const chans = store.textChannels.filter((c) => (c.category_id || null) === (cat?.id || null))}
          {#if cat || chans.length}
            <div class="cat-row">
              <span class="eyebrow">{cat ? cat.name : 'Uncategorized'}</span>
              {#if cat}<button class="btn quiet danger" onclick={() => delCategory(cat)}>Delete category</button>{/if}
            </div>
            {#each chans as c (c.id)}
              <div class="tok"><Icon name="hash" /><span class="tname">{c.name}</span><span class="spacer"></span><button class="btn quiet danger" onclick={() => delChannel(c)}>Delete</button></div>
            {/each}
          {/if}
        {/each}

      {:else if section === 'account'}
        <h2 class="display">Account</h2>
        <p>Signed in as <b>{store.me?.display_name || store.me?.username}</b> <span class="muted">@{store.me?.username}</span>{#if admin} <span class="faint mono">admin</span>{/if}</p>
        <button class="btn danger" onclick={logout}>Log out</button>
      {/if}
    </div>
  </div>
</section>

<style>
  .settings { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .head { display: flex; align-items: center; gap: 10px; padding: 10px 16px; min-height: 52px; border-bottom: 1px solid var(--line); }
  .kind { color: var(--ink-3); display: grid; }
  h1 { font-size: 19px; margin: 0; font-weight: 600; }
  .iconbtn { padding: 6px; }
  .body { flex: 1; min-height: 0; display: grid; grid-template-columns: 200px minmax(0, 1fr); }
  @media (max-width: 700px) { .body { grid-template-columns: 1fr; grid-template-rows: auto 1fr; } .toc { flex-direction: row; flex-wrap: wrap; border-right: 0 !important; border-bottom: 1px solid var(--line); } }
  .toc { display: flex; flex-direction: column; gap: 2px; padding: 16px 10px; border-right: 1px solid var(--line); }
  .toc a { padding: 7px 10px; border-radius: var(--r); color: var(--ink-2); }
  .toc a:hover { background: var(--bg-3); color: var(--ink); text-decoration: none; }
  .toc a.active { background: var(--bg-3); color: var(--ink); font-weight: 700; }
  .search { display: flex; align-items: center; gap: 8px; padding: 6px 10px; margin-bottom: 8px; color: var(--ink-3); border: 1px solid var(--line); border-radius: var(--r); }
  .search input { flex: 1; min-width: 0; background: none; border: 0; outline: 0; color: var(--ink); font-size: 13px; }
  .pane { overflow-y: auto; padding: 20px 28px 40px; max-width: 680px; }
  h2 { font-size: 26px; margin: 0 0 6px; }
  h3.eyebrow { margin: 22px 0 8px; }
  .muted.small, .faint.small { font-size: 13px; }
  .switch { display: flex; align-items: center; gap: 10px; padding: 8px 0; }
  .switch input { accent-color: var(--lamp); width: 16px; height: 16px; }
  .callout { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 12px 14px; margin: 12px 0; background: var(--bg-2); border: 1px solid var(--line); border-radius: var(--r); }
  .inline { display: flex; flex-wrap: wrap; align-items: flex-end; gap: 8px; margin: 8px 0 16px; }
  .inline .eyebrow { display: block; margin: 0 0 8px; width: 100%; }
  .inline h3 { flex-basis: 100%; }
  .inline .field { width: auto; flex: 1; min-width: 160px; }
  .inline label { display: flex; flex-direction: column; }
  .inline label .eyebrow { margin: 0 0 6px; }
  .reveal { display: grid; gap: 10px; padding: 14px; margin: 12px 0; background: var(--lamp-glow); border: 1px solid var(--lamp-dim); border-radius: var(--r-lg); }
  .secret { display: block; padding: 8px 10px; background: var(--bg); border-radius: var(--r); word-break: break-all; user-select: all; }
  .secret.small { font-size: 12px; flex: 1; }
  .row { display: flex; gap: 8px; }
  .tok { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid var(--line); border-radius: var(--r); margin-bottom: 6px; }
  .tname { font-weight: 700; }
  .spacer { flex: 1; }
  .cat-row { display: flex; align-items: center; justify-content: space-between; margin: 18px 0 6px; }
  .error { color: var(--ember); }
  select.field { appearance: auto; }
</style>
