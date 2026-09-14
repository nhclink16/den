CREATE TABLE users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL COLLATE NOCASE UNIQUE,
    display_name TEXT NOT NULL,
    password_hash TEXT,
    avatar_url TEXT,
    bot INTEGER NOT NULL DEFAULT 0 CHECK(bot IN (0,1)),
    owner_id TEXT REFERENCES users(id),
    role TEXT NOT NULL CHECK(role IN ('admin','member'))
);
CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    secret_hash TEXT NOT NULL UNIQUE,
    csrf_hash TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE TABLE tokens (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    secret_hash TEXT NOT NULL UNIQUE
);
CREATE TABLE invites (
    id TEXT PRIMARY KEY NOT NULL,
    secret_hash TEXT NOT NULL UNIQUE,
    uses_left INTEGER NOT NULL CHECK(uses_left >= 0),
    expires_at INTEGER NOT NULL
);
CREATE TABLE categories (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE channels (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
    kind TEXT NOT NULL CHECK(kind IN ('text','dm')),
    position INTEGER NOT NULL DEFAULT 0,
    dm_key TEXT UNIQUE
);
CREATE TABLE channel_members (
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY(channel_id,user_id)
);
CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    author_id TEXT NOT NULL REFERENCES users(id),
    content TEXT NOT NULL,
    reply_to TEXT REFERENCES messages(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    edited_at TEXT
);
CREATE INDEX messages_channel_id ON messages(channel_id,id);
CREATE TABLE uploads (
    id TEXT PRIMARY KEY NOT NULL,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    owner_id TEXT NOT NULL REFERENCES users(id),
    message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    size INTEGER NOT NULL CHECK(size > 0),
    offset INTEGER NOT NULL DEFAULT 0 CHECK(offset >= 0 AND offset <= size),
    complete INTEGER NOT NULL DEFAULT 0 CHECK(complete IN (0,1)),
    touched_at INTEGER NOT NULL
);
CREATE INDEX uploads_message_id ON uploads(message_id);
