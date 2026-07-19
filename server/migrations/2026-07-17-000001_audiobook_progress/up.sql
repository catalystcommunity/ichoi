CREATE TABLE audiobook_progress (
    -- A real account id, or the reserved `__guest__` profile while no accounts exist.
    account_id TEXT NOT NULL,
    track_id   TEXT NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position_ms BIGINT NOT NULL DEFAULT 0,
    completed  INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (account_id, track_id)
);

CREATE INDEX idx_audiobook_progress_track ON audiobook_progress(track_id);

-- The account column intentionally cannot be a foreign key because the global guest profile
-- is not an account. Preserve normal account-deletion cleanup explicitly.
CREATE TRIGGER cleanup_audiobook_progress_account
AFTER DELETE ON accounts
BEGIN
    DELETE FROM audiobook_progress WHERE account_id = OLD.id;
END;
