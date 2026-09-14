// The current window replaces its partial text; overlap is committed only once.
export function joinTranscript(previous: string, next: string) {
  const left = previous.trim().split(/\s+/).filter(Boolean), right = next.trim().split(/\s+/).filter(Boolean)
  const word = (s: string) => s.toLocaleLowerCase().replace(/[^\p{L}\p{N}']/gu, '')
  let overlap = Math.min(5, left.length, right.length)
  while (overlap && !left.slice(-overlap).every((s, i) => word(s) === word(right[i]!))) overlap--
  return [...left, ...right.slice(overlap)].join(' ')
}
export class DictationDraft {
  private prefix: string
  private suffix: string
  private committed = ''
  private punctuation: boolean
  constructor(text: string, caret: number, punctuation: boolean) {
    this.prefix = text.slice(0, caret); this.suffix = text.slice(caret); this.punctuation = punctuation
  }
  accept(partial: string, final: boolean) {
    const next = this.punctuation ? partial : partial.replace(/[.,!?;:]/g, '')
    const transcript = joinTranscript(this.committed, next)
    if (final) this.committed = transcript
    const before = this.prefix + (this.prefix && !/\s$/.test(this.prefix) && transcript ? ' ' : '') + transcript
    return { text: before + (transcript && this.suffix && !/^\s|^[.,!?;:]/.test(this.suffix) ? ' ' : '') + this.suffix, caret: before.length }
  }
}
