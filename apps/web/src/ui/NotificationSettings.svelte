<script lang="ts">
  import { store } from '../lib/store.svelte'
  import { router } from '../lib/router.svelte'
  import { notify } from '../lib/notify.svelte'
  import { push, pushSettings, pushStart } from '../lib/shell-push.svelte'
  import SettingRow from './SettingRow.svelte'
  import RoomIcon from './RoomIcon.svelte'
  import Icon from './Icon.svelte'

  // The iPhone app's page has no Notification at all: nothing to unblock there.
  let perm = $state<NotificationPermission | 'unsupported'>(typeof Notification !== 'undefined' ? Notification.permission : 'unsupported')
  async function enable() { await notify.ask(); perm = Notification.permission }
  // Den Shell asks iOS instead, and the row reads like iOS's own: a value, and a tap that turns it on or opens
  // iOS Settings. Coming back from iOS Settings re-checks (Shell.svelte).
  const phone = $derived(push.status?.permission ?? null)
  const phoneValue = $derived(!phone ? '' : phone !== 'granted' ? 'Off' : push.connected ? 'On' : 'Not connected')
  const phoneFoot = $derived(phone === 'undecided' ? 'Turn them on to hear about mentions and DMs when Den is closed.'
    : phone === 'denied' ? 'Turn on Allow Notifications in iOS Settings.'
    : phone === 'granted' && !push.connected ? 'Den tries again each time you open it.' : '')
  const phoneTap = () => (phone === 'undecided' ? void pushStart() : void pushSettings())
  function toggleSub(id: string) {
    const s = new Set(store.notif.subscribed_channel_ids); s.has(id) ? s.delete(id) : s.add(id)
    store.saveNotif({ subscribed_channel_ids: [...s] })
  }
</script>

<h2 class="display">Notifications</h2>
{#if push.available}
  <div class="slist callout" data-find="permission">
    <button class="nav-row" onclick={phoneTap} aria-label={`Notifications on this iPhone, ${phoneValue}`}><span class="grow">Notifications on this iPhone</span><span class="val">{phoneValue}</span><span class="chev"><Icon name="chevron" size={14} /></span></button>
  </div>
  {#if phoneFoot}<p class="list-foot">{phoneFoot}</p>{/if}
{:else if perm !== 'granted'}
  <div class="slist callout">
    <SettingRow find="permission" label={perm === 'unsupported' ? 'Notifications on this device' : 'Desktop notifications'} hint={perm === 'unsupported' ? "Phone notifications aren't available yet. Mentions and DMs wait for you in the Inbox." : perm === 'denied' ? 'Blocked in your browser settings. Allow them there for this site.' : 'Off. Turn them on to hear about mentions and DMs while Den is in the background.'}>
      {#snippet control()}{#if perm === 'default'}<button class="btn lit" onclick={enable}>Turn on</button>{/if}{/snippet}
    </SettingRow>
  </div>
{/if}
<h3 class="list-head">Always</h3>
<!-- Notifications already on: a search for them lands here. -->
<div class="slist" data-find={perm === 'granted' && !push.available ? 'permission' : undefined}>
  <SettingRow find="mentions" label="When someone mentions me">
    {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="When someone mentions me" checked={store.notif.mentions} onchange={(e) => store.saveNotif({ mentions: e.currentTarget.checked })} />{/snippet}
  </SettingRow>
  <SettingRow find="dms" label="Direct messages">
    {#snippet control()}<input class="toggle" type="checkbox" role="switch" aria-label="Direct messages" checked={store.notif.dms} onchange={(e) => store.saveNotif({ dms: e.currentTarget.checked })} />{/snippet}
  </SettingRow>
  <a class="nav-row" href="/settings/sounds" onclick={(e) => { e.preventDefault(); router.go('/settings/sounds') }}><span class="grow">Sounds</span><span class="chev"><Icon name="chevron" size={14} /></span></a>
</div>
<h3 class="list-head">Rooms you follow</h3>
<div class="slist" data-find="follow">
  {#each store.textChannels as c (c.id)}
    <div class="room">
      <RoomIcon channel={c} size={28} />
      <span class="name">{c.name}</span>
      <input class="toggle" type="checkbox" role="switch" aria-label={`Follow ${c.name}`} checked={store.notif.subscribed_channel_ids.includes(c.id)} onchange={() => toggleSub(c.id)} />
    </div>
  {/each}
</div>
<p class="list-foot">Every message in a room you follow notifies you.</p>

<style>
  .callout { margin-top: 18px; }
  .room { display: flex; align-items: center; gap: 12px; min-height: 48px; padding: 8px 16px; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* The divider starts at the name, as iOS insets it past a row's icon. */
  .slist > .room + .room { background-position: 56px 0; background-size: calc(100% - 56px) 1px; }
  @media (pointer: coarse) { .room { min-height: 52px; } }
</style>
