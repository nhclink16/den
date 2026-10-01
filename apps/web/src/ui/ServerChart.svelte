<script lang="ts">
  // One number over the last 24 hours. One series per chart, so the title names
  // it and there is no legend; one y-axis, starting at zero.
  import type { ServerSeries } from '../lib/types'
  import { formatValue } from '../lib/servers'

  let { series, ceiling, step = false }: { series: ServerSeries; ceiling?: number; step?: boolean } = $props()

  const W = 480, H = 132, L = 30, R = 8, T = 10, B = 20
  let now = $state(Date.now() / 1000)
  $effect(() => { const t = setInterval(() => (now = Date.now() / 1000), 60_000); return () => clearInterval(t) })
  const from = $derived(now - 86_400)
  const points = $derived(series.points.filter((p) => p.at >= from))
  const top = $derived.by(() => {
    const peak = Math.max(ceiling ?? 0, ...points.map((p) => p.value), 1)
    if (series.unit === 'tps') return Math.max(20, Math.ceil(peak))
    return peak <= 5 ? Math.ceil(peak) : Math.ceil(peak / 5) * 5
  })
  const x = (at: number) => L + ((at - from) / 86_400) * (W - L - R)
  const y = (v: number) => T + (1 - v / top) * (H - T - B)
  // A gap longer than five minutes is the relay being away, not a flat line.
  const runs = $derived.by(() => {
    const out: typeof points[] = []
    for (const p of points) {
      const run = out.at(-1), last = run?.at(-1)
      if (run && last && p.at - last.at <= 300) run.push(p); else out.push([p])
    }
    return out
  })
  const path = (run: typeof points) => run.map((p, i) => {
    if (i === 0) return `M${x(p.at).toFixed(1)},${y(p.value).toFixed(1)}`
    return step ? `H${x(p.at).toFixed(1)}V${y(p.value).toFixed(1)}` : `L${x(p.at).toFixed(1)},${y(p.value).toFixed(1)}`
  }).join('')
  const area = (run: typeof points) => `${path(run)}V${y(0)}H${x(run[0]!.at).toFixed(1)}Z`
  const ticks = $derived([0, top / 2, top].map((v) => (series.unit === 'tps' || Number.isInteger(v) ? v : Math.round(v))))
  const hours = $derived.by(() => {
    const out: { at: number; label: string }[] = []
    const d = new Date(from * 1000); d.setMinutes(0, 0, 0)
    for (let at = d.getTime() / 1000 + 3600; at < now; at += 3600) {
      const h = new Date(at * 1000).getHours()
      if (h % 6 === 0) out.push({ at, label: new Date(at * 1000).toLocaleTimeString([], { hour: 'numeric' }) })
    }
    return out
  })

  let hover = $state<(typeof points)[number] | null>(null)
  function move(e: PointerEvent) {
    const box = (e.currentTarget as SVGElement).getBoundingClientRect()
    const at = from + ((e.clientX - box.left) / box.width * W - L) / (W - L - R) * 86_400
    let best: (typeof points)[number] | null = null
    for (const p of points) if (!best || Math.abs(p.at - at) < Math.abs(best.at - at)) best = p
    hover = best && Math.abs(best.at - at) < 1800 ? best : null
  }
  const summary = $derived.by(() => {
    if (!points.length) return `${series.label}: no data in the last 24 hours`
    const vals = points.map((p) => p.value)
    const f = (v: number) => formatValue(v, series.unit)
    return `${series.label} over the last 24 hours: now ${f(vals.at(-1)!)}, low ${f(Math.min(...vals))}, high ${f(Math.max(...vals))}`
  })
  const time = (at: number) => new Date(at * 1000).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
</script>

<figure class="chart">
  <figcaption><span class="eyebrow">{series.label}</span><span class="faint mono">24h</span></figcaption>
  <div class="plot">
    <svg viewBox="0 0 {W} {H}" role="img" aria-label={summary} onpointermove={move} onpointerleave={() => (hover = null)}>
      {#each ticks as t (t)}
        <line class="grid" x1={L} x2={W - R} y1={y(t)} y2={y(t)} />
        <text class="axis" x={L - 6} y={y(t) + 3.5} text-anchor="end">{series.unit === 'tps' ? t : Math.round(t)}</text>
      {/each}
      {#each hours as h (h.at)}<text class="axis" x={x(h.at)} y={H - 5} text-anchor="middle">{h.label}</text>{/each}
      {#each runs as run, i (i)}
        {#if step}<path class="area" d={area(run)} />{/if}
        <path class="line" d={path(run)} />
      {/each}
      {#if hover}
        <line class="cross" x1={x(hover.at)} x2={x(hover.at)} y1={T} y2={H - B} />
        <circle class="dot" cx={x(hover.at)} cy={y(hover.value)} r="4" />
      {/if}
    </svg>
    {#if hover}
      <div class="tip" style:left="{Math.min(88, Math.max(12, (x(hover.at) / W) * 100))}%" role="presentation">
        <b class="mono">{formatValue(hover.value, series.unit)}</b> <span class="faint">{time(hover.at)}</span>
      </div>
    {/if}
    {#if !points.length}<p class="none faint">Nothing recorded yet</p>{/if}
  </div>
</figure>

<style>
  .chart { margin: 0; padding: 12px 14px 8px; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-2); min-width: 0; }
  figcaption { display: flex; justify-content: space-between; align-items: baseline; margin-bottom: 4px; }
  figcaption .mono { font-size: 11px; }
  .plot { position: relative; }
  svg { display: block; width: 100%; height: auto; overflow: visible; touch-action: pan-y; }
  .grid { stroke: var(--line); stroke-width: 1; }
  .axis { fill: var(--ink-3); font: 10px var(--mono); }
  .line { fill: none; stroke: var(--lamp); stroke-width: 2; stroke-linejoin: round; stroke-linecap: round; }
  .area { fill: color-mix(in srgb, var(--lamp) 14%, transparent); stroke: none; }
  .cross { stroke: var(--ink-3); stroke-width: 1; stroke-dasharray: 2 3; }
  .dot { fill: var(--lamp); stroke: var(--bg-2); stroke-width: 2; }
  .tip {
    position: absolute; top: -6px; transform: translate(-50%, -100%); pointer-events: none; white-space: nowrap;
    padding: 4px 8px; border: 1px solid var(--line-strong); border-radius: var(--r); background: var(--bg); font-size: 12px;
    box-shadow: 0 6px 18px -8px var(--shadow-lg);
  }
  .none { position: absolute; inset: 0 0 20px 30px; display: grid; place-items: center; margin: 0; font-size: 13px; }
</style>
