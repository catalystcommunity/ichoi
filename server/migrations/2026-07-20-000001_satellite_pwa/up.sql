ALTER TABLE output_devices ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;

CREATE TABLE access_groups (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE
);
INSERT INTO access_groups (id, name) VALUES ('everyone', 'Everyone');

CREATE TABLE account_access_groups (
    group_id TEXT NOT NULL REFERENCES access_groups(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    PRIMARY KEY (group_id, account_id)
);

CREATE TABLE satellite_tokens (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    token_sha256 TEXT NOT NULL UNIQUE,
    default_enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE TABLE satellite_token_groups (
    satellite_id TEXT NOT NULL REFERENCES satellite_tokens(id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES access_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (satellite_id, group_id)
);

CREATE TABLE output_device_groups (
    device_id TEXT NOT NULL REFERENCES output_devices(id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES access_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (device_id, group_id)
);

-- Preserve pre-migration behavior: existing outputs were visible to everyone.
INSERT INTO output_device_groups (device_id, group_id)
SELECT id, 'everyone' FROM output_devices;
