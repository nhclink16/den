-- Threads storage. Additive: no existing row is rewritten and no history is
-- reinterpreted. Roots keep messages.thread_id NULL; only replies carry it.
CREATE TABLE threads (
    id TEXT PRIMARY KEY NOT NULL,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    root_message_id TEXT NOT NULL UNIQUE REFERENCES messages(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    resolved_at TEXT,
    resolved_by TEXT REFERENCES users(id),
    -- Resolve records both who and when, or neither. There is no archive column.
    CHECK((resolved_at IS NULL) = (resolved_by IS NULL))
);
CREATE INDEX threads_channel ON threads(channel_id,id);
ALTER TABLE messages ADD COLUMN thread_id TEXT REFERENCES threads(id) ON DELETE CASCADE;
CREATE INDEX messages_channel_thread ON messages(channel_id,thread_id,id);
-- Read markers are sortable positions, not references: deleting a reply must not
-- undo what somebody already read.
CREATE TABLE thread_read_state (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    last_read_id TEXT,
    following INTEGER NOT NULL DEFAULT 1 CHECK(following IN (0,1)),
    PRIMARY KEY(user_id,thread_id)
);
-- A job's runner supplies task_id. It is never inferred from bot identity, token,
-- agent session or elapsed time, so two jobs from one bot stay independent.
CREATE TABLE thread_tasks (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL,
    thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    PRIMARY KEY(user_id,channel_id,task_id)
);
