-- The startup migrator disables foreign keys on its dedicated connection while
-- rebuilding this parent table, then checks all references before serving.
CREATE TABLE channels_new (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
    kind TEXT NOT NULL CHECK(kind IN ('text','dm','voice')),
    position INTEGER NOT NULL DEFAULT 0,
    dm_key TEXT UNIQUE
);
INSERT INTO channels_new SELECT * FROM channels;
DROP TABLE channels;
ALTER TABLE channels_new RENAME TO channels;
INSERT INTO channels(id,name,kind,position) VALUES('01' || hex(randomblob(12)),'hangout','voice',0);
CREATE TRIGGER messages_no_voice BEFORE INSERT ON messages
WHEN (SELECT kind FROM channels WHERE id=new.channel_id)='voice'
BEGIN
    SELECT RAISE(ABORT, 'Voice rooms do not accept messages');
END;
