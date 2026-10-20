<script lang="ts">
  import { onDestroy, tick, untrack } from 'svelte'
  import { forgetShownOnce } from '../lib/settings-secrets.svelte'
  import SidebarToggle from './SidebarToggle.svelte'
  import HomeButton from './HomeButton.svelte'
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { api } from '../lib/api'
  import { customizer } from '../lib/customizer.svelte'
  import { visibleSections, findSettings, reveal, type Hit, type Section } from '../lib/settings-search'
  import { soundEvents } from '../lib/sounds'
  import NavBar, { trouble } from './NavBar.svelte'
  import ProfileSettings from './ProfileSettings.svelte'
  import NotificationSettings from './NotificationSettings.svelte'
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
  import DesktopApp from './DesktopApp.svelte'
  import { isDesktop } from '../lib/desktop'
  import { native } from '../lib/native'
  import { shell } from '../lib/shell-call'

  /** `narrow`: one thing at a time. The list of sections, or a page pushed in over it, like iPhone Settings. */
  let { section, narrow }: { section?: string; narrow: boolean } = $props()
  const admin = $derived(store.me?.role === 'admin')
  // Den Shell, the iPhone app: no computer pages, and the developer ones behind one Developer row.
  const phoneApp = !!shell
  // A phone's iPhone top bar; the desktop app keeps its own header, which drags the window.
  const bar = $derived(narrow && !native)
  // Everyday settings first. Machines, access and keys are for people running
  // agents and terminals, so they sit last under Advanced, still one tap away.
  const sections = $derived(visibleSections(admin, isDesktop(), phoneApp))
  // A wide window always shows a page; a phone shows the list until you pick one.
  const current = $derived(sections.some((s) => s.id === section) ? section! : narrow ? null : 'profile')
  // A page inside a page: one sound moment (/settings/sounds/mention).
  const sub = $derived(router.route.name === 'settings' ? router.route.sub : undefined)
  const moment = $derived(current === 'sounds' ? soundEvents.find((e) => e.id === sub) : undefined)
  // ...or one room or category (/settings/rooms/:id).
  const place = $derived(current === 'rooms' && sub ? (store.textChannels.find((c) => c.id === sub) ?? store.categories.find((c) => c.id === sub)) : undefined)
  const subTitle = $derived(moment?.name ?? place?.name)
  const title = $derived(subTitle ?? sections.find((s) => s.id === current)?.label ?? 'Settings')
  const groups = $derived((['you', 'app', 'account', 'server', phoneApp ? 'developer' : 'advanced'] as const).map((g) => ({ g, list: sections.filter((s) => s.group === g) })).filter((x) => x.list.length))
  const groupLabel = { you: '', app: '', account: '', server: 'Server', advanced: 'Advanced', developer: '' } as const
  /** Inside Developer on the iPhone app: Back goes there, not to the list. */
  const under = $derived(subTitle ? current : phoneApp && sections.find((s) => s.id === current)?.group === 'advanced' ? 'developer' : null)
  const backLabel = $derived(under ? sections.find((s) => s.id === under)?.label ?? 'Settings' : 'Settings')

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
  /** The page a pending search is looking on. */
  let pendingOn: string | null = null
  function open(id: string, find?: string) {
    pending = find ?? null; pendingOn = id
    // Only a page opened from the list (or Developer) goes back to it with Back; one opened from another page swaps in the list.
    if (id !== section || sub) router.go(`/settings/${id}`, false, current === 'developer' ? '/settings/developer' : narrow && !current ? '/settings' : undefined)
    // Already showing it (a computer shows Profile at /settings): nothing changes to look again, so look now.
    if (id === current) seek()
  }
  const go = (id: string) => (e: MouseEvent) => { e.preventDefault(); open(id) }
  function toList() {
    const to = under ? `/settings/${under}` : '/settings'
    if (router.parent === to) router.back()
    else router.go(to, true)
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
  $effect(() => { void current; void subTitle; pane?.scrollTo({ top: 0 }) })
  // Pushing a page in moves keyboard focus to its title; coming back puts it on the row you left from.
  let heading = $state<HTMLElement>(), home = $state<HTMLElement>(), last: string | null = null
  $effect(() => {
    const now = current && (subTitle ? `${current}/${sub}` : current), from = last
    last = now
    // Leaving the page a search is still looking on (Back, a link) ends that search.
    if (now !== pendingOn) untrack(() => { pending = null })
    // A search landing focuses the row it found instead.
    if (!narrow || now === from || untrack(() => pending)) return
    void tick().then(() => {
      // Back from a page inside this one (a sound, a room, a developer page): onto the row it was opened from.
      const child = now && from && (from.startsWith(`${now}/`) || now === 'developer') ? pane?.querySelector<HTMLElement>(`a[href="/settings/${from}"]`) : null
      if (child) child.focus()
      else if (now) pageTitle()?.focus({ preventScroll: true })
      // The list starts at its top again, so let the row scroll into view.
      else if (from) home?.querySelector<HTMLElement>(`a[href="/settings/${from}"]`)?.focus()
    })
  })

  // --- search ---
  let q = $state('')
  // The width at which Profile drops its computer-only rows.
  const hits = $derived(findSettings(q, sections, admin, phoneApp, matchMedia('(max-width: 900px)').matches))
  function pick(h: Hit) {
    q = ''; open(h.section.id, h.entry?.find)
    // A whole section has no row to land on, and the result just went away: focus the page.
    if (!h.entry) void tick().then(focusPage)
  }
  /** The page's name in the top bar on a phone. */
  const pageTitle = () => (bar ? pane?.querySelector<HTMLElement>('.navbar h1') : heading)
  /** Keyboard focus to the top of the page: the header title on a phone, the page's own title on a computer. */
  function focusPage() {
    pane?.scrollTo({ top: 0 })
    const title = narrow ? pageTitle() : pane?.querySelector<HTMLElement>('h2')
    if (!title) return
    title.tabIndex = -1; title.focus({ preventScroll: true })
  }
  function searchKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && hits[0]) { e.preventDefault(); pick(hits[0]) }
    if (e.key === 'Escape' && q) { e.preventDefault(); q = '' }
  }

  let pluginErr = $state('')

  // The iPhone app has no desktop download page: it hands the link to a computer instead.
  let shared = $state('')
  async function shareComputer() {
    const url = `${store.origin || location.origin}/settings/desktop`
    // The link shows straight away, whatever the share sheet or clipboard then does (in Xcode Cloud's simulator,
    // neither showed anything within 3 s).
    shared = `On your computer, open ${url.replace(/^https?:\/\//, '')}.`
    if (navigator.share) {
      try { await navigator.share({ title: 'Den for your computer', text: 'Get Den for your computer: sign in, then Settings › Desktop app.', url }); return }
      catch (e) { if ((e as Error).name === 'AbortError') return }
    }
    try { await navigator.clipboard.writeText(url); shared = 'Link copied. Open it on your computer.' }
    catch { /* The link is on screen already. */ }
  }

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

<!-- `edge`: on a phone the bar reaches up under the status bar (Shell). -->
<section class="settings" class:drill={narrow} class:list-view={narrow && !current} class:edge={bar}>
  {#if !bar}
  <header class="head">
    {#if narrow && current}
      <button class="back" onclick={toList}><Icon name="back" size={22} /><span>{backLabel}</span></button>
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
  {/if}

  <div class="body">
    {#if narrow && !current}
      <!-- The phone's Settings list: you at the top, then every section. -->
      <div class="home" bind:this={home}>
        {#if bar}
          <NavBar title="Settings" large status={trouble(store.connected)}>{#snippet leading()}<HomeButton />{/snippet}</NavBar>
        {:else}<h1 class="display large">Settings</h1>{/if}
        <div class="home-body">
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
              {#if g === 'app' && phoneApp}{@render computer()}{/if}
            </nav>
            {#if g === 'app' && shared}<p class="list-foot" role="status">{shared}</p>{/if}
          {/each}
        {/if}
        </div>
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
              {#if g === 'app' && phoneApp}{@render computer(false)}{/if}
            {/each}
          {/if}
        </nav>
      {/if}

      <div class="pane" bind:this={pane} class:wide-pane={current === 'profile' || current === 'appearance'}>
        {#if bar}
          <NavBar {title} status={trouble(store.connected)}>
            {#snippet leading()}<HomeButton onback={toList} label={backLabel} aria={backLabel} />{/snippet}
            <!-- Undo shows once there is something to undo. -->
            {#snippet trailing()}{#if current === 'appearance' && customizer.changed}<button class="text-btn" onclick={() => customizer.undo()}>Undo</button>{/if}{/snippet}
          </NavBar>
        {/if}
        <div class="pane-inner">
        {#if current === 'profile'}
          <ProfileSettings />
        {:else if current === 'appearance'}
          <Appearance />
        {:else if current === 'machines' || current === 'access'}
          <MachineSettings section={current} />
        {:else if current === 'notifications'}
          <NotificationSettings />
        {:else if current === 'sounds'}
          {@render crumb()}
          <SoundSettings moment={moment?.id} />
        {:else if current === 'voice'}
          <VoiceSettings />
        {:else if current === 'spotify'}
          <SpotifySettings />
        {:else if current === 'plugins'}
          <h2 class="display">Extras</h2>
          <div class="slist gap-top">
            <SettingRow find="canvas" label="Canvas" hint="A shared drawing board anyone in a room can open.">
              {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="Canvas" checked={store.settings.canvas_enabled} onchange={async (e) => { try { store.settings = await api.put('/settings', { canvas_enabled: e.currentTarget.checked }); pluginErr = '' } catch (err) { pluginErr = (err as Error).message } }} />{/snippet}
            </SettingRow>
          </div>
          <p class="list-foot">For everyone on this server.</p>
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
          <p class="list-foot" data-find="shortcuts">Jump anywhere with <kbd>Ctrl+K</kbd>. Edit your last message with <kbd>↑</kbd> in an empty message box.{#if narrow}{' '}On a phone the room list is your home screen, so these apply on wider screens.{/if}</p>

        {:else if current === 'keys' || current === 'agents'}
          <KeySettings section={current} />
        {:else if current === 'members'}
          <MemberSettings />
        {:else if current === 'invites'}
          <InviteSettings />
        {:else if current === 'rooms'}
          {@render crumb()}
          <RoomSettings {sub} />
        {:else if current === 'desktop'}
          <DesktopApp />
        {:else if current === 'developer'}
          <h2 class="display">Developer</h2>
          <nav class="slist gap-top" aria-label="Developer">
            {#each sections.filter((s) => s.group === 'advanced') as s (s.id)}{@render navItem(s, true)}{/each}
          </nav>
          <p class="list-foot">Keys for scripts and the <code>den</code> CLI, agents that talk in Den, and who can use your terminals.</p>
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

{#snippet crumb()}
  {#if subTitle && !narrow}<button class="crumb" onclick={toList}><Icon name="back" size={16} />{backLabel}</button>{/if}
{/snippet}

{#snippet computer(tile = true)}
  <button class="nav-row" class:item={!tile} onclick={shareComputer}>
    <span class:tile class="ic"><Icon name="monitor" size={tile ? 17 : 16} /></span>
    <span class="grow">Den on your computer</span>
    {#if tile}<span class="chev"><Icon name="chevron" size={14} /></span>{/if}
  </button>
{/snippet}

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
  @media (hover: hover) { .toc .item:hover { color: var(--ink); } }
  .toc .item[aria-current] { background: var(--sel); color: var(--ink); font-weight: 700; }
  .toc .item[aria-current] .ic { color: var(--lamp); }
  .toc-label { margin: 14px 10px 4px; font-size: var(--fs-xs); font-weight: 600; color: var(--ink-3); text-transform: uppercase; letter-spacing: .04em; }
  .toc .results { background: var(--bg); }
  .find { display: flex; align-items: center; gap: 8px; padding: 0 10px 0 12px; min-height: 36px; margin-bottom: 10px; color: var(--ink-3); background: var(--bg-3); border: 1px solid transparent; border-radius: var(--r-pill); }
  :global(:where(:root:not([data-input='touch']))) .find:focus-within { border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-glow); }
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
  .val { color: var(--ink-3); font-size: var(--fs-sm); }
  .error { color: var(--danger); }
  /* The way back from a sound's own page where there is no top bar to carry it. */
  .crumb { display: inline-flex; align-items: center; gap: 2px; margin: 0 0 8px -4px; color: var(--lamp); font-size: var(--fs-sm); }

  /* Phones and narrow windows: the list, or one page pushed in with a back chevron. */
  .drill .body { grid-template-columns: minmax(0, 1fr); }
  .drill .head { position: relative; justify-content: space-between; padding: 4px 8px; border-bottom-color: transparent; }
  .back { display: inline-flex; align-items: center; gap: 0; min-height: var(--hit); padding: 0 8px 0 0; margin-inline-start: -2px; border-radius: var(--r-pill); color: var(--lamp); font-size: var(--fs-body); }
  .back:active { background: var(--press); }
  .drill .title:focus { outline: none; }
  .drill .title { position: absolute; left: 50%; transform: translateX(-50%); max-width: calc(100% - 220px); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-body); font-weight: 700; }
  .undo { min-width: 64px; min-height: var(--hit); padding: 0 6px; color: var(--lamp); font-weight: 600; text-align: end; }
  .undo:disabled { color: var(--ink-3); opacity: .6; cursor: default; }
  .drill .pane { --pane-end: max(32px, env(safe-area-inset-bottom)); padding: 0 0 var(--pane-end); }
  .drill .pane-inner { padding: 4px 16px 0; }
  /* The header carries the page's name on a phone. Only page titles: a profile card has its own h2. */
  .drill .pane-inner > :global(h2.display), .drill .pane-inner > :global(.appearance > h2.display) { display: none; }
  .home { overflow-y: auto; }
  .home-body { padding: 0 16px max(32px, env(safe-area-inset-bottom)); }
  .large { margin: 0 20px 12px; font-size: var(--fs-large); font-weight: 700; line-height: 1.1; }
  .home .find { min-height: 40px; margin-bottom: 18px; }
  .home .find input { font-size: 16px; }
  .gap { height: 22px; }
  .me { display: block; color: inherit; }
  @media (hover: hover) { .me:hover { text-decoration: none; } }
  .me .nav-row { gap: 14px; min-height: 80px; padding: 12px 16px; }
  .me b { display: block; font-size: var(--fs-title); font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub { display: block; font-size: var(--fs-sm); color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tile { display: grid; place-items: center; width: 30px; height: 30px; flex: none; border-radius: calc(var(--r) + 2px); background: color-mix(in srgb, var(--lamp) 16%, var(--bg-3)); color: var(--lamp); }
  .home .slist .nav-row { min-height: 52px; }
  .edge .home .find { margin-top: 2px; }
  .home .slist > .nav-row + .nav-row { background-position: 58px 0; background-size: calc(100% - 58px) 1px; }
</style>
