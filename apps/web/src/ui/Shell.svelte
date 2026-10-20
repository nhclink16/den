<script lang="ts">
  import { untrack } from 'svelte'
  import { desktop } from '../lib/desktop.svelte'
  import { call } from '../lib/call.svelte'
  import CallDock from './CallDock.svelte'
  import CallView from './CallView.svelte'
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import Sidebar from './Sidebar.svelte'
  import ChannelView from './ChannelView.svelte'
  import Members from './Members.svelte'
  import Inbox from './Inbox.svelte'
  import Settings from './Settings.svelte'
  import SpotifyCallback from './SpotifyCallback.svelte'
  import Palette from './Palette.svelte'
  import ProfilePopover from './ProfilePopover.svelte'
  import Customizer from './Customizer.svelte'
  import { customizer } from '../lib/customizer.svelte'
  import { objects } from '../lib/objects.svelte'
  import Search from './Search.svelte'
  import ServerView from './ServerView.svelte'
  import { notify } from '../lib/notify.svelte'
  import { listenForPush, pushBadge, pushStart } from '../lib/shell-push.svelte'
  import { listenForSelection } from '../lib/haptics'
  import Icon from './Icon.svelte'
  import PushAsk from './PushAsk.svelte'

  let palette = $state(false)
  // Narrow screens show one thing at a time: the room list is the home screen,
  // and every other page has a back button to it.
  let narrow = $state(matchMedia('(max-width: 900px)').matches)

  $effect(() => {
    const mq = matchMedia('(max-width: 900px)')
    const fn = () => { narrow = mq.matches }
    mq.addEventListener('change', fn)
    return () => mq.removeEventListener('change', fn)
  })
  // A wide window has the list beside the room, so home is the first room.
  $effect(() => {
    if (narrow || router.route.name !== 'home') return
    const first = store.textChannels.find((c) => c.kind === 'text') || store.dms[0]
    if (first) untrack(() => router.go(`/c/${first.id}`, true))
  })

  // Remember the last place outside Settings, for the customizer to open over.
  $effect(() => { if (router.route.name !== 'settings') customizer.lastPlace = location.pathname + location.search })

  function key(e: KeyboardEvent) {
    if ((e.target as HTMLElement).closest('[data-terminal-focus="true"]')) return
    const mod = e.ctrlKey || e.metaKey
    if (mod && e.key.toLowerCase() === 'k') { e.preventDefault(); palette = !palette }
    else if (mod && e.key === '\\') { e.preventDefault(); store.saveLayout({ sidebar: !store.layout.sidebar }) }
    else if (mod && e.shiftKey && e.key.toLowerCase() === 'm') { e.preventDefault(); store.saveLayout({ members: !store.layout.members }) }

  }

  const currentChannel = $derived(router.route.name === 'channel' ? store.channel(router.route.id) : undefined)
  const showMembers = $derived(!narrow && store.layout.members && !!currentChannel)
  const showSidebar = $derived(!narrow && store.layout.sidebar)
  // The expanded call brings its own toolbar, but only where a call view renders:
  // a room under an expanded canvas or terminal, or a room that isn't there, has
  // none, and the dock keeps Mute and Leave then.
  const callShown = $derived(router.route.name !== 'channel' || (!!currentChannel && !objects.expanded))
  // A phone shows the call as a pill at the top while you chat (CallView), so the
  // bar at the bottom only carries the Jam then, or the buttons where no call view is.
  const callDock = $derived(!!call.channel && !showSidebar && !(narrow && callShown && !call.expanded))
  const backTo = $derived(({ inbox: 'Inbox', search: 'Search', settings: 'Settings' } as Record<string, string>)[router.route.name] ?? '')
  // Appearance covers the whole screen on phones, so the app behind it is out of reach.
  let phone = $state(matchMedia('(max-width: 650px)').matches)
  $effect(() => {
    const mq = matchMedia('(max-width: 650px)')
    const fn = () => { phone = mq.matches }
    mq.addEventListener('change', fn)
    return () => mq.removeEventListener('change', fn)
  })

  $effect(() => {
    const down = (e: KeyboardEvent) => call.keydown(e), up = (e: KeyboardEvent) => call.keyup(e)
    const release = () => { if (!desktop.global) call.setHeld(false) }
    window.addEventListener('keydown', down); window.addEventListener('keyup', up)
    window.addEventListener('blur', release); document.addEventListener('visibilitychange', release)
    return () => { window.removeEventListener('keydown', down); window.removeEventListener('keyup', up); window.removeEventListener('blur', release); document.removeEventListener('visibilitychange', release); void call.leave() }
  })
  // Den Shell's notifications: registered for whoever just signed in, and again each time Den comes back.
  $effect(() => untrack(() => {
    const stop = listenForPush()
    void pushStart({ askFirst: true })
    const back = () => { if (document.visibilityState === 'visible') void pushStart({ askFirst: true }) }
    document.addEventListener('visibilitychange', back)
    return () => { stop(); document.removeEventListener('visibilitychange', back) }
  }))
  $effect(() => pushBadge(store.readState.values()))
  $effect(listenForSelection)
  // Den Shell keeps a call running through a page reload; show it again once the rooms have loaded.
  $effect(() => { if (store.channels.length) untrack(() => void call.resumeShell()) })
  // Per-device avatar preferences, read by Avatar through root attributes.
  $effect(() => {
    const root = document.documentElement
    if (store.layout.avatarShape === 'theme') delete root.dataset.avatarShape
    else root.dataset.avatarShape = store.layout.avatarShape
    root.dataset.presence = store.layout.presence
  })
  // A call that couldn't connect says so for a moment over whatever is on screen, then goes, like an iPhone toast.
  $effect(() => {
    if (!call.error) return
    const timer = setTimeout(() => (call.error = ''), 6000)
    return () => clearTimeout(timer)
  })
  notify.attach()
</script>

<svelte:window onkeydown={key} />

<div class="shell bg-host scope-app" class:narrow class:no-sidebar={!showSidebar} class:no-members={!showMembers} inert={(customizer.open && phone) || undefined}>
  {#if showSidebar}
    <aside class="sidebar bg-host scope-sidebar"><Sidebar /></aside>
  {/if}

  <main class="main bg-host scope-chat">
    {#if call.blurNotice}<div class="call-status" role="status"><span>{call.blurNotice}</span><button class="btn quiet" onclick={() => (call.blurNotice = '')}>Dismiss</button></div>{/if}
    {#if call.reconnecting}<div class="call-status" role="status">Reconnecting to the call…</div>{/if}
    {#if call.audioBlocked}<div class="call-status" role="status"><span>Your browser blocked the call audio.</span><button class="btn lit" onclick={() => call.startAudio()}>Play call audio</button></div>{/if}
    {#if call.channel && router.route.name !== 'channel'}<CallView {narrow} back={backTo} titled />{/if}
    {#if !call.channel || !call.expanded || router.route.name === 'channel'}
    {#if router.route.name === 'channel'}
      {#if currentChannel}
        {#key store.origin + currentChannel.id}
          <ChannelView channel={currentChannel} {narrow} />
        {/key}
      {:else}
        <div class="empty"><p class="muted">That room isn't here.</p><a href="/" onclick={(e) => { e.preventDefault(); router.go('/') }}>Back to the den</a></div>
      {/if}
    {:else if router.route.name === 'inbox'}
      <Inbox {narrow} />
    {:else if router.route.name === 'search'}
      <Search q={router.route.q} channelId={router.route.channel} {narrow} />
    {:else if router.route.name === 'settings'}
      <Settings section={router.route.section} {narrow} />
    {:else if router.route.name === 'server'}
      <ServerView slug={router.route.slug} {narrow} />
    {:else if router.route.name === 'home' && narrow}
      <!-- The phone home is the room list, so it takes the sidebar's wallpaper. -->
      <div class="home bg-host scope-sidebar edge"><Sidebar phone /></div>
    {:else if router.route.name === 'spotify-callback'}
      <SpotifyCallback route={router.route} />
    {/if}
    {/if}
    {#if callDock}<CallDock controls={!(callShown && (narrow || call.expanded))} />{/if}
  </main>

  {#if showMembers && currentChannel}
    <aside class="members"><Members channel={currentChannel} /></aside>
  {/if}
</div>

{#if call.error}<div class="call-error" role="alert"><button title="Dismiss" onclick={() => (call.error = '')}><Icon name="phone" />{call.error}</button></div>{/if}
{#if store.toast}<div class="toast" role="status">{store.toast}<button aria-label="Dismiss notification" onclick={() => store.toast = ''}>×</button></div>{/if}

{#if palette}<Palette onclose={() => (palette = false)} />{/if}
<ProfilePopover />
<Customizer />
<PushAsk />

<style>
  .toast { position: fixed; bottom: calc(24px + env(safe-area-inset-bottom, 0px)); left: 50%; transform: translateX(-50%); z-index: 50; max-width: calc(100% - 24px); padding: 9px 10px 9px 14px; border: 1px solid var(--line-strong); border-radius: var(--r-lg); background: var(--bg-2); display: flex; align-items: center; gap: 12px; box-shadow: 0 14px 40px -10px var(--shadow-lg); }
  @media (prefers-reduced-motion: no-preference) { .toast { animation: toast-in .24s var(--ease-out); } }
  @keyframes toast-in { from { opacity: 0; transform: translateX(-50%) translateY(8px); } }
  .toast button { border-radius: var(--r); color: var(--ink-2); }
  @media (hover: hover) { .toast button:hover { color: var(--ink); background: var(--bg-3); } }
  .toast button { min-width: 24px; min-height: 24px; }
  .call-error {
    position: fixed; top: calc(env(safe-area-inset-top, 0px) + 8px); left: 50%; transform: translateX(-50%); z-index: 50;
    width: max-content; max-width: calc(100% - 32px);
    border-radius: var(--r-pill); background: var(--bg-2); box-shadow: 0 0 0 1px var(--line), 0 10px 30px -8px var(--shadow-lg);
  }
  .call-error button { display: flex; align-items: center; gap: 8px; min-height: 44px; padding: 10px 18px; border-radius: inherit; color: var(--ink); font-size: var(--fs-sm); font-weight: 600; text-align: start; }
  .call-error :global(svg) { flex: none; color: var(--danger); }
  @media (prefers-reduced-motion: no-preference) { .call-error { animation: drop-in .32s var(--ease-out); } }
  @keyframes drop-in { from { opacity: 0; transform: translateX(-50%) translateY(-16px); } }
  .call-status { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 8px 12px; color: var(--lamp); background: var(--bg-2); border-bottom: 1px solid var(--line); }
  /* Den Shell draws edge to edge: the app keeps clear of the notch and, in landscape, the sides. The bottom is
     each screen's own, so its lists can scroll under the home indicator. */
  .shell {
    height: 100%; display: grid;
    padding: env(safe-area-inset-top, 0px) env(safe-area-inset-right, 0px) 0 env(safe-area-inset-left, 0px);
    grid-template-columns: var(--sidebar-w) minmax(0, 1fr) var(--members-w);
    grid-template-areas: 'sidebar main members';
  }
  .shell.no-sidebar { grid-template-columns: minmax(0, 1fr) var(--members-w); grid-template-areas: 'main members'; }
  .shell.no-members { grid-template-columns: var(--sidebar-w) minmax(0, 1fr); grid-template-areas: 'sidebar main'; }
  .shell.no-sidebar.no-members { grid-template-columns: minmax(0, 1fr); grid-template-areas: 'main'; }
  .sidebar { grid-area: sidebar; background: var(--bg-2); border-right: 1px solid var(--line); min-height: 0; }
  .main { grid-area: main; min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .members { grid-area: members; background: var(--bg-2); border-left: 1px solid var(--line); min-height: 0; overflow-y: auto; }
  .empty { flex: 1; display: grid; place-content: center; text-align: center; gap: 6px; }

  .shell.narrow { grid-template-columns: minmax(0, 1fr); grid-template-areas: 'main'; }
  /* A phone screen whose NavBar is the first thing on screen (`.edge`) reaches up under the status bar, so its
     content can scroll under the bar's blur; the bar keeps its own row clear with --nav-inset. Anything above
     it (a call pill, a status strip) keeps the shell's padding instead. */
  .shell.narrow:has(> .main > :global(.edge:first-child)) { padding-top: 0; }
  .shell.narrow > .main > :global(.edge:first-child) { --nav-inset: env(safe-area-inset-top, 0px); }
  .home { flex: 1; min-height: 0; }
</style>
