# Threads night results — web/Electron activation lane

Written by the Opus implementation lane (w11:pY) overnight, 2026-09-16, against
the ownership transfer in `THREADS-NIGHT-HANDOFF.md`.

## What was published

| PR | Branch | Base | Size |
|---|---|---|---|
| [#43](https://github.com/nhclink16/den/pull/43) manual thread acceptance runners | `feat/thread-acceptance-runners` | `feat/thread-web-state` (#42) | 1189 insertions, 3 files |
| [#44](https://github.com/nhclink16/den/pull/44) web/Electron thread activation | `feat/thread-web-ui` | `feat/thread-acceptance-runners` | 2905 insertions / 186 deletions excluding evidence images |

Heads: #43 `e8c5b1a`, #44 `9e6633c`. Both **unmerged** and cross-linked; #44's
body names the exact runner revision its acceptance ran against. Nothing was
merged, deployed, or pushed to `main`.

#44 breakdown: production 2312/184 over 22 files, focused node tests 592/1 over
5 files, CI one line. That is under the 3000-line stop line in the packaging
brief, and nothing was moved out of the PR to get there.

## Two real bugs were found and fixed overnight

**An orphaned paging flag wedged history, permanently.** Found by the peer lane
reviewing the frozen fetch source. `loadOlder` sets `loadingOlder[key]` and
clears it in a `finally` gated on ownership. When a `latest` load supersedes it —
a reconnect resync, or reopening the room — the superseded request correctly
declines to clear the flag, because by then it may belong to a newer request.
But the replacement did not clear it either, and nothing else would: the only
other clear site is keyed by thread id and never runs for a channel. The result,
on an ordinary trigger, was a permanent spinner and a conversation that could
never load history again for the rest of the session. `logout` had the same
orphan into the next account.

The reason none of the twelve `message-fetch` tests caught it is worth keeping:
every one of them constructs the registry directly. The registry was correct and
the Store's *use* of it was not, so no test at that level could fail.
`scripts/store-paging.test.ts` now runs the actual Store. Its control — an older
page that is never superseded — passes with and without the fix; only the
superseded ordering discriminates.

**A control was unreachable at 200% zoom.** Found by the *genuinely measured*
zoom, on its first real run. At 700x900 with zoom 2 the viewport is 350px and the
conversation header's controls need about 400px, so **Resolve** sat at x 325..402
against a 350px edge: `scrollWidth` 401 vs `clientWidth` 349. Fixed by letting
the header wrap — shrinking the title cannot help, since the buttons exceed the
width with the title fully collapsed, and at every width where they fit nothing
changes.

This is the direct payoff of the correction demanded earlier: the old CDP check
could never have found it, because it never zoomed anything.

## Evidence

Everything is under `/mnt/storage/den-thread-web-state-evidence/`.

- `web-ui-final-acceptance/` — the runs both PRs rest on: `browser/`, `electron/`
- `web-ui-final-acceptance/clip-defect/` — the clipping defect: the run that
  caught it, the measurement probe, the 350px screenshot
- `web-ui-paging-fix/` — the peer's finding, the diff, the red/green table
- `web-ui-source-freeze-final.txt` — core aggregate `d6ec95b6`, tests aggregate
  `c0c9ff3f`. **Reproducible**, unlike earlier freezes — the peer could not
  recompute the previous aggregate, which was a fair complaint. The file states
  the exact command and the committed tree recomputes to both values.

Measured results on the shipped source:

    82 node tests, check 1121 files 0 errors, build clean, desktop bridge 3/3
    browser  PASS  incl. a real PTY in a conversation with observed shell output
    electron PASS  getZoomFactor 1 -> 2, DPR 1 -> 2, CSS width 700 -> 350,
                   native window bounds unchanged, controls inside on both axes

## Things I got wrong, recorded rather than buried

**I overwrote three of Astra's preserved probe results.** My working copies of
Astra's probe scripts were verbatim copies including their hardcoded output
paths, so re-running them rewrote Astra's files in place. `astra-review2` lost
nothing — the overwritten file duplicated a preserved before/after pair that
survives. `astra-review3/original-three-rechecked.json` kept its conclusion but
lost its provenance. `astra-review3/store-probe.json` genuinely lost its RED
record of three reproduced defects, and it cannot be regenerated: the review3
Store bytes are not among any preserved source copy. Those red values survive in
Astra's own prose in `thread-web-ui-review3.md` §2. Full account, including what
I did about it, in `OVERWRITE-INCIDENT.md`. `astra-review4` was never touched.

**I printed a host token** into my session transcript by `cat`-ing a scratch
host config. It belongs to a disposable host pointed at a throwaway localhost
fixture, so the exposure is limited to that scratch server, but it should not
have happened and the file was not read again.

**My first "final" browser run failed for a reason I caused**: I pointed it at
`127.0.0.1` while the isolated server's `DEN_ORIGIN` said `localhost`. Reads
render and every write is rejected, which looks exactly like a broken composer.
Documented in `docs/THREAD-ACCEPTANCE.md` so the next person loses minutes
instead of a run.

**My fit check tested one axis.** Reading the screenshot from my own fix, I
realised a wrapping header can push a control *below the fold* rather than past
the edge — equally unreachable, and it would have passed. Measured vertically
(fine), then strengthened the assertion to both axes and to recording each
control's actual box rather than a boolean.

## Two harness defects that had been making runs quietly meaningless

- The Electron runner adopted the newest `workshop-*` channel, which had grown
  to **73 replies** from earlier runs. Every run was a different test and the
  keyboard walk eventually could not finish at all. Each runner now builds its
  own fixture. Keyboard now lands "Back to room" in 19 stops.
- `BASELINE=1` had no early exit, so it would have run into assertions an
  unchanged #42 client cannot satisfy. The preserved baseline evidence came from
  a version that stopped after the viewport shapes; that behaviour is restored.

## State and next human actions

- **#43 triggers no CI workflow.** Its files — `.mjs` runners and a doc — fall
  outside every workflow's path filter. That is expected, not a missing gate,
  and its runners are manual by design. Its correctness rests on the runs in
  `web-ui-final-acceptance/`.
- **#44 CI**: green on head `9e6633c` — `Web client` **success**, `Electron
  desktop` **success**, both on the `pull_request` event.
- #41 remains a draft with its separately documented standalone-host **SIGINT**
  shutdown failure. It was not investigated and not exercised. No run tonight is
  a control for it: the PTY cleanup observed here was a clean **SIGTERM**.
- Nothing about Mac, device, or OS-toast behaviour was tested, and nothing here
  claims it was.
- **The peer never received my replies.** Both messages I sent to
  `impl-thread-contract` tonight were held for recipient approval and not
  delivered — one expired, one was still pending. So they do not know the fix
  landed, do not have the new freeze, and the delta re-review they asked for
  ("send a new freeze when it is in and I will re-review just that delta") did
  **not** happen. Their review of `8f1e3bb7` is real and is what found the
  paging bug; everything after that point is unreviewed by them.

  I published anyway on their explicit "not blocking publication in my view once
  the loadingOlder clear is in", and both PRs are unmerged precisely so this is
  still correctable. The delta is written up for them at
  `den-thread-web-state-evidence/FOR-IMPL-THREAD-CONTRACT.md`, including the one
  judgement call I most want a second opinion on: leaving `exhausted` alone at
  logout while clearing `loadingOlder`.
- The peer lane (`wt-thread-native` / `feat/thread-native`) owns native iOS. I
  offered to review their PR; nothing had arrived by the time of writing, and
  given the delivery failure they may not have received that offer either.

## Left running

The isolated fixture is still up in case the morning wants to poke at it, and is
safe to kill:

    den-server on 127.0.0.1:17061  (scratch DB, DEN_WEB_DIR -> the built client)
    den-host    pointed at it via DEN_HOST_CONFIG_DIR, for the PTY step
    Xvfb :78

None of it touches the shared server on :7000, the :5173 dev client, or the real
keyring. Every Electron run used a disposable Secret Service under its own
`dbus-run-session` and a throwaway profile, discarded with its root.

Both PRs want a human read before merging, in order: #43 then #44.
