CREATE TABLE devices (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    token_id TEXT REFERENCES tokens(id) ON DELETE CASCADE,
    platform TEXT NOT NULL CHECK (platform = 'ios'),
    token TEXT NOT NULL,
    environment TEXT NOT NULL CHECK (environment IN ('sandbox', 'production')),
    app_version TEXT NOT NULL,
    registered_at INTEGER NOT NULL,
    generation INTEGER NOT NULL DEFAULT 1,
    CHECK ((session_id IS NULL) <> (token_id IS NULL)),
    UNIQUE(environment, token)
);
CREATE INDEX devices_user ON devices(user_id);

CREATE TABLE call_invitations (
    channel_id TEXT PRIMARY KEY REFERENCES channels(id) ON DELETE CASCADE,
    from_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL
);
CREATE TABLE call_invite_recipients (
    channel_id TEXT NOT NULL REFERENCES call_invitations(channel_id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    declined INTEGER NOT NULL DEFAULT 0 CHECK (declined IN (0, 1)),
    PRIMARY KEY(channel_id, user_id)
);
