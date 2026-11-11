-- Keep artwork separate from scan results. Media folders can be read-only.
-- No foreign key: scan grouping can remove and restore the same album ID.
CREATE TABLE artwork (
    kind TEXT NOT NULL CHECK (kind IN ('album', 'artist')),
    target_id TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('manual', 'automatic')),
    content_type TEXT NOT NULL,
    data BLOB NOT NULL,
    updated_by TEXT,
    PRIMARY KEY (kind, target_id, source)
);

CREATE TABLE library_editors (
    account_id TEXT PRIMARY KEY NOT NULL REFERENCES accounts(id) ON DELETE CASCADE
);
