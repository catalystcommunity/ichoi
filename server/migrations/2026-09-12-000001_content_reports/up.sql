CREATE TABLE content_reports (
    id                  TEXT PRIMARY KEY NOT NULL,
    reporter_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    target_type         TEXT NOT NULL CHECK (target_type IN ('playlist', 'account')),
    target_id           TEXT NOT NULL,
    reason              TEXT NOT NULL CHECK (reason IN (
                            'objectionable-content', 'harassment', 'spam', 'other'
                        )),
    details             TEXT,
    status              TEXT NOT NULL DEFAULT 'open'
                        CHECK (status IN ('open', 'resolved', 'dismissed')),
    created_at          TEXT NOT NULL,
    resolved_at         TEXT
);

CREATE INDEX idx_content_reports_status_created
    ON content_reports(status, created_at DESC, id);
CREATE INDEX idx_content_reports_target
    ON content_reports(target_type, target_id);
