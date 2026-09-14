CREATE TABLE user_appearance (
    user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    appearance TEXT NOT NULL CHECK (json_valid(appearance))
);
