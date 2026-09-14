CREATE TABLE reactions (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    emoji TEXT NOT NULL,
    PRIMARY KEY(message_id,user_id,emoji)
);
CREATE TABLE read_state (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    last_read_id TEXT NOT NULL,
    PRIMARY KEY(user_id,channel_id)
);
CREATE TABLE notification_preferences (
    user_id TEXT PRIMARY KEY NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    mentions INTEGER NOT NULL DEFAULT 1 CHECK(mentions IN (0,1)),
    dms INTEGER NOT NULL DEFAULT 1 CHECK(dms IN (0,1))
);
CREATE TABLE channel_subscriptions (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    PRIMARY KEY(user_id,channel_id)
);
CREATE TABLE message_mentions (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY(message_id,user_id)
);
ALTER TABLE messages ADD COLUMN mentions_indexed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE uploads ADD COLUMN thumbnail_ready INTEGER NOT NULL DEFAULT 0;
CREATE VIRTUAL TABLE message_search USING fts5(content,content='messages',content_rowid='rowid',tokenize='unicode61');
CREATE TRIGGER messages_fts_insert AFTER INSERT ON messages BEGIN
    INSERT INTO message_search(rowid,content) VALUES(new.rowid,new.content);
END;
CREATE TRIGGER messages_fts_delete AFTER DELETE ON messages BEGIN
    INSERT INTO message_search(message_search,rowid,content) VALUES('delete',old.rowid,old.content);
END;
CREATE TRIGGER messages_fts_update AFTER UPDATE OF content ON messages BEGIN
    INSERT INTO message_search(message_search,rowid,content) VALUES('delete',old.rowid,old.content);
    INSERT INTO message_search(rowid,content) VALUES(new.rowid,new.content);
END;
INSERT INTO message_search(message_search) VALUES('rebuild');
