# Track 3: terminal and upload complaints

Branch `fix/terminal-uploads`. Read `docs/night/README.md`, `docs/M7B-NOTES.md`, and `docs/M1-NOTES.md`.

Nicholas's words from #issues:

1. **"why does the terminal record by default, thats cool but not very useful- maybe have an option for that?"** — recording is currently unconditional. Make it opt-in per session: a toggle in the terminal's header, default **off**, remembered per host in the user's account settings. When off, the server stores no recording and the ended card shows no play control. Add a line to Settings under Machines saying what recording does and where the files go. Existing recordings stay readable.

2. **"herdr in den's terminal is not exactly usable with clicking"** — mouse events are not forwarded to the PTY, so any TUI that expects clicks is crippled. Forward mouse input: when the terminal has focus and the program has enabled a mouse tracking mode, translate clicks, drags and wheel into the escape sequences the PTY expects, and let Ghostty's handler tell you when tracking is on. Wheel scrolling must still scroll scrollback when tracking is off. Verify specifically by running `herdr` inside a Den terminal and clicking between its panes and tabs.

3. **"when i was in the middle of uploading a clip i went to a different channel/hangout. when i came back it was gone/didnt persist"** — pending uploads live in the composer component, so navigating away destroys them. Move in-flight uploads into a store that outlives the view, keyed by channel, so switching rooms and coming back shows the same upload still going with its progress. Uploads continue in the background while you are elsewhere. Show a small indicator on that room's sidebar row while one is in flight. A reload legitimately ends them; say so in the notes rather than faking resume.

PR title "Terminal recording opt-in, mouse support, uploads that survive navigation". Before and after for each, plus a capture of clicking around herdr inside a Den terminal and one of leaving a room mid-upload and returning. Post `[astra-terminal] PR open` to fable.
