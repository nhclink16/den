export const meta = {
  name: 'den_ui_visual_audit',
  description: 'Screenshot Den across surfaces and viewports, critique each, verify findings',
  phases: [
    { title: 'Capture', detail: 'one agent per surface/viewport, screenshot and critique' },
    { title: 'Verify', detail: 'skeptical second look at each candidate defect' },
    { title: 'Synthesize', detail: 'dedupe, rank, report coverage' },
  ],
}

// args: { baseUrl, user, password, outDir }
const cfg = {
  baseUrl: args?.baseUrl || 'http://localhost:5173',
  user: args?.user || 'nicholas',
  password: args?.password || '',
  outDir: args?.outDir || '/tmp/den-ui-audit',
}

// Deliberately not a cross product. Each surface is paired with the viewport that
// actually stresses it: settings has a 650px breakpoint, the call grid broke on a
// portrait monitor last night, phones are where tap targets and overflow show up.
const SURFACES = [
  { path: '/login', w: 390, h: 844, scheme: 'dark', note: 'Signup/login on a phone. Field order, hint legibility, keyboard obstruction.' },
  { path: '/login', w: 1280, h: 800, scheme: 'light', note: 'Login on a laptop in light mode. Card proportions, contrast.' },
  { path: '/', w: 390, h: 844, scheme: 'dark', note: 'Channel view on a phone. Message density, composer, safe areas.' },
  { path: '/', w: 1280, h: 800, scheme: 'dark', note: 'Channel view on a laptop. Reading measure, rhythm, hover affordances.' },
  { path: '/', w: 1100, h: 1900, scheme: 'dark', note: 'Channel view on a PORTRAIT monitor. This orientation shipped broken once.' },
  { path: '/', w: 1920, h: 1080, scheme: 'light', note: 'Channel view on a large light display. Wasted space, max widths.' },
  { path: '/inbox', w: 390, h: 844, scheme: 'dark', note: 'Inbox on a phone. Empty and populated states.' },
  { path: '/inbox', w: 1280, h: 800, scheme: 'light', note: 'Inbox on a laptop, light mode.' },
  { path: '/find', w: 1280, h: 800, scheme: 'dark', note: 'Search. Empty state, result legibility.' },
  { path: '/settings/appearance', w: 390, h: 844, scheme: 'dark', note: 'Settings below the 650px breakpoint: the section strip scrolls itself.' },
  { path: '/settings/appearance', w: 1280, h: 800, scheme: 'light', note: 'Appearance on a laptop. Theme cards, previews, contrast slider.' },
  { path: '/settings/sounds', w: 1280, h: 800, scheme: 'dark', note: 'Sounds: shipped hours ago, least looked at. Per-event rows.' },
  { path: '/settings/voice', w: 390, h: 844, scheme: 'dark', note: 'Voice on a phone. Sliders and device pickers are tap-target risks.' },
  { path: '/settings/account', w: 1280, h: 800, scheme: 'dark', note: 'Account. Profile fields, avatar/banner, destructive actions.' },
  { path: '/settings/layout', w: 768, h: 1024, scheme: 'dark', note: 'Layout on a tablet, the least-tested width.' },
  { path: '/settings/agents', w: 1280, h: 800, scheme: 'light', note: 'Agents. This is the surface an outside contributor judges the project by.' },
]

const RUBRIC = `
Judge only what the screenshot and the collected facts support. You are looking for defects a
careful designer would flag, not for things to say.

Defect classes that count:
- Text clipped, truncated, or overlapping.
- Horizontal scrolling of the page body, or an element overflowing its container.
- Tap targets under about 32px on a touch viewport.
- Contrast too low to read comfortably, especially secondary text and placeholders.
- Focus states missing or invisible.
- Misalignment or inconsistent spacing that reads as unfinished.
- Empty states that look broken rather than intentionally empty.
- Controls whose purpose is not guessable from the screen.

Explicitly NOT defects:
- Personal taste in colour or font.
- Density choices that are consistent across the app.
- Anything you cannot see evidence for in the screenshot or facts.

For every defect: name the element, say what is wrong, say what it should be instead, and rate
severity high/medium/low. If the surface is fine, return an empty list and say so. There is no
quota; zero findings on a clean screen is the correct answer.`

const FINDING_SCHEMA = {
  type: 'object',
  properties: {
    surface: { type: 'string' },
    viewport: { type: 'string' },
    captureOk: { type: 'boolean' },
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          element: { type: 'string' },
          problem: { type: 'string' },
          expected: { type: 'string' },
          severity: { type: 'string', enum: ['high', 'medium', 'low'] },
          evidence: { type: 'string' },
        },
        required: ['element', 'problem', 'expected', 'severity'],
      },
    },
  },
  required: ['surface', 'viewport', 'findings'],
}

phase('Capture')
const captured = await parallel(
  SURFACES.map((s, i) => () => {
    const out = `${cfg.outDir}/${String(i).padStart(2, '0')}-${s.path.replace(/\//g, '_')}-${s.w}x${s.h}-${s.scheme}.png`
    return agent(
      `Audit one Den surface visually.

Run exactly this, from the repository root, then read the PNG it writes:

  mkdir -p ${cfg.outDir}
  node scripts/workflows/shot.mjs --url ${cfg.baseUrl} --path "${s.path}" --out "${out}" --w ${s.w} --h ${s.h} --scheme ${s.scheme} --user ${cfg.user} --password '${cfg.password}'

It prints one JSON line with objective facts: whether the page scrolls horizontally, which
elements overflow, which tap targets are under 32px, and any console errors. Treat those as
evidence, and look at the screenshot yourself for everything else.

Surface: ${s.path} at ${s.w}x${s.h}, ${s.scheme} mode.
Why this combination: ${s.note}

${RUBRIC}

If the capture fails, set captureOk false and explain rather than inventing findings.`,
      { label: `see:${s.path}@${s.w}`, phase: 'Capture', schema: FINDING_SCHEMA, tier: 'medium' },
    )
  }),
)

const candidates = captured
  .filter(Boolean)
  .flatMap((r) => (r.findings || []).map((f) => ({ ...f, surface: r.surface, viewport: r.viewport })))
log(`${candidates.length} candidate defects across ${SURFACES.length} surfaces`)

phase('Verify')
const judged = await parallel(
  candidates.map((f) => () =>
    verify(f, {
      reviewers: 2,
      lens: 'Is this a real, visible defect that a user would notice, or is it taste, or unsupported by the evidence? Default to rejecting it if you are unsure.',
    }).then((v) => ({ ...f, real: v.real, votes: v.realCount })),
  ),
)
const confirmed = judged.filter(Boolean).filter((f) => f.real)
log(`${confirmed.length} of ${candidates.length} survived verification`)

phase('Synthesize')
return await agent(
  `Turn these confirmed UI defects into one report for the maintainer.

${JSON.stringify(confirmed, null, 2)}

Coverage attempted: ${SURFACES.map((s) => `${s.path}@${s.w}x${s.h}/${s.scheme}`).join(', ')}
Surfaces whose capture failed: ${captured.filter((r) => r && r.captureOk === false).map((r) => r.surface).join(', ') || 'none'}

Group by surface, order by severity, and merge duplicates that are really one defect appearing
on several screens (say which screens). For each, give the element, what is wrong, what it should
be, and severity.

End with a short coverage note saying what was not looked at, including any failed captures and
anything these viewports could not reach, such as an active call, a screen share, or the terminal.
Do not pad the report. If the app is largely fine, say that plainly.`,
  { label: 'report', phase: 'Synthesize', tier: 'big' },
)
