// Which completed upload IDs a composer submission actually consumed.
//
// Slash commands (for example /canvas) create objects, not messages, so they
// never transmit pending attachments. Only an ordinary message send consumes
// them: treating a successful command like a send silently drops the user's
// completed uploads without delivering them anywhere.
export function consumedUploadIds(isCommand: boolean, ready: string[]): string[] {
  return isCommand ? [] : ready
}
