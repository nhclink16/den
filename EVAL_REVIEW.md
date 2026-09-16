# Review of review-proposal.diff

Scope: `crates/den-server/src/uploads.rs` only. Patch not applied. Findings below are
from code inspection against the current implementation; the patched binary was not executed.

## 1. PATCH /uploads/{id} loses owner and visibility checks (confirmed)

- Affected line/operation: `chunk()` ownership load, current line 115:
  `let row = owned(&s, &a, &id).await?;` proposed as `load(&s, &id).await?;`.
- `owned()` calls `visible()` (channel must be visible, else 404 `not_found`) and then
  requires `owner_id == auth user` (else 403 `forbidden`). `load()` does neither.
- Concrete reproduction: alice begins an upload (`POST /uploads`, size 10), writes
  offset 0 (`PATCH /uploads/{id}` with `Upload-Offset: 0`). Bob, a different member who
  knows/guesses the upload ULID, sends `PATCH /uploads/{id}` with `Upload-Offset: 0`
  (or the current offset) and his own bytes.
- Expected vs actual: expected `403` with `{"error":"forbidden"}`, offset/file unchanged
  (this is exactly what `upload_resume_survives_uncommitted_bytes...` asserts for bob).
  Actual with the patch: `200`, bob's bytes overwrite alice's `.part` and advance the
  committed offset. A non-member of a DM channel could do the same because the
  `visible()` check is also gone (expected `404 not_found`).
- Regression-test idea: alice creates an upload; bob `PATCH`es at the correct offset and
  must get `403` with `error == "forbidden"`; `GET /uploads/{id}` offset and the `.part`
  bytes are unchanged. Add a DM variant where an outsider gets `404`.

## 2. Removing `file.set_len(offset)` leaves crash garbage (confirmed by inspection)

- Affected lines/operation: `chunk()` file write, current lines 132-133:
  `file.set_len(offset as u64).await?;` plus the comment
  `// A crash may leave bytes beyond the committed offset. Retries replace them.`
- Concrete reproduction: begin size-10 upload; `PATCH` offset 0 with `b"01234"`
  (committed offset 5); simulate a crash that leaves more than the remaining bytes,
  e.g. append 10 garbage bytes so `{id}.part` length is 15 while committed offset is 5;
  retry `PATCH` offset 5 with `b"56789"` and then `POST /uploads/{id}/complete`.
- Expected vs actual: expected truncation to 5 before the write, final file exactly
  `b"0123456789"` (length 10) and `complete == true`. Actual without `set_len`: seek to
  5 and overwrite only bytes 5-9, leaving bytes 10-14, so the stored file is 15 bytes;
  `complete` then fails `409` `conflict` (`Stored size mismatch`) or serves trailing
  garbage. The existing resume test only appends exactly 5 `b"crash"` bytes
  (length == size), so the overwrite happens to fit and would not catch this.
- Regression-test idea: after the first chunk, append a longer-than-remaining trailer
  (e.g. 10 bytes), retry the final chunk, complete, then `GET /uploads/{id}/file` and
  assert the exact 10 bytes; without the truncate this fails on length/content.
