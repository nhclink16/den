# Cleanup brief: test artifacts in production

Owner: Astra. Small task.

1. In production (`https://denchat.app`, use the m6 credentials file and the admin CLI token as before), delete the smoke artifacts: every ended `terminal` card and its recording posted by `nicholas`, `m6_bob`, or `m6_ari` in `#general`, `#plans`, and their DMs; the `canvas` smoke objects named `smoke`; the `Desktop smoke` and `M3 smoke` messages; the access-request cards in the nicholas ↔ m6_bob DM; the `M9 sea glass` custom theme on `m6_bob`; and the `Den-M4-*` temporary servers' leftovers if any touched production. Do not touch anything authored by a real person that is not obviously a smoke artifact; when unsure, list it in the notes instead of deleting. Back up first with `den-backup`.
2. Make every smoke script (`m1`, `m3`, `m6-turn`, `m7a`, `m7b`, `m7c`, `m8-host`, `m9`, `m4` native stub) delete what it creates at the end, including on failure via a `finally`, unless `DEN_SMOKE_KEEP=1` is set. Verify by running each against the dev server and asserting the room has no new messages, objects, or requests afterward.
3. Write `docs/CLEANUP-NOTES.md` with what was deleted, what was left and why, then `herdr agent prompt fable "[astra-m8] cleanup done"`.
