<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { call, type CallParticipant } from '../lib/call.svelte'
  import type { Share } from '../lib/call-shares'
  import type { ObjectSummary } from '../lib/types'
  import { objects } from '../lib/objects.svelte'
  import { objectKind } from '../plugins'
  import { bounds, pack, participantSlots, presetCells, type Preset, type Cell, type TileSpec } from '../lib/call-layout'
  import { callLayouts } from '../lib/call-layout.svelte'
  import { callLine } from '../lib/room-list'
  import { disclosurePopover } from '../lib/disclosure-popover'
  import { shell } from '../lib/shell-call'
  import CallLayoutTile from './CallLayoutTile.svelte'
  import ParticipantVolume from './ParticipantVolume.svelte'
  import NavBar from './NavBar.svelte'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  let { back = '', titled = false }: { back?: string; titled?: boolean } = $props()
  const line = $derived(callLine(call.participants, call.reconnecting))
  type Tile = TileSpec & { participant?: CallParticipant; share?: Share; object?: ObjectSummary }
  let mobile = $state(window.innerWidth < 900), width = $state(0), height = $state(0)
  // Two different questions, and conflating them swapped people's layouts. How to
  // arrange tiles depends on the room the grid actually has, so it follows the
  // container. Which saved layout to load is a property of the window, so it follows
  // the window: hiding the member list makes the grid wider, and that must not count
  // as a rotation and hand you the other orientation's arrangement.
  let windowPortrait = $state(matchMedia('(orientation: portrait)').matches)
  const portrait = $derived(height > width && width > 0)
  const stacked = $derived(mobile || portrait)
  let scroll: HTMLDivElement
  let preview = $state<Record<string, Cell> | null>(null), dragging = $state('')
  let cancelDrag: (() => void) | undefined
  // Orientation is read outside untrack so a real rotation reloads that shape's layout.
  $effect(() => { const id = call.channel?.id, shape = windowPortrait; if (id) untrack(() => callLayouts.open(id, mobile, shape)) })
  onMount(() => {
    const mq = matchMedia('(max-width: 899px)'), update = () => { mobile = mq.matches; cancelDrag?.() }
    const turn = matchMedia('(orientation: portrait)'), rotate = () => { windowPortrait = turn.matches; cancelDrag?.() }
    mq.addEventListener('change', update)
    turn.addEventListener('change', rotate)
    return () => { mq.removeEventListener('change', update); turn.removeEventListener('change', rotate); cancelDrag?.() }
  })
  const tiles = $derived.by<Tile[]>(() => {
    const slots = participantSlots(call.participants.map(p => p.id), callLayouts.value.tiles)
    const out: Tile[] = []
    for (const participant of call.participants) for (const share of participant.screens) out.push({ key: `${slots[participant.id]}:screen:${share.name}`, kind: 'screen', participant, share })
    const object = objects.active
    if (object && objectKind(object.kind)?.tile && (object.kind === 'canvas' || object.kind === 'terminal')) out.push({ key: `${object.created_by}:${object.kind}:${object.id}`, kind: object.kind, object })
    for (const participant of call.participants) out.push({ key: `${slots[participant.id]}:cam`, kind: 'cam', participant })
    return out
  })
  // A phone shows video only where there is some (a camera that's on, a share, a canvas), like
  // FaceTime. Everyone else is a face: big and centred when nobody has video, a row under it otherwise.
  const shown = $derived(mobile ? tiles.filter(t => t.kind !== 'cam' || t.participant?.camera) : tiles)
  const faces = $derived(mobile ? call.participants.filter(p => !p.camera) : [])
  const big = $derived(!shown.length)
  const face = $derived(!big ? 52 : faces.length <= 2 ? 112 : faces.length <= 4 ? 88 : faces.length <= 9 ? 68 : 52)
  let facesHeight = $state(0)
  // The room the tiles have: what the faces under them leave.
  const room = $derived(height - (shown.length && faces.length ? facesHeight + 12 : 0))
  const preset = $derived(mobile && callLayouts.value.preset === 'Custom' ? 'Focus' : callLayouts.value.preset)
  const cells = $derived(preview || (preset === 'Custom' ? pack(shown, callLayouts.value.tiles) : presetCells(shown, preset, callLayouts.value.tiles, stacked)))
  const rows = $derived(Math.max(1, ...Object.values(cells).map(c => c.row + c.h)))
  const stepX = $derived((width + 8) / 12)
  // Keep preset rows within the viewport when practical; custom layouts can scroll.
  // The ceiling is a ratio of the column width so a tile keeps a sane shape. Portrait
  // gets a taller ceiling: bounding rows by width alone left most of a tall screen
  // empty, which is what a vertical monitor looked like before. Anything still left
  // over is split above and below by the canvas margins rather than all falling below.
  // A phone starts from square cells and stretches the rows to fill the screen, like
  // FaceTime, up to twice as tall as wide so one or two people don't become towers.
  // More rows than fit keep square cells and scroll.
  const stepY = $derived(mobile ? Math.max(24, stepX, Math.min(stepX * 2, (room - 8) / rows)) : Math.max(24, Math.min(stepX * (portrait ? 1.3 : .8), (height - 8) / rows)))
  function choose(value: Preset) { cancelDrag?.(); callLayouts.save(value, presetCells(shown, value, callLayouts.value.tiles, mobile)) }
  function reset() { cancelDrag?.(); callLayouts.reset() }
  function pin(key: string) {
    // Mobile presents Custom as Focus; pinning must not save its display geometry
    // over the desktop arrangement. Only the pin changes in that case.
    const saved = callLayouts.value
    const original = mobile && saved.preset === 'Custom' ? saved.tiles[key] || cells[key]! : cells[key]!
    callLayouts.save(saved.preset, { ...(mobile && saved.preset === 'Custom' ? {} : cells), [key]: { ...original, pinned: !original.pinned } })
  }
  function move(key: string, dx: number, dy: number, resize: boolean) {
    const old = cells[key]!
    const next = bounds({ ...old, ...(resize ? { w: old.w + dx, h: old.h + dy } : { col: old.col + dx, row: old.row + dy }) })
    callLayouts.save('Custom', pack(shown, { ...cells, [key]: next }, key))
  }
  function drag(key: string, e: PointerEvent, resize: boolean) {
    if (mobile || e.button !== 0) return
    e.preventDefault(); cancelDrag?.()
    const target = e.currentTarget as HTMLElement
    const root = target.closest<HTMLElement>('.layout-tile')!; root.focus()
    const start = { x: e.clientX, y: e.clientY, scroll: scroll.scrollTop, stepY, cells: { ...cells }, tile: { ...cells[key]! } }
    target.setPointerCapture(e.pointerId)
    let moved = false
    function update(event: PointerEvent) {
      if (event.pointerId !== e.pointerId) return
      if (!moved && Math.hypot(event.clientX - start.x, event.clientY - start.y) < 4) return
      moved = true; dragging = key
      const box = scroll.getBoundingClientRect()
      if (event.clientY > box.bottom - 30) scroll.scrollTop += 14
      if (event.clientY < box.top + 30) scroll.scrollTop -= 14
      const dx = Math.round((event.clientX - start.x) / stepX), dy = Math.round((event.clientY - start.y + scroll.scrollTop - start.scroll) / start.stepY)
      const next = bounds({ ...start.tile, ...(resize ? { w: start.tile.w + dx, h: start.tile.h + dy } : { col: start.tile.col + dx, row: start.tile.row + dy }) })
      preview = pack(shown, { ...start.cells, [key]: next }, key)
    }
    function finish(event?: PointerEvent) {
      if (event && event.pointerId !== e.pointerId) return
      if (event?.type === 'pointerup' && moved && preview) callLayouts.save('Custom', preview)
      target.removeEventListener('pointermove', update); target.removeEventListener('pointerup', finish); target.removeEventListener('pointercancel', finish); target.removeEventListener('lostpointercapture', finish)
      if (target.hasPointerCapture(e.pointerId)) target.releasePointerCapture(e.pointerId)
      preview = null; dragging = ''; cancelDrag = undefined
    }
    cancelDrag = () => finish()
    target.addEventListener('pointermove', update); target.addEventListener('pointerup', finish); target.addEventListener('pointercancel', finish); target.addEventListener('lostpointercapture', finish)
  }
</script>

{#snippet talkLine(cls: string)}
  <span class="line {cls}" class:talking={!!line.talking && !call.reconnecting}>
    {#if line.talking && !call.reconnecting}<span class="talk-bars" aria-hidden="true"><i></i><i></i><i></i></span>{/if}
    <span class="say">{[line.talking, line.people].filter(Boolean).join(' · ')}</span>
  </span>
{/snippet}
{#snippet person(p: CallParticipant)}
  <span class="pic" class:speaking={p.speaking}>
    {#if p.music}<span class="music" style:--s="{face}px"><Icon name="music" size={Math.round(face * .4)} /></span>{:else}<Avatar userId={p.userId} size={face} instance={call.owner ?? undefined} presence={false} />{/if}
    {#if p.muted && !p.music}<span class="muted" data-testid="muted-mic" role="img" aria-label="Microphone muted"><Icon name="mic-off" size={big ? 16 : 12} /></span>{/if}
  </span>
  <span class="nm">{p.name}{#if p.device}<small> · {p.device}</small>{/if}</span>
  {#if p.unseen && big}<span class="note">{p.unseen === 'screen' ? 'Sharing their screen' : 'Camera on'}</span>{/if}
{/snippet}

{#if mobile}
  <!-- The phone's bar: the chevron puts the call away, like FaceTime's. -->
  <NavBar title={call.title} subline={sub}>
    {#snippet leading()}<button class="icon-btn collapse" aria-label="Collapse call" title="Back to the chat" onclick={() => (call.expanded = false)}><Icon name="chevron" size={28} /></button>{/snippet}
  </NavBar>
  {#snippet sub()}{@render talkLine('sub')}{/snippet}
{:else}
<div class="call-head">
  <div class="what">
    {#if titled}<h2>{call.title}</h2>{/if}
    {@render talkLine('')}
  </div>
  <div class="presets segments small" role="group" aria-label="Call layout">
    {#each ['Auto', 'Even', 'Focus'] as p}<button aria-pressed={preset === p} onclick={() => choose(p as Preset)}>{p}</button>{/each}
    {#if preset === 'Custom'}<span>Custom</span>{/if}
  </div>
  <button class="btn small collapse" title="Collapse call" onclick={() => (call.expanded = false)}><Icon name="collapse" /><span class="lbl">{back ? `Back to ${back}` : 'Back'}</span></button>
</div>
{/if}
<div class="grid-scroll" class:faces-only={mobile && big} bind:this={scroll} bind:clientHeight={height}>
  {#if shown.length}
  <div class="grid-canvas" class:dragging={!!dragging} bind:clientWidth={width}
    style={`--step-x:${stepX}px;--step-y:${stepY}px;height:${rows * stepY}px`}>
    {#each shown as tile (tile.key)}
      <CallLayoutTile tileKey={tile.key} cell={cells[tile.key]!} participant={tile.participant} share={tile.share} object={tile.object} {mobile}
        onpin={() => pin(tile.key)} onmove={(dx, dy, resize) => move(tile.key, dx, dy, resize)} onreset={reset} ondrag={(e, resize) => drag(tile.key, e, resize)} />
    {/each}
    {#if dragging && cells[dragging]}
      {@const c = cells[dragging]!}
      <div class="drop-target" style={`left:${c.col * stepX}px;top:${c.row * stepY}px;width:${c.w * stepX - 8}px;height:${c.h * stepY - 8}px`}></div>
    {/if}
  </div>
  {/if}
  {#if faces.length}
    <ul class="people" class:big style:--face="{face}px" style:--cols={faces.length <= 1 ? 1 : faces.length <= 4 ? 2 : 3} bind:clientHeight={facesHeight} aria-label="In the call">
      {#each faces as p (p.id)}
        <li>
          {#if !p.local && !p.music && !shell}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions (Escape closes the disclosure) -->
            <details class="who" onkeydown={(e) => { if (e.key === 'Escape') { e.preventDefault(); e.currentTarget.open = false; e.currentTarget.querySelector('summary')?.focus() } }}>
              <summary aria-label={`${p.name}${p.muted ? ', muted' : ''}, audio options`}>{@render person(p)}</summary>
              <div class="sheet who-sheet" popover="auto" use:disclosurePopover><p class="who-name">{p.name}</p><ParticipantVolume userId={p.userId} name={p.name} /></div>
            </details>
          {:else}
            <div class="who">{@render person(p)}</div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .call-head { container-type: inline-size; flex: none; display: flex; align-items: center; gap: 12px; min-height: 56px; padding: 10px 20px; }
  .what { flex: 1; min-width: 0; display: flex; flex-direction: column; line-height: 1.25; }
  h2 { margin: 0; font-family: var(--display); font-size: var(--fs-title); font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .line { display: flex; align-items: center; gap: 6px; min-width: 0; font-size: var(--fs-sm); color: var(--ink-2); }
  .say { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .presets { flex: none; }
  /* Custom is not a choice to click: it names a layout you dragged into. */
  .presets span { align-self: center; padding: 0 10px; font-size: var(--fs-xs); font-weight: 600; color: var(--ink-3); }
  /* A long room name gives way before the layout switch does. */
  .btn.small { flex: 0 1 auto; min-width: 0; max-width: 40%; min-height: 32px; padding: 4px 14px; font-size: var(--fs-sm); }
  .lbl { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  /* A narrow room (both side panels open): Back is just its icon, and still says where it goes to a screen reader. */
  @container (max-width: 560px) { .btn.small { padding: 4px 9px; } .lbl { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; } }
  /* Clear of the floating bar of buttons below. */
  .grid-scroll { flex: 1; min-height: 0; margin: 0 20px 96px; overflow: auto; scrollbar-width: thin; display: flex; }
  /* Auto margins centre the rows when they are shorter than the viewport and
     collapse to nothing when they overflow, which keeps the top row reachable. */
  .grid-canvas { position: relative; width: 100%; flex: none; margin-block: auto; }
  .dragging { background-image: linear-gradient(to right, var(--grid-guide) 1px, transparent 1px), linear-gradient(to bottom, var(--grid-guide) 1px, transparent 1px); background-size: var(--step-x) var(--step-y); }
  .dragging :global(.layout-tile) { opacity: .8; }
  .drop-target { position: absolute; pointer-events: none; border: 2px solid var(--lamp); border-radius: var(--r-tile); background: var(--grid-guide); }
  @media (max-width: 899px) {
    .grid-scroll { margin: 0 12px calc(104px + env(safe-area-inset-bottom, 0px)); flex-direction: column; }
  }
  /* The bar's own line: grey, and the talker in ink with the accent bars. */
  .line.sub { justify-content: center; font-size: 12px; color: var(--ink-3); }
  .line.sub.talking { color: var(--ink); }
  .call-head .line.talking, .line.sub.talking { color: var(--ink); }
  .collapse :global(svg) { stroke-width: 2.2; }
  /* Faces: centred in the call when nobody has video, a row under the tiles otherwise. */
  .people { list-style: none; margin: 12px 0 0; padding: 0; display: flex; flex-wrap: wrap; justify-content: center; gap: 14px 10px; flex: none; }
  .people.big { margin: auto 0; display: grid; grid-template-columns: repeat(var(--cols), minmax(0, 1fr)); gap: 28px 12px; padding-block: 12px; }
  .faces-only { justify-content: center; }
  .people li { min-width: 0; display: flex; justify-content: center; }
  .who { display: flex; flex-direction: column; align-items: center; gap: 8px; min-width: 0; max-width: 100%; }
  .people:not(.big) .who { width: calc(var(--face) + 20px); gap: 4px; }
  .who > summary { list-style: none; display: flex; flex-direction: column; align-items: center; gap: inherit; min-width: 0; max-width: 100%; cursor: pointer; border-radius: var(--r); }
  .who > summary::-webkit-details-marker { display: none; }
  .pic { position: relative; display: flex; border-radius: var(--avatar-r, 50%); transition: box-shadow var(--t); }
  /* Talking: a ring in the accent with a soft glow, as Discord's voice screen does. */
  .pic.speaking { box-shadow: 0 0 0 3px var(--bg), 0 0 0 6px var(--lamp), 0 0 28px 6px color-mix(in srgb, var(--lamp) 45%, transparent); }
  .music { width: var(--s); height: var(--s); display: grid; place-items: center; border-radius: 50%; background: var(--bg-3); color: var(--ink-2); }
  .muted { position: absolute; right: -2px; bottom: -2px; display: grid; place-items: center; padding: 4px; border-radius: 50%; background: var(--bg-3); color: var(--danger); box-shadow: 0 0 0 2px var(--bg); }
  .nm { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--fs-body); font-weight: 600; color: var(--ink); }
  .people:not(.big) .nm { font-size: var(--fs-xs); font-weight: 500; color: var(--ink-2); }
  .nm small { font-weight: 400; color: var(--ink-2); }
  .note { margin-top: -4px; font-size: var(--fs-xs); color: var(--ink-3); }
  .who-sheet { position: fixed; inset: auto; margin: 0; padding: 8px 8px 12px; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-2); color: var(--ink); box-shadow: var(--lift); }
  .who-name { margin: 6px 8px 0; font-weight: 700; }
  @media (max-width: 600px) { .who-sheet { padding-inline: 16px; } .who-sheet :global(.volume-control) { width: auto; } }
</style>
