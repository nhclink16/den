import type { ObjectSummary } from './types'
// Core owns the currently docked object. Renderers belong to plugins.
export const objects = $state({ active: null as ObjectSummary | null, expanded: false, presence: {} as Record<string, string[]> })
export function openObject(object: ObjectSummary) { objects.active = object; objects.expanded = false }
