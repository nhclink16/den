import { init, Terminal, FitAddon, type GhosttyCell } from 'ghostty-web'
import { mouseInput } from './mouse'
const ready = init()
export async function mount(host: HTMLElement, cols: number, rows: number, input: (text: string | Uint8Array) => void, canInput: () => boolean) {
  await Promise.all([ready, document.fonts.load('14px "Den Terminal Mono"')])
  const css = getComputedStyle(host)
  const term = new Terminal({ cols, rows, fontFamily: css.getPropertyValue('--mono').trim(), fontSize: 14, scrollback: 5000, cursorBlink: true, theme: { background: css.getPropertyValue('--bg').trim(), foreground: css.getPropertyValue('--ink').trim(), cursor: css.getPropertyValue('--lamp').trim() } })
  const fit = new FitAddon(); term.loadAddon(fit); term.open(host)
  term.onData(input)
  const removeMouse = mouseInput(host, term, input, canInput)
  Object.assign(host, { denTerminal: term })
  // ghostty-web 0.4 resolves default colors into cells at open and has no
  // runtime VT palette setter. Recolor those defaults only at the renderer
  // boundary, leaving the parser, cursor, scrollback and other ANSI colors intact.
  const defaults = term.wasmTerm!.getColors()
  const rgb = (hex: string) => hex.slice(1).match(/../g)!.map(v => parseInt(v, 16))
  let foreground = rgb(css.getPropertyValue('--ink').trim()), background = rgb(css.getPropertyValue('--bg').trim())
  const recolor = (cells: GhosttyCell[] | null) => cells?.map(cell => {
    const c = { ...cell }
    if (c.fg_r === defaults.foreground.r && c.fg_g === defaults.foreground.g && c.fg_b === defaults.foreground.b) [c.fg_r, c.fg_g, c.fg_b] = foreground as [number, number, number]
    if (c.bg_r === defaults.background.r && c.bg_g === defaults.background.g && c.bg_b === defaults.background.b) [c.bg_r, c.bg_g, c.bg_b] = background as [number, number, number]
    return c
  }) ?? null
  const themeBuffer = <T extends object>(source: T) => new Proxy(source, { get(target, key) {
    const value = Reflect.get(target, key)
    if (typeof value === 'function' && (key === 'getLine' || key === 'getScrollbackLine')) return (...args: unknown[]) => recolor(value.apply(target, args))
    return typeof value === 'function' ? value.bind(target) : value
  } })
  const draw = term.renderer!.render.bind(term.renderer!)
  term.renderer!.render = (buffer, force, viewport, scrollback, opacity) => draw(themeBuffer(buffer), force, viewport, scrollback ? themeBuffer(scrollback) : undefined, opacity)
  const redraw = () => { if (term.wasmTerm) term.renderer?.render(term.wasmTerm, true) }
  const themeObserver = new MutationObserver(() => {
    const current = getComputedStyle(host)
    const theme = { background: current.getPropertyValue('--bg').trim(), foreground: current.getPropertyValue('--ink').trim(), cursor: current.getPropertyValue('--accent').trim() }
    Object.assign(term.options.theme, theme)
    term.renderer?.setTheme(theme)
    foreground = rgb(theme.foreground); background = rgb(theme.background)
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
  return { term, fit, redraw, fitScreen, destroy: () => {removeMouse(); themeObserver.disconnect(); term.dispose(); delete (host as HTMLElement & { denTerminal?: Terminal }).denTerminal} }
}
export function screen(term: Terminal) {
  const buffer = term.buffer.active; const lines: string[] = []
  for (let y = 0; y < buffer.length; y++) lines.push(buffer.getLine(y)?.translateToString(true) || '')
  return lines.slice(-term.rows).join('\n')
}
