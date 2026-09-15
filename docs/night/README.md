# Overnight work, 2026-09-15

Nicholas dropped issues and ideas in Den's #issues and #ideas. Everything below traces
to one of those, to `docs/UI-REVIEW-2026-09-15.md`, or to his message asking for Electron.

**Working rules for every lane tonight**
1. Work on a branch named in your brief. Never push to `main`.
2. Open a pull request when the work is done and verified, with `gh pr create`.
3. **Every PR that changes something visible must show before and after.** Capture the
   same screen, same viewport, same theme, before your change and after it. Put both
   images in the PR body with `![before](...)` and `![after](...)`. Upload them by
   committing them under `docs/shots/pr/<branch>/` and linking the raw GitHub URL.
   For anything animated (dragging, a menu opening, a stream closing), record a short
   silent screen capture with ffmpeg and attach an mp4 or gif the same way.
4. A PR with no visual evidence for a visual change gets sent back.
5. Say plainly in the PR what you could not verify and why.
6. Human-scale tests, as always. Stage explicit paths.
