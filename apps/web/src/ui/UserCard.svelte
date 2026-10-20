<script lang="ts">
  // A person at a glance: picture, name, what they are up to, and a way to talk.
  // The same card is the popover in chat and the live preview in Profile settings,
  // where `draft` overlays unsaved edits on the saved profile. `sheet` is a phone's
  // bottom sheet: presence as the dot on the picture, and a row of round actions.
  import { store, type Store } from '../lib/store.svelte'
  import { mediaUrl } from '../lib/native'
  import { liveStatus } from '../lib/status'
  import { personHue } from '../lib/people.svelte'
  import { isAnimated, reducedMotion, stillUrl } from '../lib/motion.svelte'
  import type { User } from '../lib/types'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import { shownActivities, activityVerb, activitySince, activityIcon } from '../lib/activity'
  import { disclosurePopover } from '../lib/disclosure-popover'

  let { userId, instance = store, draft, onmessage, onedit, oncall, sheet = false }: {
    userId: string; instance?: Store; draft?: Partial<User>
    onmessage?: () => void; onedit?: () => void; oncall?: () => void; sheet?: boolean
  } = $props()

  const saved = $derived(instance.user(userId))
  const user = $derived(saved ? { ...saved, ...draft } as User : undefined)
  const name = $derived(user ? user.display_name || user.username : 'Someone')
  const online = $derived(instance.online.has(userId))
  const hue = $derived(personHue(userId, user))
  // Re-checked every half minute so a status that times out while the card is open goes.
  let now = $state(Date.now())
  $effect(() => { const id = setInterval(() => (now = Date.now()), 30_000); return () => clearInterval(id) })
  const status = $derived(liveStatus(user?.status, now))
  const me = $derived(userId === instance.me?.id)
  const doing = $derived(shownActivities(instance.activities.get(userId)))
  // The half-minute `now` above also moves "for 12m" while the card is open.
  // A GIF banner plays on the card, unless the system asks for less motion: then only on hover.
  let hovered = $state(false)
  const banner = $derived(user?.banner_url && isAnimated(user.banner_url) && reducedMotion.current && !hovered ? stillUrl(user.banner_url) : user?.banner_url)
  let copied = $state(false)
  async function copyName(e: Event) {
    const menu = (e.currentTarget as HTMLElement).closest('details')
    try { await navigator.clipboard.writeText(`@${user!.username}`); copied = true; setTimeout(() => { copied = false; menu?.removeAttribute('open') }, 900) } catch { menu?.removeAttribute('open') }
  }
</script>

{#snippet status_()}
  {#if status}
    <p class="status">{#if status.emoji}<span class="emoji">{status.emoji}</span>{/if}<span>{status.text}</span></p>
  {/if}
{/snippet}

{#if user}
  <article class="card" class:sheet style="--hue:{hue}" aria-label={`${name}'s profile`} data-hover-animates onpointerenter={() => (hovered = true)} onpointerleave={() => (hovered = false)}>
    <div class="banner" class:image={!!user.banner_url}>
      {#if banner}<img src={mediaUrl(banner, instance.origin)} alt="" />{/if}
    </div>
    <div class="face"><Avatar {userId} {instance} size={sheet ? 88 : 76} animate /></div>

    <div class="body">
      <h2 class="display">{name}</h2>
      <p class="handle">
        <span>@{user.username}</span>
        {#if user.bot}<span class="tag lit"><Icon name="bot" size={11} />Agent</span>{/if}
        {#if user.role === 'admin'}<span class="tag">Admin</span>{/if}
      </p>
      {#if sheet}
        {@render status_()}
        <div class="actions">
          {#if me && onedit}
            <button class="act" onclick={onedit}><span class="ic"><Icon name="edit" size={22} /></span>Edit</button>
          {:else if !me}
            {#if onmessage}<button class="act" aria-label={`Message ${name}`} onclick={onmessage}><span class="ic"><Icon name="thread" size={22} /></span>Message</button>{/if}
            {#if oncall && !user.bot}<button class="act" aria-label={`Call ${name}`} onclick={oncall}><span class="ic"><Icon name="phone" size={22} /></span>Call</button>{/if}
            <details class="act-more">
              <summary class="act" aria-label="More"><span class="ic"><Icon name="more" size={22} /></span>More</summary>
              <div class="menu" popover="auto" use:disclosurePopover><button onclick={copyName}><Icon name={copied ? 'check' : 'copy'} size={18} />{copied ? 'Copied' : 'Copy Username'}</button></div>
            </details>
          {/if}
        </div>
      {:else}
        <p class="presence" class:here={online}><span class="pip" aria-hidden="true"></span>{online ? 'Here now' : 'Away'}</p>
      {/if}

      {#if doing.length}
        <ul class="doing" aria-label="Activity">
          {#each doing as a (a.slot + a.name)}
            <li class:live={a.kind === 'playing'}>
              <span class="art" aria-hidden="true">{#if a.image_url}<img src={a.image_url} alt="" referrerpolicy="no-referrer" />{:else}<Icon name={activityIcon[a.kind]} size={18} />{/if}</span>
              <span class="what">
                <span class="verb">{activityVerb(a.kind)}</span>
                <span class="thing">{a.name}</span>
                <span class="more">{#if a.details}{a.details}{' · '}{/if}{activitySince(a.started_at, now)}</span>
              </span>
            </li>
          {/each}
        </ul>
      {/if}

      {#if !sheet}{@render status_()}{/if}
      {#if user.bio}
        <!-- Plain text by contract: never Markdown or HTML. -->
        <p class="bio">{user.bio}</p>
      {/if}

      {#if sheet}<!-- The actions are up under the name. -->
      {:else if me && onedit}
        <button class="btn" onclick={onedit}><Icon name="edit" size={14} /> Edit profile</button>
      {:else if !me && onmessage}
        <button class="btn lit" onclick={onmessage}><Icon name="send" size={14} /> Message {user.display_name || user.username}</button>
      {/if}
    </div>
  </article>
{/if}

<style>
  .card {
    width: 300px; max-width: 100%; overflow: hidden;
    background: var(--bg-2); border: 1px solid var(--line-strong); border-radius: var(--r-lg);
  }
  /* A chosen banner, or the person's own colour as a soft wash in the theme's light. */
  .banner {
    height: 92px; position: relative;
    background:
      radial-gradient(120% 140% at 20% 0%, color-mix(in oklch, oklch(.7 .15 var(--hue)) 70%, transparent), transparent 70%),
      linear-gradient(135deg, oklch(.5 .12 var(--hue)), oklch(.32 .09 calc(var(--hue) + 40)));
  }
  .banner.image { background: var(--bg-3); }
  .banner img { width: 100%; height: 100%; object-fit: cover; display: block; }
  /* The picture sits on the banner's edge, cut out of it by a ring of the card. */
  .face { margin: -40px 0 0 16px; width: max-content; position: relative; --presence-cut: var(--bg-2); }
  .face :global(.avatar) { box-shadow: 0 0 0 5px var(--bg-2); }
  :global(:root[data-presence='ring']) .face :global(.avatar.online) { box-shadow: 0 0 0 5px var(--bg-2), 0 0 0 6.5px var(--success); }
  .body { padding: 8px 16px 16px; display: grid; gap: 8px; }
  h2 { margin: 0; font-size: 21px; line-height: 1.15; overflow-wrap: anywhere; }
  p { margin: 0; }
  .handle { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; margin-top: -4px; font-size: var(--fs-sm); color: var(--ink-2); }
  .tag {
    display: inline-flex; align-items: center; gap: 3px; padding: 1px 6px; border-radius: var(--r-pill, 999px);
    font: 600 var(--fs-xs)/1.45 var(--body); color: var(--ink-2); background: var(--bg-3);
  }
  .tag.lit { color: var(--lamp); background: var(--lamp-glow); }
  .presence { display: flex; align-items: center; gap: 6px; font-size: var(--fs-xs); color: var(--ink-2); }
  .pip { width: 7px; height: 7px; border-radius: 50%; box-shadow: inset 0 0 0 1.5px var(--ink-3); }
  .presence.here .pip { background: var(--success); box-shadow: none; }
  .doing { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
  .doing li { display: flex; align-items: center; gap: 10px; padding: 8px; border-radius: var(--r); background: var(--bg-3); }
  /* A game in progress is the live thing on the card. */
  .doing li.live { background: color-mix(in srgb, var(--lamp) 12%, var(--bg-3)); box-shadow: inset 0 0 0 1px var(--lamp-dim); }
  .art {
    flex: none; width: 40px; height: 40px; border-radius: calc(var(--r) - 1px); overflow: hidden;
    display: grid; place-items: center; background: var(--bg-2); color: var(--ink-2);
  }
  .live .art { color: var(--lamp); }
  .art img { width: 100%; height: 100%; object-fit: cover; }
  .what { min-width: 0; display: grid; }
  .verb { font: 600 var(--fs-xs)/1.4 var(--body); color: var(--ink-2); }
  .live .verb { color: var(--lamp); }
  .thing { font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .more { font-size: var(--fs-xs); color: var(--ink-2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .status {
    display: flex; align-items: center; gap: 8px; padding: 8px 10px; border-radius: var(--r);
    background: var(--bg-3); overflow-wrap: anywhere;
  }
  .emoji { font-size: 18px; line-height: 1; }
  .bio { line-height: 1.45; white-space: pre-line; overflow-wrap: anywhere; color: var(--ink); }
  .btn { justify-content: center; margin-top: 4px; }

  /* A phone's sheet: edge to edge, a taller banner, and round actions like an iPhone contact card. */
  .card.sheet { width: 100%; border: 0; border-radius: 0; background: none; }
  .sheet .banner { height: 120px; }
  .sheet .face { margin: -46px 0 0 16px; }
  .sheet .body { padding: 10px 16px 20px; gap: 10px; }
  .sheet h2 { font-size: 26px; }
  .actions { display: flex; justify-content: center; gap: 28px; margin: 6px 0 4px; }
  .act { display: flex; flex-direction: column; align-items: center; gap: 6px; min-width: 56px; font-size: 12px; font-weight: 500; color: var(--ink-2); cursor: pointer; list-style: none; }
  .act::-webkit-details-marker { display: none; }
  .ic { display: grid; place-items: center; width: 52px; height: 52px; border-radius: 50%; background: var(--bg-3); color: var(--lamp); transition: opacity var(--t-fast); }
  .act:active .ic { opacity: .6; }
  .menu { position: fixed; inset: auto; margin: 0; padding: 4px; min-width: 200px; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-2); color: var(--ink); box-shadow: var(--lift); }
  .menu button { display: flex; align-items: center; gap: 10px; width: 100%; min-height: 44px; padding: 0 12px; border-radius: var(--r); font-size: var(--fs-body); text-align: start; }
  .menu button:active { background: var(--press); }
</style>
