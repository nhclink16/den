CREATE TABLE call_fetch_tickets (
    digest TEXT PRIMARY KEY,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    generation INTEGER NOT NULL,
    invitation_id TEXT NOT NULL REFERENCES call_invitations(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL,
    UNIQUE(device_id,invitation_id)
);
CREATE INDEX call_fetch_tickets_expiry ON call_fetch_tickets(expires_at);
