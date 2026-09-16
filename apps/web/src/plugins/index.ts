import type { Component } from 'svelte'
import type { Conversation } from '../lib/conversation'
import type { ObjectSummary } from '../lib/types'

export type ObjectProps = { object: ObjectSummary }
/// `conversation` is the composer the command was invoked from: absent for the
/// room, present for a thread panel. Cards land where the person was typing.
export type CommandContext = {
  channelId: string
  conversation?: Conversation
  /// The message the composer was quoting when the command was invoked. A card
  /// created from a quoted reply keeps that arrow; cancelling the quote is a
  /// separate act and does not change where the card is placed.
  replyToId?: string | null
  args: string
  post: (content: string) => Promise<void>
}
export type Plugin = {
  objectKinds: Record<string, { card: Component<ObjectProps>; view?: Component<ObjectProps>; tile?: Component<ObjectProps> }>
  slashCommands: { name: string; hint: string; run: (ctx: CommandContext) => void | Promise<void> }[]
  paletteActions: { id: string; label: string; hint: string; disabled?: boolean; run: () => void | Promise<void> }[]
}
export const plugins: Plugin[] = []
export function registerPlugin(plugin: Plugin) { plugins.push(plugin) }
export function objectKind(kind: string) { return plugins.find((p) => p.objectKinds[kind])?.objectKinds[kind] }
