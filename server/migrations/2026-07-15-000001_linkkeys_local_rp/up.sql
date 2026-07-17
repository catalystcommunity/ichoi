CREATE TABLE linkkeys_local_rp_identities (
    fingerprint     TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    identity_bundle BLOB NOT NULL,
    active          INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL,
    expires_at      TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_linkkeys_local_rp_one_active
    ON linkkeys_local_rp_identities(active) WHERE active = 1;

CREATE TABLE linkkeys_trusted_identities (
    domain     TEXT NOT NULL,
    handle     TEXT NOT NULL DEFAULT '',
    source     TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (domain, handle)
);

CREATE TABLE linkkeys_login_attempts (
    attempt_sha256  TEXT PRIMARY KEY NOT NULL,
    pending_login   TEXT NOT NULL,
    expected_handle TEXT,
    created_at      TEXT NOT NULL,
    expires_at      TEXT NOT NULL
);

CREATE TABLE linkkeys_login_exchanges (
    code_sha256 TEXT PRIMARY KEY NOT NULL,
    account_id  TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL,
    expires_at  TEXT NOT NULL
);
