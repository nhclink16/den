// Submit orchestration for the chat composer, kept free of Svelte runes so
// node tests can drive the exact branching without mounting a component.
//
// A slash command (e.g. /canvas) creates an object; it never transmits the
// composer's pending uploads. Only an ordinary message send consumes the
// completed attachment IDs. Consuming them on the command path silently drops
// the user's unsent selection while the server-side bytes sit unattached.

export type ComposerCommand = {
  name: string
  run: (ctx: { channelId: string; args: string; post: (content: string) => Promise<void> }) => void | Promise<void>
}

export type ComposerSubmit = {
  channelId: string
  content: string
  ready: string[]
  replyTo?: string
  commands: ComposerCommand[]
  send: (channelId: string, content: string, opts: { reply_to?: string; upload_ids: string[] }) => Promise<void>
  post: (channelId: string, content: string) => Promise<void>
  sent: (channelId: string, ids: string[]) => void
}

export function matchCommand(content: string, commands: { name: string }[]): { name: string; args: string } | null {
  const found = commands.find((c) => content === `/${c.name}` || content.startsWith(`/${c.name} `))
  return found ? { name: found.name, args: content.slice(found.name.length + 1).trim() } : null
}

export async function runComposerSubmit(deps: ComposerSubmit): Promise<{ handled: 'command' | 'message' }> {
  const match = matchCommand(deps.content, deps.commands)
  if (match) {
    const command = deps.commands.find((c) => c.name === match.name)!
    await command.run({ channelId: deps.channelId, args: match.args, post: (content) => deps.post(deps.channelId, content) })
    // The command succeeded without sending pending uploads, so the completed
    // attachments stay queued for the user to send next.
    return { handled: 'command' }
  }
  await deps.send(deps.channelId, deps.content, { reply_to: deps.replyTo, upload_ids: deps.ready })
  deps.sent(deps.channelId, deps.ready)
  return { handled: 'message' }
}
