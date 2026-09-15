# Track 2: the call and stream complaints

Branch `fix/call-streams`. Read `docs/night/README.md`, `docs/M3-NOTES.md`, `docs/M7C-NOTES.md`, and the call components under `apps/web/src/ui/`.

Every item here is Nicholas's own words from Den's #issues and #ideas. Fix all of them.

1. **"cant turn down someone elses stream volume"** — there is no per-participant volume anywhere. Add a volume control per remote participant: a slider in the tile's overflow menu, and the same control in the member list row for anyone in the call. It sets the `volume` on that participant's audio element, persists per participant in localStorage, and shows a muted glyph at zero. A "Mute for me" toggle alongside it, which is distinct from muting their microphone for everyone.

2. **"if im streaming multiple things it doesnt let me close out of just one from the hangout"** — the sharer can only stop all shares at once. Each of your own share tiles must have its own stop control that ends only that track, reachable from the tile itself and from the call dock's share menu. The main share button keeps stopping everything.

3. **"the icon in the top left of them is weird, what even is that"** and **"streams should show the icon of the app here and the name of the app instead of window:37438947"** — share tiles are labelled with the raw track id. Use the capture's real source name from `MediaStreamTrack.getSettings().displaySurface` and the track label, mapped to something human: "Helium — docs", "Terminal", "Screen 1". Where the browser gives a `displaySurface` of `monitor`, `window` or `browser`, pick a matching glyph rather than the current mystery icon. Never show a numeric window handle to a person. If the browser gives nothing usable, fall back to "Screen" or "Window", not an id.

4. **"can it have more mobility when not in fullscreen, dragging down so it takes up as much space as we want it to"** — the docked call strip above the chat has a fixed height. Give it a drag handle on its bottom edge so it can be resized from about 100px up to most of the column, remembered per room, with a double-click to reset. M7c already does this for the object dock; reuse that pattern and the same keyboard affordance.

5. **"should the hangout be above #issues"** — voice rooms sort below text rooms regardless of their position. Voice rooms belong at the top of the sidebar, above the first category, since that is where people look for them. Keep their relative order by position.

Do not touch `apps/web/src/ui/Appearance.svelte`, `ThemeCard.svelte`, `ThemePreview.svelte`, `BackgroundSettings.svelte` or `SettingRow.svelte`; Fable owns those tonight.

PR title "Calls: per-person volume, per-share stop, real share names, resizable dock". Before and after images for every one of the five, and a short capture of dragging the dock and of stopping one share while another keeps running. Post `[astra-streams] PR open` to fable.
