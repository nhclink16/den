-- Showing "Listening to …" to everyone is its own choice, separate from connecting
-- for Jams. Existing connections were made under the Jam-only promise, so off.
ALTER TABLE spotify_accounts ADD COLUMN share_listening INTEGER NOT NULL DEFAULT 0;
