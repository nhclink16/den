CREATE TABLE call_invitations_next (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    from_user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL,
    state TEXT NOT NULL DEFAULT 'ringing' CHECK (state IN ('ringing','active','cancelled','expired','ended')),
    ringing_expired INTEGER NOT NULL DEFAULT 0,
    finished_at INTEGER
);
-- Upgrade the single old invitation per channel without losing its recipients.
INSERT INTO call_invitations_next(id,channel_id,from_user_id,expires_at)
SELECT channel_id,channel_id,from_user_id,expires_at FROM call_invitations;
CREATE TABLE call_invite_recipients_next (
    invitation_id TEXT NOT NULL REFERENCES call_invitations_next(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    declined INTEGER NOT NULL DEFAULT 0 CHECK (declined IN (0,1)),
    answer_id TEXT,
    answer_credential TEXT,
    answer_session INTEGER,
    accepted_at INTEGER,
    joined INTEGER NOT NULL DEFAULT 0 CHECK (joined IN (0,1)),
    PRIMARY KEY(invitation_id,user_id),
    CHECK (answer_id IS NULL OR declined=0)
);
INSERT INTO call_invite_recipients_next(invitation_id,user_id,declined)
SELECT channel_id,user_id,declined FROM call_invite_recipients;
DROP TABLE call_invite_recipients;
DROP TABLE call_invitations;
ALTER TABLE call_invitations_next RENAME TO call_invitations;
ALTER TABLE call_invite_recipients_next RENAME TO call_invite_recipients;
CREATE UNIQUE INDEX call_invitation_current ON call_invitations(channel_id) WHERE state IN ('ringing','active');
CREATE INDEX call_invitation_cleanup ON call_invitations(finished_at);
