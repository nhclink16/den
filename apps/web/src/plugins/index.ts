import type { Component, ComponentProps } from 'svelte'
import type Icon from '../ui/Icon.svelte'
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
/// A way in from the composer's +: a round tile on a phone, a row on a computer, shown with the
/// slash command it stands for. `run` gets the same context as that command. Text it returns goes
/// into the message box instead, for when an argument still has to be chosen.
export type ComposerItem = { label: string; icon: ComponentProps<typeof Icon>['name']; command: string; run: (ctx: CommandContext) => void | string | Promise<void | string> }
export type Plugin = {
  objectKinds: Record<string, { card: Component<ObjectProps>; view?: Component<ObjectProps>; tile?: Component<ObjectProps> }>
  /// `arg` names what goes after the command ("<machine>") and the values the list offers for it. With
  /// no values the list leaves the command out, since picking it could only fail; typed, it still runs.
  slashCommands: { name: string; hint: string; arg?: { label: string; values: string[] }; run: (ctx: CommandContext) => void | Promise<void> }[]
  paletteActions: { id: string; label: string; hint: string; disabled?: boolean; run: () => void | Promise<void> }[]
  composerItems?: ComposerItem[]
}
export const plugins: Plugin[] = []
export function registerPlugin(plugin: Plugin) { plugins.push(plugin) }
export function objectKind(kind: string) { return plugins.find((p) => p.objectKinds[kind])?.objectKinds[kind] }
