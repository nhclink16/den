CREATE TABLE devices_next (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    token_id TEXT REFERENCES tokens(id) ON DELETE CASCADE,
    platform TEXT NOT NULL CHECK (platform = 'ios'),
    token TEXT NOT NULL,
    environment TEXT NOT NULL CHECK (environment IN ('sandbox', 'production')),
    purpose TEXT NOT NULL CHECK (purpose IN ('alert', 'voip')),
    client_id TEXT,
    app_version TEXT NOT NULL,
    registered_at INTEGER NOT NULL,
    generation INTEGER NOT NULL DEFAULT 1,
    CHECK ((session_id IS NULL) <> (token_id IS NULL)),
    CHECK (purpose <> 'voip' OR client_id IS NOT NULL),
    UNIQUE(environment, purpose, token)
);
INSERT INTO devices_next
SELECT id,user_id,session_id,token_id,platform,token,environment,'alert',NULL,
       app_version,registered_at,generation FROM devices;
DROP TABLE devices;
ALTER TABLE devices_next RENAME TO devices;
CREATE INDEX devices_user ON devices(user_id);
