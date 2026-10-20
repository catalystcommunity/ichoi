-- Who may use an account-owned browser share. A share without a row is open to everyone:
-- every signed-in account. `selected` limits it to the owner and the listed accounts.
CREATE TABLE player_share_access (
    player_id TEXT PRIMARY KEY NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    access    TEXT NOT NULL CHECK (access IN ('everyone', 'selected'))
);

-- Accounts an owner picked. They are kept while access is `everyone`, so the owner can
-- switch back to `selected` without losing the list.
CREATE TABLE player_share_accounts (
    player_id  TEXT NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    PRIMARY KEY (player_id, account_id)
);

CREATE INDEX idx_player_share_accounts_account
    ON player_share_accounts(account_id);
