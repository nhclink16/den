CREATE TABLE spotify_accounts (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- ChaCha20-Poly1305 over the refresh token, with the user id as associated
    -- data so a row cannot be transplanted between users. No plaintext column,
    -- and access tokens are never stored at all.
    refresh_nonce BLOB NOT NULL,
    refresh_ciphertext BLOB NOT NULL,
    -- Key generation, so a rotated data key invalidates rather than mis-decrypts.
    key_version INTEGER NOT NULL DEFAULT 1,
    account_name TEXT,
    scopes TEXT NOT NULL,
    connected_at INTEGER NOT NULL,
    -- connected_at + 180 days. Advisory; a rejection from Spotify is authoritative.
    expires_at INTEGER NOT NULL,
    -- Set when Spotify rejects the refresh token. Drives the reauthorize state.
    needs_reauth INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE jams (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    host_id TEXT NOT NULL REFERENCES users(id),
    started_at INTEGER NOT NULL,
    ended_at INTEGER
);
-- One live Jam per room; ended rows stay for history without blocking a new one.
CREATE UNIQUE INDEX jams_live ON jams(channel_id) WHERE ended_at IS NULL;
CREATE TABLE jam_joins (
    jam_id TEXT NOT NULL REFERENCES jams(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    joined_at INTEGER NOT NULL,
    PRIMARY KEY (jam_id, user_id)
);
