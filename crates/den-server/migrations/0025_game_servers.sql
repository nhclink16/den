-- Game servers reported by a relay (the Minecraft relay first). The live state
-- is held in memory; these rows keep the sidebar entry, page facts and history
-- across a Den restart.
CREATE TABLE game_servers (
    slug TEXT PRIMARY KEY,
    -- ServerInfo JSON without the icon.
    info TEXT NOT NULL,
    icon BLOB,
    -- Latest all-time totals from the relay, JSON list of ServerPlaytime.
    playtime TEXT NOT NULL DEFAULT '[]',
    -- The API key that registered this server. Another key is refused until this
    -- one is revoked, which clears it.
    token_id TEXT REFERENCES tokens(id) ON DELETE SET NULL,
    updated_at INTEGER NOT NULL
);

-- One point per key per minute, pruned after about a day.
CREATE TABLE game_server_samples (
    slug TEXT NOT NULL REFERENCES game_servers(slug) ON DELETE CASCADE,
    key TEXT NOT NULL,
    at INTEGER NOT NULL,
    value REAL NOT NULL,
    PRIMARY KEY (slug, key, at)
);

-- Who was online and when, for "past 7 days" playtime. ended_at is NULL while
-- the player is on; seen_at bounds a session that a Den restart cut short.
-- Sessions that ended more than 30 days ago are deleted.
CREATE TABLE game_server_sessions (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL REFERENCES game_servers(slug) ON DELETE CASCADE,
    player TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    seen_at INTEGER NOT NULL,
    ended_at INTEGER
);
CREATE INDEX game_server_sessions_by_slug ON game_server_sessions(slug, ended_at);
