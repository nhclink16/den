-- Jams are now voice/DM only. End any Jam pinned to a text channel.
UPDATE jams
   SET ended_at = strftime('%s', 'now')
 WHERE ended_at IS NULL
   AND channel_id IN (SELECT id FROM channels WHERE kind = 'text');

-- Whether the music queue was paused automatically when a Jam started.
-- The queue resumes only when jam_paused=1, so a user-initiated pause survives.
ALTER TABLE music_rooms ADD COLUMN jam_paused INTEGER NOT NULL DEFAULT 0;

-- Unix-second timestamp recording when the call first became empty for the
-- active Jam (used by the auto-end watcher). NULL means the call is not empty.
ALTER TABLE jams ADD COLUMN empty_since INTEGER;

-- Last time the watcher saw the host's Spotify playing, Unix seconds. NULL
-- falls back to started_at.
ALTER TABLE jams ADD COLUMN last_playing_at INTEGER;
