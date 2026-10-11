/// How close to the bottom counts as having seen the newest message.
export const NEAR = 80
/// How much of what was already read stays above the "new" line, for context:
/// about one message.
const CONTEXT = 96

/// Whether the reader is looking at the newest message.
export function atBottom(scrollHeight: number, scrollTop: number, clientHeight: number) {
  return scrollHeight - scrollTop - clientHeight < NEAR
}

/// Where a conversation opens. `line` is where the "new" line sits in the
/// scrolled content, or undefined when nothing is unread. If the line would be
/// on screen from the bottom, the conversation opens at the bottom as before;
/// otherwise it opens with the line near the top, so the reader starts where
/// they left off. `bottom` says whether that counts as having read the tail.
export function landing(line: number | undefined, scrollHeight: number, clientHeight: number) {
  const end = Math.max(0, scrollHeight - clientHeight)
  const top = line === undefined || line >= end ? end : Math.max(0, line - CONTEXT)
  return { top, bottom: atBottom(scrollHeight, top, clientHeight) }
}
