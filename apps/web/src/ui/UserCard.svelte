<script lang="ts">
  // A person at a glance: picture, name, what they are up to, and a way to talk.
  // The same card is the popover in chat and the live preview in Profile settings,
  // where `draft` overlays unsaved edits on the saved profile.
  import { store, type Store } from '../lib/store.svelte'
  import { mediaUrl } from '../lib/native'
  import { personHue } from '../lib/people.svelte'
  import type { User } from '../lib/types'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'

  let { userId, instance = store, draft, onmessage, onedit }: {
    userId: string; instance?: Store; draft?: Partial<User>
    onmessage?: () => void; onedit?: () => void
  } = $props()

  const saved = $derived(instance.user(userId))
  const user = $derived(saved ? { ...saved, ...draft } as User : undefined)
  const name = $derived(user ? user.display_name || user.username : 'Someone')
  const online = $derived(instance.online.has(userId))
  const hue = $derived(personHue(userId, user))
  const status = $derived(user?.status && (user.status.emoji || user.status.text) ? user.status : null)
  const me = $derived(userId === instance.me?.id)
</script>

{#if user}
  <article class="card" style="--hue:{hue}" aria-label={`${name}'s profile`}>
    <div class="banner" class:image={!!user.banner_url}>
      {#if user.banner_url}<img src={mediaUrl(user.banner_url, instance.origin)} alt="" />{/if}
    </div>
    <div class="face"><Avatar {userId} {instance} size={76} /></div>

    <div class="body">
      <h2 class="display">{name}</h2>
      <p class="handle">
        <span class="mono">@{user.username}</span>
        {#if user.bot}<span class="tag lit"><Icon name="bot" size={11} />agent</span>{/if}
        {#if user.role === 'admin'}<span class="tag">admin</span>{/if}
      </p>
      <p class="presence" class:here={online}><span class="pip" aria-hidden="true"></span>{online ? 'Here now' : 'Away'}</p>

      {#if status}
        <p class="status">{#if status.emoji}<span class="emoji">{status.emoji}</span>{/if}<span>{status.text}</span></p>
      {/if}
      {#if user.bio}
        <!-- Plain text by contract: never Markdown or HTML. -->
        <p class="bio">{user.bio}</p>
      {/if}

      {#if me && onedit}
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
  .handle { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; margin-top: -4px; font-size: 13px; color: var(--ink-2); }
  .tag {
    display: inline-flex; align-items: center; gap: 3px; padding: 1px 6px; border-radius: var(--r-pill, 999px);
    font: 600 11px/1.45 var(--mono); letter-spacing: .04em; text-transform: uppercase; color: var(--ink-2); background: var(--bg-3);
  }
  .tag.lit { color: var(--lamp); background: var(--lamp-glow); }
  .presence { display: flex; align-items: center; gap: 6px; font-size: 12px; color: var(--ink-2); }
  .pip { width: 7px; height: 7px; border-radius: 50%; box-shadow: inset 0 0 0 1.5px var(--ink-3); }
  .presence.here .pip { background: var(--success); box-shadow: none; }
  .status {
    display: flex; align-items: center; gap: 8px; padding: 8px 10px; border-radius: var(--r);
    background: var(--bg-3); font-size: 14px; overflow-wrap: anywhere;
  }
  .emoji { font-size: 18px; line-height: 1; }
  .bio { font-size: 14px; line-height: 1.45; white-space: pre-line; overflow-wrap: anywhere; color: var(--ink); }
  .btn { justify-content: center; margin-top: 4px; }
</style>
