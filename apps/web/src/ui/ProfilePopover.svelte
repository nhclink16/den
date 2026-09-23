<script lang="ts">
  // The profile card as a light-dismiss popover beside whatever opened it: an avatar,
  // a name, a mention or a row in the people list. Escape and outside clicks close
  // it (the popover API does both), and focus returns to the opener.
  import { tick } from 'svelte'
  import { profileCard } from '../lib/people.svelte'
  import { router } from '../lib/router.svelte'
  import UserCard from './UserCard.svelte'

  let node = $state<HTMLDivElement>()

  function place() {
    const anchor = profileCard.anchor
    if (!node || !anchor?.isConnected) return
    const a = anchor.getBoundingClientRect(), box = node.getBoundingClientRect()
    const vw = document.documentElement.clientWidth, vh = document.documentElement.clientHeight
    // Beside the opener if it fits, else on the other side, else over it.
    let left = a.right + 10
    if (left + box.width > vw - 8) left = a.left - box.width - 10
    if (left < 8) left = Math.max(8, Math.min(a.left, vw - box.width - 8))
    const top = Math.max(8, Math.min(a.top - 12, vh - box.height - 8))
    node.style.left = `${left}px`; node.style.top = `${top}px`
  }

  $effect(() => {
    const id = profileCard.userId
    if (!node) return
    if (!id) { if (node.matches(':popover-open')) node.hidePopover(); return }
    if (!node.matches(':popover-open')) node.showPopover()
    void tick().then(() => { place(); node?.querySelector<HTMLElement>('button, [href]')?.focus({ preventScroll: true }) })
  })

  function toggled(e: ToggleEvent) {
    if (e.newState !== 'closed') return
    const opener = profileCard.anchor
    profileCard.close()
    if (opener?.isConnected && !node?.contains(document.activeElement)) opener.focus({ preventScroll: true })
  }

  async function message() {
    const s = profileCard.instance, id = profileCard.userId
    if (!s || !id) return
    profileCard.close()
    const c = await s.openDm([id])
    router.go(`/c/${c.id}`)
  }
  function edit() { profileCard.close(); router.go('/settings/profile') }
</script>

<svelte:window onresize={place} />

<div class="pop" popover="auto" bind:this={node} ontoggle={toggled}>
  {#if profileCard.userId && profileCard.instance}
    {#key profileCard.userId}
      <UserCard userId={profileCard.userId} instance={profileCard.instance} onmessage={message} onedit={edit} />
    {/key}
  {/if}
</div>

<style>
  .pop {
    position: fixed; inset: auto; margin: 0; padding: 0; border: 0; background: none; color: var(--ink);
    overflow: visible; border-radius: var(--r-lg);
    box-shadow: 0 24px 60px -16px var(--shadow-lg), 0 2px 6px var(--shadow);
  }
  @media (prefers-reduced-motion: no-preference) {
    .pop:popover-open { animation: card-in .18s var(--ease-out); }
  }
  @keyframes card-in { from { opacity: 0; transform: translateY(4px) scale(.98); } }
</style>
