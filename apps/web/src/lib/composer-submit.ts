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
