-- Removing a member keeps their row so their messages still have an author.
-- A removed member cannot log in and has no sessions, tokens or devices.
ALTER TABLE users ADD COLUMN removed_at INTEGER;
