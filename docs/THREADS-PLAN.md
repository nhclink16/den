# Threads

Design for the milestone following PRs #30–32, based on SQLx 0.9.0 from #35.
`DIRECTION.md` is the product
decision: conversations reach an outcome, and agent work belongs with the
conversation that caused it. Threads are part of a room, not a second inbox.

## Rules before implementation

### A reply creates the conversation

- The first new reply to a message creates its thread in the same transaction
  as the reply. Concurrent first replies create one thread, enforced by a unique
  root-message key. Clicking Reply can show the composer before anything is saved.
- A reply to a thread reply stays in that same thread. There are no nested threads.
- The original message stays in the room. Its thread has an inferred title,
  initially the first nonempty line of the root, whitespace collapsed and limited
  to 80 characters. An object-only root uses its object's name. A fallback is
  "Conversation". Renaming the thread does not edit the root message.
- Titles are snapshots. Later root edits do not overwrite a chosen title.
- Existing messages and old quoted replies stay where they are. The migration
  does not reinterpret history, move message IDs, or rewrite read positions.
- All members who can see the channel can see and participate in its threads.
  Following a thread affects unread state, never access. Voice rooms cannot host
  threads. DMs inherit their existing exact membership checks.

### Resolve, never archive

- Threads stay open until an explicit Resolve action. No inactivity timer,
  expiry, automatic archive, or automatic resolution from an agent disconnect.
- Any member of the channel may rename, resolve, or reopen its shared threads.
- Resolving retains messages, attachments, objects, search results and read state.
  It removes the thread from the open strip, not from history. Resolved threads
  remain reachable from their root, search, a direct link, and a resolved list.
- New messages or object cards in a resolved thread return a conflict. Reopening
  is explicit; a delayed agent update must not silently undo someone's resolution.
- Resolving is a conversation action. It does not stop a PTY, revoke a terminal
  grant, or freeze an existing canvas. Existing object permissions still apply.
- Deleting a message that roots a thread is refused. This preserves the whole
  conversation and avoids making one message-delete operation delete other
  people's replies or live objects. Other deletions keep the existing rules,
  including the live-terminal-card guard. Channel deletion retains its existing
  authority and the reconciliation protection from #30.

### One unread total, separate positions for what was actually read

**A channel's unread total is unread main-conversation messages plus relevant
unread thread replies, counted once each. Reading the room never reads a collapsed
thread. Reading one thread never reads another thread or the main conversation.**

- A thread is relevant when the user follows it, when it is in a DM they belong
  to, or when they subscribed to notifications for its channel.
- Creating a thread follows it for the root author and first participant.
  Replying or receiving a mention in a new message follows it for that user.
  Opening or reading does not follow it: inspecting a conversation must not undo
  an explicit Unfollow. Follow and Unfollow actions are available.
- When someone first joins an existing thread, their starting position is the
  last reply before the action that brought them in. A new mention is therefore
  unread, without turning the entire old conversation into new unread activity.
  Explicit Follow starts at the current tail; it is not a request to notify about
  historical messages. Existing read positions only move forward.
- Opening loads messages, then reads through the displayed tail. An empty thread
  uses its root ID as the watermark, so a reply arriving between those requests
  stays unread. GET endpoints have no read/follow side effects.
- Newly subscribing to a channel starts previously irrelevant threads at their
  current tails. Preserve unread in threads already relevant through following
  or DM membership. Changing another notification preference does not reset any
  position. A missing thread position never falls back to the moving room cursor.
- Mention-follow belongs to new-message creation, not mention indexing shared by
  edits and startup backfill. Edited mentions retain the existing no-new-alert
  policy; they do not enroll someone into a previously irrelevant thread.
- Own messages never count. Resolving does not clear unread messages. An unread
  resolved thread remains reachable from the channel's unread view.
- Notification preferences retain their current meaning: mentions, DMs, and
  subscribed channels may notify. Following alone adds unread activity, not new
  unsolicited system notifications. A message qualifying in several ways still
  contributes one count and at most one notification.
- Each thread has its own per-user position. The existing channel position is the
  main-conversation position for the new clients. The channel total and all badges
  remain server-authoritative; the client must stop optimistically setting the
  whole channel to zero when only its main conversation was read.
- The unread thread list uses exactly the same relevance/count predicate as the
  channel total and includes resolved threads. Clearing the badge must not require
  paging through unrelated resolved history.
- A room-wide "Mark all read" is explicit and advances both main and relevant
  thread positions to a captured tail. Later arrivals remain unread.

### Agent tasks group themselves without guessing what a task is

- A task has a stable opaque `task_id`, scoped to the authenticated author and
  channel. It identifies one job, not the bot, API token, process, or long-lived
  agent session. Two jobs from the same bot must remain independent.
- The first task message supplies the root automatically. If it replies to an
  existing request, that request is the root instead. Later messages with the
  same task ID go to the same thread without repeating its ID or creating it.
- Canvas and terminal cards accept the same context and join the same thread.
  The first item may itself be an object card. Posting and creating the task
  mapping are atomic; a rejected upload or invalid destination creates neither.
- An explicit thread/reply target must agree with an existing task mapping.
  Conflicting context is an error, not permission to redirect an established job.
- Completion calls Resolve explicitly. Reusing a resolved task ID is a conflict;
  a new job gets a new ID. Retrying or reconnecting never starts another thread.
- The CLI accepts task context from `DEN_TASK_ID` or `--task`, and explicit
  destinations from `--thread`. A runner establishes the task ID once per job;
  all sends and object creation inherit it. The Den skill documents this as the
  normal workflow, including carrying the triggering message as the first reply.
- No tracked Hermes/OpenClaw adapter exists in this tree. Start with the CLI;
  external adapters must pass their real task/run identity through the same API.
  Unscoped bot messages remain ordinary messages. Silently grouping them by time
  or bot identity would merge unrelated work, so that is deliberately not inferred.

## Mixed-version rollout, and what a flat read means

A channel read with `roots_only` absent or false is deliberately a **flat
acknowledgement**: it marks the main conversation and every thread of that channel
read through the supplied marker. That is what an old client displaying one
timeline means by it, and it is also what an explicit room-wide "Mark all read"
means. The server cannot tell those two apart from the request body and must not
try — no client-version heuristic, no silently different meaning.

The consequence is worth stating plainly rather than discovering later. Read
positions are shared across a person's devices, so **an old client showing the flat
timeline can acknowledge thread replies that an updated device is keeping
collapsed.** Independent thread unread is therefore not reliable for an account
until that account's active clients are updated — and that includes an already-open
web tab, which keeps running old code until it is reloaded, and an installed native
app until it is actually updated. Publishing the server PRs is not a deployment
instruction, and shipping web and native together still cannot upgrade a session
already running.

Do not try to bridge this by sending `roots_only=true` from a client that still
renders the flat timeline. That client has no thread UI, so the replies it has
already shown the reader would stay unread with nothing available to clear them.

## Compatibility checked in current code

- `den_core::Event` has the receive-only `#[serde(other)] Unknown` fallback.
  `den tail` skips it and has a regression that receives a known event afterward
  on the same socket. Unknown fields on known Rust message types are tolerated.
- The web client dispatches by `ev.type`; unknown tags fall through. Native iOS
  routes by the JSON tag before decoding payloads and has `default: break`.
  Its generation schema removes `additionalProperties: false`, so new optional
  fields do not reject otherwise valid known responses.
- This proves source compatibility for these current clients, not that every
  installed binary has been upgraded. Ungated thread events require the #22
  Rust consumer baseline and the tolerant iOS response baseline. Pre-catch-all
  binaries must be upgraded or retired before deploying these events. Keep the
  existing music/sounds gates. Do not add a threads query gate.
- Existing `MessageCreated`/`MessageEdited` events continue carrying messages,
  including thread replies. New optional `thread_id` and root thread summary
  fields let new clients place them. Older tolerant clients retain a flat view.
- Channel message listing remains flat by default for old clients. New clients
  request `roots_only=true`; threads have their own paginated message endpoint.
  Legacy channel read calls retain their flat-view meaning. New calls explicitly
  mark only roots, and thread reads use the thread endpoint. These are data scopes,
  not WebSocket feature gates.
- New thread metadata events are ordinary channel-authorized variants. Per-user
  thread read-state events are restricted to that user. No private DM metadata
  may leak through a global thread event or lookup.

## Storage and contract

Add migration `0019_threads.sql`. Leave applied migrations unchanged.

- `threads`: its own ULID, channel, unique root message, title, creator,
  creation time, nullable resolution time/actor. The two resolution fields must
  both be null or both present. Root and channel references cascade on channel
  deletion; the HTTP root-message deletion guard preserves individual threads.
- `messages.thread_id`: nullable FK to threads. Null identifies main-conversation
  messages, including roots. A composite channel/thread/message index supports
  both timelines. Server validation enforces matching channel and one-level roots.
- `thread_read_state`: user/thread key, nullable last-read ID, following boolean.
  Positions survive unfollow and resolution. They are not membership permissions.
- `thread_tasks`: author/channel/task-ID key pointing to a thread. This persists
  task identity across reconnects and server restarts without a time heuristic.
- Objects continue belonging to their message. Do not add a second independent
  thread assignment to the objects table. Terminal authorization is unchanged.
- Offline export/restore must retain all these rows and relationships.

Shared types in `den-core` precede implementation. Add `ThreadSummary`,
`ThreadReadState`, `ThreadView`, thread-list query and metadata/follow update inputs.
`ThreadSummary` includes root ID, channel, title, creator, timestamps, resolution,
reply count, last reply ID and last activity. It contains no personal read state.
`ThreadView` pairs the summary with the requesting user's read state.
`MarkThreadRead` carries only a message watermark; reading does not follow.

Add optional/defaulted context fields to message creation, object creation,
terminal open/share, typing, and message responses. Add the root-only query/read
scope with legacy-compatible defaults. Generate client schemas from the actual
server OpenAPI; do not hand-maintain parallel request or response types.

Expected routes:

- `GET /channels/{id}/threads` lists all threads. `resolved=false` selects the
  open strip, `resolved=true` selects resolved history. `unread_only=true` selects
  threads contributing to the caller's channel unread total. The filters combine
  with AND; omitting the resolution filter includes unread resolved threads.
- `GET /threads/{id}` returns metadata and the caller's read state.
- `GET /threads/{id}/messages` paginates replies; `/messages/{root}` gets the root.
- `PATCH /threads/{id}` renames, resolves, or reopens.
- `PUT /threads/{id}/read` advances this thread's read position.
- `PUT /threads/{id}/follow` follows/unfollows.
- Existing message/object creation accepts thread/task context. No separate
  manual thread-creation endpoint or naming dialog is needed.

## Reviewable delivery order

1. **Contract and storage.** This plan, migration, shared types, generated schema,
   and only the compatibility changes needed to keep current consumers compiling.
   No thread behavior enabled yet. Prove fresh/upgrade migration and unchanged
   old request decoding, including unknown-event compatibility.
2. **Server conversations and agent grouping.** Atomic first reply and task
   routing, resolve/reopen, permissions, metadata and shared event delivery.
   This stage retains the existing flat read accounting; no threaded UI ships yet.
3. **Server unread model.** Independent thread positions, aggregate channel totals,
   follow actions, filtered unread listing and private read-state events. This is
   reviewed separately because a mistake can silently acknowledge unread content.
   A data-only migration captures existing flat channel watermarks into thread
   positions before their meaning changes. Later flat/mark-all reads advance both
   scopes; roots-only reads advance only the room. Do not introduce a second
   channel cursor or make missing thread positions follow the live room cursor.
4. **Objects and agent client.** Thread-aware terminal/canvas creation and sharing,
   CLI task context and thread commands, and agent skill instructions. This makes
   task grouping useful before polishing the room UI.
5. **Web/Electron.** Persistent collapsed open-thread strip, root badges, thread
   conversation, resolve/reopen/rename, read/follow behavior, search/deep links and
   original object renderers. Preserve #32 for both room and thread composers.
6. **Native iOS.** Included in this milestone as a separate client PR. Use the same
   unread and lifecycle rules, native conversation navigation, and real simulator
   acceptance. The flat compatibility path supports the staged rollout; it is not
   the final native experience.

Each PR depends on its predecessor where necessary and contains only its own
piece. Open for human review; do not merge. At most two Opus implementation lanes,
separate worktrees, explicit owned paths. Schema/server/store/composer ownership
is sequential where files overlap. Astra owns design, acceptance, and diff review.

## Acceptance that can disprove the design

- Two simultaneous first replies make one thread. Replying to a reply cannot nest.
  Invalid uploads, wrong-channel replies and inaccessible DMs leave no partial rows.
- Two concurrent task IDs from one bot produce separate threads; reconnect/restart
  with the same task ID continues the original. An established mapping cannot be
  redirected, and a resolved task cannot silently reopen itself.
- Resolve, refetch/restart, rename and reopen retain content. No passage of time
  changes resolution. Resolving a thread with a real retained PTY does not kill it.
- An unread thread reply survives reading a newer main-conversation message.
  Reading one of two threads clears only that one. Channel and thread reads on
  two clients converge, including a reply arriving across a mark-all-read request.
  Own messages, mentions, subscriptions and DMs count once under the written rule.
- Thread lookup, list, mutations, search and WS delivery enforce the parent
  channel's access. Test a nonmember against a private DM in REST and WebSocket.
- A task's real terminal and canvas cards land inside its thread, remain usable
  with existing grants, and survive export/restore. Channel deletion retains #30's
  orphan-session reconciliation. Do not modify host reconciliation to add threads.
- An unknown thread event followed by a known message leaves tolerant consumers
  connected. Legacy flat listing and mark-read still work without new parameters.
- Browser acceptance uses two people and an agent: a request becomes a thread,
  progress collapses, objects work, unread is coherent, resolve removes only the
  open-strip entry, and reopen works. Capture before/after images and actual API
  state. Verify desktop landscape/portrait transitions and narrow-window navigation.
- Delay sends in both room and thread composers while typing newer drafts, changing
  reply context or switching threads. Preserve #32 and #29's attachment contract.
- Focused regressions first, then relevant existing suites once. Use SQLx 0.9.0
  and the restored normal timeout from #35. Sequence query/metadata generation
  between lanes. Do not change dependencies or timeouts as part of threads.
