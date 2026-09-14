import type { Component } from 'svelte'
import type { ObjectSummary } from '../lib/types'

export type ObjectProps = { object: ObjectSummary }
export type CommandContext = { channelId: string; args: string; post: (content: string) => Promise<void> }
export type Plugin = {
  objectKinds: Record<string, { card: Component<ObjectProps>; view?: Component<ObjectProps>; tile?: Component<ObjectProps> }>
  slashCommands: { name: string; hint: string; run: (ctx: CommandContext) => void | Promise<void> }[]
  paletteActions: { id: string; label: string; hint: string; disabled?: boolean; run: () => void | Promise<void> }[]
}
export const plugins: Plugin[] = []
export function registerPlugin(plugin: Plugin) { plugins.push(plugin) }
export function objectKind(kind: string) { return plugins.find((p) => p.objectKinds[kind])?.objectKinds[kind] }
