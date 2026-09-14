import { init, Terminal, FitAddon } from 'ghostty-web'
const ready = init()
export async function mount(host: HTMLElement, cols: number, rows: number, input: (text: string) => void) {
  await Promise.all([ready, document.fonts.load('14px "Den Terminal Mono"')])
  const css = getComputedStyle(host)
  const term = new Terminal({ cols, rows, fontFamily: css.getPropertyValue('--mono').trim(), fontSize: 14, scrollback: 5000, cursorBlink: true, theme: { background: css.getPropertyValue('--bg').trim(), foreground: css.getPropertyValue('--ink').trim(), cursor: css.getPropertyValue('--lamp').trim() } })
  const fit = new FitAddon(); term.loadAddon(fit); term.open(host)
  term.onData(input)
  Object.assign(host, { denTerminal: term })
  const redraw = () => { if (term.wasmTerm) term.renderer?.render(term.wasmTerm, true) }
  const themeObserver = new MutationObserver(() => {
    const current = getComputedStyle(host)
    term.options.theme = { background: current.getPropertyValue('--bg').trim(), foreground: current.getPropertyValue('--ink').trim(), cursor: current.getPropertyValue('--accent').trim() }
    term.options.fontFamily = current.getPropertyValue('--mono').trim()
    redraw()
  })
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['style'] })
  const fitScreen = () => {
    const metrics = term.renderer?.getMetrics(); if (!metrics || !host.clientWidth || !host.clientHeight) return
    const factor = Math.min(14 / term.options.fontSize, host.clientWidth / (term.cols * metrics.width), host.clientHeight / (term.rows * metrics.height))
    const size = Math.max(3, Math.floor(term.options.fontSize * factor * 10) / 10)
    if (Math.abs(term.options.fontSize - size) > .1) term.options.fontSize = size
  }
  return { term, fit, redraw, fitScreen, destroy: () => {themeObserver.disconnect(); term.dispose(); delete (host as HTMLElement & { denTerminal?: Terminal }).denTerminal} }
}
export function screen(term: Terminal) {
  const buffer = term.buffer.active; const lines: string[] = []
  for (let y = 0; y < buffer.length; y++) lines.push(buffer.getLine(y)?.translateToString(true) || '')
  return lines.slice(-term.rows).join('\n')
}
