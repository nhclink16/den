// Which conversation a composer, draft or upload queue belongs to.
//
// The identity is the channel plus the ROOT MESSAGE, not the server's thread ID.
// A thread only exists once its first reply has been saved, so before that the
// root is the only stable handle a composer has; adopting the thread ID later
// would change identity mid-send and throw away the draft that caused it.
//
// A null root means the room itself. Reply target is deliberately NOT part of
// this: you can change who you are quoting inside one conversation without
// moving to a different one.
export type Conversation = { channelId: string; rootId: string | null }

export const room = (channelId: string): Conversation => ({ channelId, rootId: null })

export function sameConversation(a: Conversation, b: Conversation): boolean {
  return a.channelId === b.channelId && a.rootId === b.rootId
}

// A string form for keying maps. The separator cannot appear in a ULID, so a
// room key can never collide with a conversation key in the same channel.
export function conversationKey(c: Conversation): string {
  return `${c.channelId}:${c.rootId ?? ''}`
}
