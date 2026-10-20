CREATE TABLE account_blocks (
    blocker_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    blocked_account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    created_at         TEXT NOT NULL,
    PRIMARY KEY (blocker_account_id, blocked_account_id),
    CHECK (blocker_account_id <> blocked_account_id)
);

CREATE INDEX idx_account_blocks_blocked_account
    ON account_blocks(blocked_account_id);
