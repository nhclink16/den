import type { Box } from './box'
import { conversationKey, type Conversation } from './conversation.ts'

export type Draft = { text: string; replyToId: string | null; revision: number }

// Draft ownership, outside the composer that shows it.
//
// A composer is mounted and unmounted by navigation, by a call expanding, by a
// thread panel opening beside it. The half-written message has to outlive that,
// so it lives here, keyed by conversation. Two conversations in one room each
// keep their own text and quote target.
//
// Memory only, for the life of one account on one instance: a draft is not worth
// writing to disk, and logout clears every one of them rather than leaving them
// for whoever signs in next.
//
// `revision` counts EDITS, not text states. Typing A, then B, then A again is two
// edits, so a send captured at the first A does not clear what is in the box now.
// It only ever increases while the draft exists, and entries are kept rather than
// deleted when emptied, so a number a send is still holding cannot be reused.
//
// Revisions do restart when the owner is emptied, which is why `token` exists: a
// send captures it and refuses to clear anything if the owner has been cleared
// and refilled since. Comparing revisions alone would let a completion from one
// account clear an identical-looking draft belonging to the next.
export class Drafts {
  private static readonly blank: Draft = { text: '', replyToId: null, revision: 0 }
  private entries: Box<Record<string, Draft>>
  private lifetime = 0

  constructor(entries: Box<Record<string, Draft>>) { this.entries = entries }

  /// Changes whenever this owner is emptied. Everything a send compares must be
  /// read from the same owner at the same token.
  get token() { return this.lifetime }
  holds(token: number) { return token === this.lifetime }

  for(c: Conversation): Draft { return this.entries.get()[conversationKey(c)] ?? Drafts.blank }

  private write(c: Conversation, part: Partial<Draft>) {
    this.entries.set({ ...this.entries.get(), [conversationKey(c)]: { ...this.for(c), ...part } })
  }

  /// Record an edit. Setting the same text is not an edit, so a re-render cannot
  /// inflate the count and strand a send that is still in flight.
  setText(c: Conversation, text: string) {
    if (this.for(c).text === text) return
    this.write(c, { text, revision: this.for(c).revision + 1 })
  }

  /// The quote target is part of the draft and outlives the message list: a reply
  /// parent can scroll out of the loaded page while the draft answering it is
  /// still open. Only an explicit cancel, or the send that used it, clears it.
  setReplyTo(c: Conversation, replyToId: string | null) {
    if (this.for(c).replyToId === replyToId) return
    this.write(c, { replyToId })
  }

  clear() {
    this.lifetime++
    this.entries.set({})
  }
}
