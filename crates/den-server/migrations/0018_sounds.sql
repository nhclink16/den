CREATE TABLE user_sounds (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    preferences TEXT NOT NULL
);
CREATE TABLE server_sounds (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    pack TEXT NOT NULL
);
