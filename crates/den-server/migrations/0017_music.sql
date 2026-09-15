CREATE TABLE music_rooms (
    room_id TEXT PRIMARY KEY REFERENCES channels(id) ON DELETE CASCADE,
    paused INTEGER NOT NULL DEFAULT 0,
    position_seconds REAL NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0,
    epoch INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE music_queue (
    id TEXT PRIMARY KEY,
    room_id TEXT NOT NULL REFERENCES music_rooms(room_id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    title TEXT NOT NULL,
    duration REAL,
    thumbnail TEXT,
    added_by TEXT NOT NULL REFERENCES users(id),
    position INTEGER NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('queued','loading','playing','paused','failed'))
);
CREATE INDEX music_queue_order ON music_queue(room_id,position);
