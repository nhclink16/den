# Eval summary

## Changes

- A. `crates/den-cli/src/main.rs`: added `den read CHANNEL --all`.
  - New `--all` flag; `check_read_args()` rejects `--before`+`--after` and
    `limit` outside 1..200 (plus ULID shape) before any HTTP in both modes.
  - Channel resolved once via `resolve_channel()`; one-page path unchanged.
  - `read_all()` pages with existing `limit` as page size: `--after` walks forward
    from the exclusive lower bound, `--before`/no-cursor walks backward from the
    newest page using `before = first_id`. Pages collected, deduped by id, printed
    once as a single ascending-by-id array. Later-page HTTP errors propagate via
    `anyhow` (nonzero exit, nothing printed to stdout). Full page with no cursor
    progress bails instead of looping. Added unit tests
    `read_args_reject_both_cursors_and_bad_limits` and
    `sort_messages_orders_ascending_and_dedups`.
- B. `crates/den-server/tests/api/uploads.rs`: two new integration tests, no
  implementation changes.
  - `upload_admission_rejects_bad_size_and_filename_without_side_effects`: size
    0/-5/2MiB give 413 `upload_size`; empty/slash/backslash/control/256-char
    filenames give 400 `invalid_request`; asserts `error` codes exactly and that
    rejected admissions leave 0 rows and 0 files; size 1 and 1048576 succeed.
  - `upload_pending_limit_is_per_owner_and_frees_on_complete`: 5 pendings then 6th
    gives 409 `conflict` with no new row/file; bob still gets 200; completing one
    alice upload (PATCH + complete) lets alice create again, pending back to 5.
- C. Wrote `EVAL_REVIEW.md` (2 findings, patch not applied).

## Exact checks / outcomes

- `cargo test -p den`: 2 passed, 0 failed.
- `cargo test -p den-server --test api uploads`: 5 passed (2 new + 2 existing
  upload tests + 1 portable match), 0 failed.
- `cargo check -p den`: ok.
- `cargo fmt -p den -p den-server`: applied; `cargo fmt --check` clean after.
- `git status`: modified `crates/den-cli/src/main.rs`,
  `crates/den-server/tests/api/uploads.rs`; untracked `EVAL_REVIEW.md` (+ this file).
  No commits (per instructions), no `den-core`/migration/dependency changes.

## Remaining concerns

- CLI `--all` verified only by unit tests for validation/sort, not end-to-end against
  a live server (no multi-page live pagination run); cursor-progress guards are
  reasoned, not exercised against a stub server.
- Upload tests use a fresh `Test` (max_upload 1MiB, local temp dir); disk-space
  reservation path (`507 insufficient_storage`) not covered.
- Review findings are code-inspection confirmed, not executed against a patched
  binary; cache-control hunk (`private, no-store` -> `no-store`) judged harmless and
  not flagged.

## Unfinished tasks

- None; A, B, and C complete as scoped.
