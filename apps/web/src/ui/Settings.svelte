<script lang="ts">
  import { onDestroy, tick, untrack } from 'svelte'
  import { forgetShownOnce } from '../lib/settings-secrets.svelte'
  import SidebarToggle from './SidebarToggle.svelte'
  import HomeButton from './HomeButton.svelte'
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { api } from '../lib/api'
  import { notify } from '../lib/notify.svelte'
  import { customizer } from '../lib/customizer.svelte'
  import { visibleSections, findSettings, reveal, type Hit, type Section } from '../lib/settings-search'
  import ProfileSettings from './ProfileSettings.svelte'
  import MachineSettings from './MachineSettings.svelte'
  import SettingRow from './SettingRow.svelte'
  import SoundSettings from './SoundSettings.svelte'
  import VoiceSettings from './VoiceSettings.svelte'
  import SpotifySettings from './SpotifySettings.svelte'
  import MemberSettings from './MemberSettings.svelte'
  import KeySettings from './KeySettings.svelte'
  import InviteSettings from './InviteSettings.svelte'
  import RoomSettings from './RoomSettings.svelte'
  import Appearance from './Appearance.svelte'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import { roomName } from '../lib/room-list'
  import { themes } from '../lib/theme.svelte'
  import DesktopApp from './DesktopApp.svelte'
  import { isDesktop } from '../lib/desktop'

  /** `narrow`: one thing at a time. The list of sections, or a page pushed in over it, like iPhone Settings. */
  let { section, narrow }: { section?: string; narrow: boolean } = $props()
  const admin = $derived(store.me?.role === 'admin')
  // Everyday settings first. Machines, access and keys are for people running
  // agents and terminals, so they sit last under Advanced, still one tap away.
  const sections = $derived(visibleSections(admin, isDesktop()))
  // A wide window always shows a page; a phone shows the list until you pick one.
  const current = $derived(sections.some((s) => s.id === section) ? section! : narrow ? null : 'profile')
  const title = $derived(sections.find((s) => s.id === current)?.label ?? 'Settings')
  const groups = $derived((['you', 'app', 'account', 'server', 'advanced'] as const).map((g) => ({ g, list: sections.filter((s) => s.group === g) })).filter((x) => x.list.length))
  const groupLabel = { you: '', app: '', account: '', server: 'Server', advanced: 'Advanced' } as const

  // On a computer Appearance isn't a page: it opens over the room you came from,
  // so each change shows where it matters. A phone has no room beside it, so there
  // it is a page with a preview at the top.
  $effect(() => {
    if (current !== 'appearance' || narrow) return
    untrack(() => { customizer.show(pending ?? undefined); pending = null; router.go(customizer.lastPlace, true) })
  })
  // The page still offers Undo, back to how things looked when you opened it.
  $effect(() => {
    if (current !== 'appearance' || !narrow) return
    untrack(() => customizer.begin())
    // Widening the window hands the page over to the panel, which keeps the same Undo.
    return () => queueMicrotask(() => { if (!customizer.open) customizer.end() })
  })

  // --- moving around ---
  let pane = $state<HTMLElement>()
  let pending = $state<string | null>(null)
  function open(id: string, find?: string) {
    pending = find ?? null
    // Only a page opened from the list goes back to it with Back; one opened from another page swaps in the list.
    if (id !== section) router.go(`/settings/${id}`, false, narrow && !current ? '/settings' : undefined)
    // Already showing it (a computer shows Profile at /settings): nothing changes to look again, so look now.
    if (id === current) seek()
  }
  const go = (id: string) => (e: MouseEvent) => { e.preventDefault(); open(id) }
  function toList() {
    if (router.parent === '/settings') router.back()
    else router.go('/settings', true)
  }
  // Land on the row a search found. Some pages load what they show, so look a few times.
  function seek() {
    const find = pending
    if (!find) return
    let tries = 0
    const look = () => {
      if (pending !== find) return
      if (pane && reveal(pane, find)) { pending = null; return }
      // Nothing to land on (it isn't shown right now): focus the page instead.
      if (++tries > 12) { pending = null; focusPage(); return }
      setTimeout(look, 120)
    }
    void tick().then(look)
  }
  $effect(() => { void current; if (pending) seek() })
  // A new page starts at its top.
  $effect(() => { void current; pane?.scrollTo({ top: 0 }) })
  // Pushing a page in moves keyboard focus to its title; coming back puts it on the row you left from.
  let heading = $state<HTMLElement>(), home = $state<HTMLElement>(), last: string | null = null
  $effect(() => {
    const now = current, from = last
    last = now
    // A search landing focuses the row it found instead.
    if (!narrow || now === from || untrack(() => pending)) return
    void tick().then(() => {
      if (now) heading?.focus({ preventScroll: true })
      // The list starts at its top again, so let the row scroll into view.
      else if (from) home?.querySelector<HTMLElement>(`a[href="/settings/${from}"]`)?.focus()
    })
  })

  // --- search ---
  let q = $state('')
  const hits = $derived(findSettings(q, sections, admin))
  function pick(h: Hit) {
    q = ''; open(h.section.id, h.entry?.find)
    // A whole section has no row to land on, and the result just went away: focus the page.
    if (!h.entry) void tick().then(focusPage)
  }
  /** Keyboard focus to the top of the page: the header title on a phone, the page's own title on a computer. */
  function focusPage() {
    pane?.scrollTo({ top: 0 })
    const title = narrow ? heading : pane?.querySelector<HTMLElement>('h2')
    if (!title) return
    title.tabIndex = -1; title.focus({ preventScroll: true })
  }
  function searchKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && hits[0]) { e.preventDefault(); pick(hits[0]) }
    if (e.key === 'Escape' && q) { e.preventDefault(); q = '' }
  }

  // --- notifications ---
  // The iPhone app's page has no Notification at all: nothing to unblock there.
  let perm = $state<NotificationPermission | 'unsupported'>(typeof Notification !== 'undefined' ? Notification.permission : 'unsupported')
  async function enable() { await notify.ask(); perm = Notification.permission }
  function toggleSub(id: string) {
    const s = new Set(store.notif.subscribed_channel_ids); s.has(id) ? s.delete(id) : s.add(id)
    store.saveNotif({ subscribed_channel_ids: [...s] })
  }
  let pluginErr = $state('')

  // Keys and invite links shown once stay across sections, and go when you leave Settings.
  onDestroy(forgetShownOnce)

  async function logout() { await store.logout(); router.go('/login') }
</script>

{#snippet finder()}
  <div class="find input-shell" role="search">
    <Icon name="search" size={16} />
    <input bind:value={q} placeholder="Find a setting" aria-label="Find a setting" enterkeyhint="search" autocomplete="off" onkeydown={searchKey} />
    {#if q}<button class="clear" onclick={() => (q = '')} aria-label="Clear search"><Icon name="x" size={12} /></button>{/if}
  </div>
{/snippet}

{#snippet results()}
  {#if hits.length}
    <nav class="slist results" aria-label="Settings found">
      {#each hits as h (h.section.id + (h.entry?.find ?? '') + (h.entry?.label ?? ''))}
        <a class="nav-row" href="/settings/{h.section.id}" onclick={(e) => { e.preventDefault(); pick(h) }}>
          <span class="grow"><span class="hit">{h.entry?.label ?? h.section.label}</span>{#if h.entry}<span class="where">{h.section.label}</span>{/if}</span>
          <span class="chev"><Icon name="chevron" size={14} /></span>
        </a>
      {/each}
    </nav>
  {:else}
    <p class="none" role="status">Nothing matches “{q.trim()}”.</p>
  {/if}
{/snippet}

<section class="settings" class:drill={narrow} class:list-view={narrow && !current}>
  <header class="head">
    {#if narrow && current}
      <button class="back" onclick={toList}><Icon name="back" size={22} /><span>Settings</span></button>
      <h1 class="title" tabindex="-1" bind:this={heading}>{title}</h1>
      {#if current === 'appearance'}
        <button class="undo" disabled={!customizer.changed} onclick={() => customizer.undo()}>Undo</button>
      {:else}<span class="undo" aria-hidden="true"></span>{/if}
    {:else if narrow}
      <HomeButton />
    {:else}
      {#if !store.layout.sidebar}<SidebarToggle />{/if}
      <span class="kind"><Icon name="gear" size={18} /></span>
      <h1 class="display">Settings</h1>
    {/if}
  </header>

  <div class="body">
    {#if narrow && !current}
      <!-- The phone's Settings list: you at the top, then every section. -->
      <div class="home" bind:this={home}>
        <h1 class="display large">Settings</h1>
        {@render finder()}
        {#if q.trim()}
          {@render results()}
        {:else}
          {#if store.me}
            <a class="slist me" href="/settings/profile" onclick={go('profile')}>
              <span class="nav-row">
                <Avatar userId={store.me.id} size={56} presence={false} />
                <span class="grow"><b>{store.me.display_name || store.me.username}</b><span class="sub">{#if store.me.status?.emoji || store.me.status?.text}{store.me.status.emoji ?? ''} {store.me.status.text ?? ''}{:else}Profile, picture and status{/if}</span></span>
                <span class="chev"><Icon name="chevron" size={14} /></span>
              </span>
            </a>
          {/if}
          {#each groups.filter((x) => x.g !== 'you') as { g, list } (g)}
            {#if groupLabel[g]}<h2 class="list-head">{groupLabel[g]}</h2>{:else}<div class="gap"></div>{/if}
            <nav class="slist" aria-label={groupLabel[g] || 'Settings'}>
              {#each list as s (s.id)}{@render navItem(s, true)}{/each}
            </nav>
          {/each}
        {/if}
      </div>
    {:else}
      {#if !narrow}
        <nav class="toc" aria-label="Settings sections">
          {@render finder()}
          {#if q.trim()}
            {@render results()}
          {:else}
            {#each groups as { g, list } (g)}
              {#if groupLabel[g]}<span class="toc-label">{groupLabel[g]}</span>{/if}
              {#each list as s (s.id)}{@render navItem(s, false)}{/each}
            {/each}
          {/if}
        </nav>
      {/if}

      <div class="pane" bind:this={pane} class:wide-pane={current === 'profile' || current === 'appearance'}>
        <div class="pane-inner">
        {#if current === 'profile'}
          <ProfileSettings />
        {:else if current === 'appearance'}
          <Appearance />
        {:else if current === 'machines' || current === 'access'}
          <MachineSettings section={current} />
        {:else if current === 'notifications'}
          <h2 class="display">Notifications</h2>
          <p class="muted">Quiet by default. You get told about mentions and direct messages. Follow a room to hear about everything in it.</p>
          {#if perm !== 'granted'}
            <div class="slist callout">
              <SettingRow find="permission" label={perm === 'unsupported' ? 'Notifications on this device' : 'Desktop notifications'} hint={perm === 'unsupported' ? "Phone notifications aren't available yet. Mentions and DMs wait for you in the Inbox." : perm === 'denied' ? 'Blocked in your browser settings. Allow them there for this site.' : 'Off. Turn them on to hear about mentions and DMs while Den is in the background.'}>
                {#snippet control()}{#if perm === 'default'}<button class="btn lit" onclick={enable}>Turn on</button>{/if}{/snippet}
              </SettingRow>
            </div>
          {/if}
          <h3 class="list-head">Always</h3>
          <!-- Notifications already on: a search for them lands here. -->
          <div class="slist" data-find={perm === 'granted' ? 'permission' : undefined}>
            <SettingRow find="mentions" label="When someone mentions me">
              {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="When someone mentions me" checked={store.notif.mentions} onchange={(e) => store.saveNotif({ mentions: e.currentTarget.checked })} />{/snippet}
            </SettingRow>
            <SettingRow find="dms" label="Direct messages">
              {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="Direct messages" checked={store.notif.dms} onchange={(e) => store.saveNotif({ dms: e.currentTarget.checked })} />{/snippet}
            </SettingRow>
            <a class="nav-row" href="/settings/sounds" onclick={go('sounds')}><span class="grow">Notification sounds and volume</span><span class="chev"><Icon name="chevron" size={14} /></span></a>
          </div>
          <h3 class="list-head">Rooms you follow</h3>
          <div class="slist" data-find="follow">
            {#each store.textChannels as c (c.id)}
              {@const name = roomName(c, themes.appearance.room_icons)}
              <SettingRow label={name}>
                {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label={`Follow ${name}`} checked={store.notif.subscribed_channel_ids.includes(c.id)} onchange={() => toggleSub(c.id)} />{/snippet}
              </SettingRow>
            {/each}
          </div>
          <p class="list-foot">Every message in a followed room notifies you. Follow sparingly.</p>

        {:else if current === 'sounds'}
          <SoundSettings />
        {:else if current === 'voice'}
          <VoiceSettings />
        {:else if current === 'spotify'}
          <SpotifySettings />
        {:else if current === 'plugins'}
          <h2 class="display">Plugins</h2>
          <p class="muted">Extras for everyone on this server.</p>
          <div class="slist gap-top">
            <SettingRow find="canvas" label="Canvas" hint="A shared drawing board anyone in a room can open. Uses tldraw.">
              {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="Canvas" checked={store.settings.canvas_enabled} onchange={async (e) => { try { store.settings = await api.put('/settings', { canvas_enabled: e.currentTarget.checked }); pluginErr = '' } catch (err) { pluginErr = (err as Error).message } }} />{/snippet}
            </SettingRow>
          </div>
          {#if pluginErr}<p role="alert" class="error">{pluginErr}</p>{/if}
        {:else if current === 'layout'}
          <h2 class="display">Layout</h2>
          <p class="muted">What shows beside the conversation on a wide screen.</p>
          <div class="slist gap-top">
            <SettingRow find="sidebar" label="Show the room list" hint="Ctrl+\">
              {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="Show the room list" checked={store.layout.sidebar} onchange={(e) => store.saveLayout({ sidebar: e.currentTarget.checked })} />{/snippet}
            </SettingRow>
            <SettingRow find="members" label="Show people" hint="Ctrl+Shift+M">
              {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="Show people" checked={store.layout.members} onchange={(e) => store.saveLayout({ members: e.currentTarget.checked })} />{/snippet}
            </SettingRow>
          </div>
          <p class="list-foot" data-find="shortcuts">Jump anywhere with <kbd>Ctrl+K</kbd>. Edit your last message with <kbd>↑</kbd> in an empty message box.{#if narrow} On a phone the room list is your home screen, so these apply on wider screens.{/if}</p>

        {:else if current === 'keys' || current === 'agents'}
          <KeySettings section={current} />
        {:else if current === 'members'}
          <MemberSettings />
        {:else if current === 'invites'}
          <InviteSettings />
        {:else if current === 'rooms'}
          <RoomSettings />
        {:else if current === 'desktop'}
          <DesktopApp />
        {:else if current === 'account'}
          <h2 class="display">Account</h2>
          <div class="slist gap-top">
            <SettingRow label="Name">{#snippet control()}<span class="val">{store.me?.display_name || store.me?.username}</span>{/snippet}</SettingRow>
            <SettingRow label="Username">{#snippet control()}<span class="val">@{store.me?.username}</span>{/snippet}</SettingRow>
            <SettingRow label="Role">{#snippet control()}<span class="val">{admin ? 'Admin' : 'Member'}</span>{/snippet}</SettingRow>
          </div>
          <div class="slist gap-top" data-find="logout"><button class="action-row danger" onclick={logout}>Log out</button></div>
          <p class="list-foot">Logging out signs out this device only.</p>
        {/if}
        </div>
      </div>
    {/if}
  </div>
</section>

{#snippet navItem(s: Section, tile: boolean)}
  <a class="nav-row" class:item={!tile} href="/settings/{s.id}" aria-current={current === s.id ? 'page' : undefined} onclick={go(s.id)}>
    <span class:tile class="ic"><Icon name={s.icon as 'gear'} size={tile ? 17 : 16} /></span>
    <span class="grow">{s.label}</span>
    {#if tile}<span class="chev"><Icon name="chevron" size={14} /></span>{/if}
  </a>
{/snippet}

<style>
  .settings { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .head { display: flex; align-items: center; gap: 10px; padding: 10px 16px; min-height: 52px; border-bottom: 1px solid var(--line); }
  .kind { color: var(--ink-3); display: grid; }
  .head h1 { font-size: var(--fs-title); margin: 0; font-weight: 600; }
  .body { flex: 1; min-height: 0; display: grid; grid-template-columns: 232px minmax(0, 1fr); grid-template-rows: minmax(0, 1fr); }

  /* The section list on a computer. */
  .toc { display: flex; flex-direction: column; gap: 1px; padding: 14px 10px; border-right: 1px solid var(--line); background: var(--bg-2); overflow-y: auto; }
  .toc .item { min-height: 34px; padding: 0 10px; gap: 10px; border-radius: var(--r); color: var(--ink-2); font-size: var(--fs-sm); }
  .toc .item .ic { display: grid; color: var(--ink-3); }
  .toc .item:hover { color: var(--ink); }
  .toc .item[aria-current] { background: var(--sel); color: var(--ink); font-weight: 700; }
  .toc .item[aria-current] .ic { color: var(--lamp); }
  .toc-label { margin: 14px 10px 4px; font-size: var(--fs-xs); font-weight: 600; color: var(--ink-3); text-transform: uppercase; letter-spacing: .04em; }
  .toc .results { background: var(--bg); }
  .find { display: flex; align-items: center; gap: 8px; padding: 0 10px 0 12px; min-height: 36px; margin-bottom: 10px; color: var(--ink-3); background: var(--bg-3); border: 1px solid transparent; border-radius: var(--r-pill); }
  .find:focus-within { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-glow); }
  .find input { flex: 1; min-width: 0; padding: 6px 0; background: none; border: 0; outline: 0; color: var(--ink); font-size: var(--fs-sm); }
  .find input::placeholder { color: var(--ink-3); }
  .clear { display: grid; place-items: center; width: 22px; height: 22px; border-radius: 50%; background: var(--line); color: var(--ink-2); flex: none; }
  .results .nav-row { min-height: 44px; }
  .hit { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .where { display: block; font-size: var(--fs-xs); color: var(--ink-3); }
  .none { margin: 8px 6px; font-size: var(--fs-sm); color: var(--ink-3); }

  /* The page. */
  /* Positioned, so hidden file and colour inputs scroll with the page instead of
     stretching the document: in Den Shell that let the whole page scroll, header and all. */
  .pane, .home { position: relative; }
  .pane { --pane-end: 48px; overflow-y: auto; padding: 28px 28px var(--pane-end); min-width: 0; }
  .pane-inner { width: 100%; max-width: 640px; margin-inline: auto; }
  .wide-pane .pane-inner { max-width: 940px; }
  .pane-inner > :global(p.muted) { margin: 0; max-width: 62ch; }
  .gap-top { margin-top: 20px; }
  .callout { margin-top: 18px; }
  .val { color: var(--ink-3); font-size: var(--fs-sm); }
  .error { color: var(--danger); }

  /* Phones and narrow windows: the list, or one page pushed in with a back chevron. */
  .drill .body { grid-template-columns: minmax(0, 1fr); }
  .drill .head { position: relative; justify-content: space-between; padding: 4px 8px; border-bottom-color: transparent; }
  .back { display: inline-flex; align-items: center; gap: 0; min-height: var(--hit); padding: 0 8px 0 0; margin-inline-start: -2px; border-radius: var(--r-pill); color: var(--lamp); font-size: var(--fs-body); }
  .back:active { background: var(--press); }
  .drill .title:focus { outline: none; }
  .drill .title { position: absolute; left: 50%; transform: translateX(-50%); max-width: calc(100% - 220px); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-body); font-weight: 700; }
  .undo { min-width: 64px; min-height: var(--hit); padding: 0 6px; color: var(--lamp); font-weight: 600; text-align: end; }
  .undo:disabled { color: var(--ink-3); opacity: .6; cursor: default; }
  .drill .pane { --pane-end: max(32px, env(safe-area-inset-bottom)); padding: 4px 16px var(--pane-end); }
  /* The header carries the page's name on a phone. Only page titles: a profile card has its own h2. */
  .drill .pane-inner > :global(h2.display), .drill .pane-inner > :global(.appearance > h2.display) { display: none; }
  .home { overflow-y: auto; padding: 0 16px max(32px, env(safe-area-inset-bottom)); }
  .large { margin: 0 4px 12px; font-size: var(--fs-large); font-weight: 700; line-height: 1.1; }
  .home .find { min-height: 40px; margin-bottom: 18px; }
  .home .find input { font-size: 16px; }
  .gap { height: 22px; }
  .me { display: block; color: inherit; }
  .me:hover { text-decoration: none; }
  .me .nav-row { gap: 14px; min-height: 80px; padding: 12px 16px; }
  .me b { display: block; font-size: var(--fs-title); font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { display: block; font-size: var(--fs-sm); color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tile { display: grid; place-items: center; width: 30px; height: 30px; flex: none; border-radius: calc(var(--r) + 2px); background: color-mix(in srgb, var(--lamp) 16%, var(--bg-3)); color: var(--lamp); }
  .home .slist .nav-row { min-height: 52px; }
  .home .slist > .nav-row + .nav-row { background-position: 58px 0; background-size: calc(100% - 58px) 1px; }
</style>
