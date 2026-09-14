CREATE TABLE objects (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    message_id TEXT UNIQUE REFERENCES messages(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT '{}',
    version INTEGER NOT NULL DEFAULT 0,
    thumbnail_upload_id TEXT REFERENCES uploads(id) ON DELETE SET NULL,
    created_by TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX objects_channel ON objects(channel_id);
CREATE TABLE settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    canvas_enabled INTEGER NOT NULL DEFAULT 1 CHECK (canvas_enabled IN (0, 1))
);
INSERT INTO settings(id) VALUES(1);
