CREATE TABLE hosts (
 id TEXT PRIMARY KEY, owner_id TEXT NOT NULL REFERENCES users(id), name TEXT NOT NULL,
 token_hash TEXT NOT NULL UNIQUE, online INTEGER NOT NULL DEFAULT 0,
 last_seen INTEGER, direct_url TEXT
);
CREATE TABLE host_enrollments (
 code_hash TEXT PRIMARY KEY, owner_id TEXT NOT NULL REFERENCES users(id), expires_at INTEGER NOT NULL
);
CREATE TABLE grants (
 id TEXT PRIMARY KEY, host_id TEXT NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
 grantee_id TEXT NOT NULL REFERENCES users(id),
 capability TEXT NOT NULL CHECK(capability IN ('terminal_view','terminal_control')),
 expires_at INTEGER, created_by TEXT NOT NULL REFERENCES users(id), created_at INTEGER NOT NULL, revoked_at INTEGER
);
CREATE INDEX grants_access ON grants(host_id,grantee_id);
CREATE TABLE access_log (
 id TEXT PRIMARY KEY, host_id TEXT NOT NULL, owner_id TEXT NOT NULL REFERENCES users(id),
 actor_id TEXT NOT NULL REFERENCES users(id), action TEXT NOT NULL, subject_id TEXT, created_at INTEGER NOT NULL
);
CREATE TABLE terminal_sessions (
 id TEXT PRIMARY KEY REFERENCES objects(id) ON DELETE CASCADE,
 host_id TEXT NOT NULL REFERENCES hosts(id) ON DELETE CASCADE,
 recording_upload_id TEXT REFERENCES uploads(id)
);
CREATE TABLE terminal_cards (
 object_id TEXT PRIMARY KEY REFERENCES objects(id) ON DELETE CASCADE,
 session_id TEXT NOT NULL REFERENCES terminal_sessions(id) ON DELETE CASCADE
);

-- Keep recordings protected even when their chat card or machine is removed.
CREATE TABLE terminal_recordings (upload_id TEXT PRIMARY KEY REFERENCES uploads(id) ON DELETE CASCADE, session_id TEXT NOT NULL);
