-- Until now a channel's last_read_id has meant the flat timeline, thread replies
-- included, because that is the only timeline any client could display. The read
-- model landing with this migration reinterprets it as the main conversation's
-- position alone, so the flat fact has to be materialized into per-thread
-- positions here, at the moment its meaning changes. Afterwards a thread with no
-- row is a static floor and never follows the channel cursor.
--
-- Data only: no message, root, task mapping or read_state row is rewritten, and
-- no flat_read_id column is added. Markers are sortable positions, so one need
-- not reference a message that still exists.
INSERT INTO thread_read_state(user_id, thread_id, last_read_id, following)
SELECT r.user_id, t.id, r.last_read_id, 0
FROM read_state r
JOIN threads t ON t.channel_id = r.channel_id
-- Every thread of the channel, resolved ones included: resolution changes no read
-- position, so a resolved thread that was already read must not come back unread.
-- SQLite needs this WHERE to tell the SELECT apart from the upsert clause.
WHERE true
ON CONFLICT(user_id, thread_id) DO UPDATE SET
    -- A later position somebody already holds wins, but max(NULL, x) is NULL in
    -- SQLite, so a saved NULL has to take the channel marker rather than blank it.
    last_read_id = CASE
        WHEN thread_read_state.last_read_id IS NULL THEN excluded.last_read_id
        WHEN thread_read_state.last_read_id < excluded.last_read_id THEN excluded.last_read_id
        ELSE thread_read_state.last_read_id
    END;
-- following is deliberately absent from the update: an affiliation phase 2a wrote
-- is preserved, and a row created here is following=0 explicitly, despite the
-- column's default of 1, because a flat read is not somebody joining a thread.
