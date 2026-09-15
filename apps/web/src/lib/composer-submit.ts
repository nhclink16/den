// Planning for the composer submit path. Pure so node:test can pin the
// attachment contract without a browser.
//
// Slash commands such as /canvas never receive pending upload IDs, so a
// successful command must not consume completed attachments. Ordinary sends,
// including attachment-only sends, consume exactly what they transmit.
export type ComposerPlan =
  | { kind: 'command'; name: string; args: string; uploadIds: string[] }
  | { kind: 'message'; uploadIds: string[] }

export function planComposerSubmit(
  content: string,
  ready: string[],
  commandNames: string[],
): ComposerPlan | null {
  const trimmed = content.trim()
  if (!trimmed && ready.length === 0) return null
  for (const name of commandNames) {
    if (trimmed === `/${name}`) return { kind: 'command', name, args: '', uploadIds: [] }
    if (trimmed.startsWith(`/${name} `))
      return { kind: 'command', name, args: trimmed.slice(name.length + 1).trim(), uploadIds: [] }
  }
  return { kind: 'message', uploadIds: [...ready] }
}

// The composer stays editable while a send is in flight, so clearing the box on
// success is only correct if what is on screen is still what was submitted.
// Revision counts edits rather than comparing text: typing A -> B -> A during a
// pending send is a new draft and must survive. Channel and reply target are part
// of the identity so a completion cannot clear a changed reply context, or a
// different channel's state in a composer that outlived the send.
export type DraftIdentity = {
  channelId: string
  replyToId: string | null
  revision: number
}

export function shouldClearDraft(submitted: DraftIdentity, current: DraftIdentity): boolean {
  return (
    submitted.channelId === current.channelId &&
    submitted.replyToId === current.replyToId &&
    submitted.revision === current.revision
  )
}
