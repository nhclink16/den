// Which machine `/terminal <name>` means. Pure, so node:test can check every wording.
type Machine = { id: string; name: string; online: boolean }

const SET_UP = 'Machines are set up in Settings › Machines.'

/// The one online machine called `typed` (by name or ID), or a sentence saying why there isn't one
/// and what to type instead.
export function pickMachine(hosts: Machine[], typed: string): { id: string } | { error: string } {
  const online = hosts.filter((h) => h.online)
  const called = (h: Machine) => h.name === typed || h.id === typed
  const matches = online.filter(called)
  if (matches.length === 1) return { id: matches[0]!.id }
  if (matches.length > 1) return { error: `${matches.length === 2 ? 'Two' : matches.length} machines are called ${typed}. Use the machine’s ID.` }
  if (!online.length) return { error: typed ? `No machine called ${typed} is online. ${SET_UP}` : `No machine is online. ${SET_UP}` }
  const names = [...new Set(online.map((h) => `/terminal ${h.name}`))]
  const tryThese = `Try ${new Intl.ListFormat('en', { type: 'disjunction' }).format(names)}.`
  if (!typed) return { error: `Which machine? ${tryThese}` }
  return { error: hosts.some(called) ? `${typed} isn’t online. ${tryThese}` : `No machine called ${typed}. ${tryThese}` }
}
