-- Ichoi initial schema (SQLite). Pure DDL; idempotent data backfills are "transforms"
-- run separately at boot (see db::transforms), never here.

CREATE TABLE accounts (
    id           TEXT PRIMARY KEY NOT NULL,      -- uuid@domain.tld (§8)
    handle       TEXT NOT NULL,
    display_name TEXT,
    role         TEXT NOT NULL DEFAULT 'member', -- admin | member | guest (§6.6)
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE sessions (
    token_sha256 TEXT PRIMARY KEY NOT NULL,      -- we store only the hash (§7.2)
    account_id   TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at   TEXT NOT NULL
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE trusted_domains (
    domain TEXT PRIMARY KEY NOT NULL
);

CREATE TABLE libraries (
    id   TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,                          -- music | audiobook
    path TEXT NOT NULL
);

CREATE TABLE artists (
    id   TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE albums (
    id             TEXT PRIMARY KEY NOT NULL,
    title          TEXT NOT NULL,
    artist_id      TEXT REFERENCES artists(id),
    year           INTEGER,
    has_cover_art  INTEGER NOT NULL DEFAULT 0,
    cover_art_path TEXT
);

CREATE TABLE tracks (
    id                 TEXT PRIMARY KEY NOT NULL,
    library_id         TEXT NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    root_relative_path TEXT NOT NULL,            -- portability (§7)
    title              TEXT NOT NULL,
    artist_id          TEXT REFERENCES artists(id),
    album_id           TEXT REFERENCES albums(id),
    track_no           INTEGER,
    disc_no            INTEGER,
    duration_ms        BIGINT  NOT NULL DEFAULT 0,
    codec              TEXT NOT NULL,
    bitrate_kbps       INTEGER,
    sample_rate        INTEGER NOT NULL DEFAULT 0,
    channels           INTEGER NOT NULL DEFAULT 0,
    bit_depth          INTEGER,
    size_bytes         BIGINT  NOT NULL DEFAULT 0,
    mtime              TEXT NOT NULL,
    content_hash       TEXT,                     -- cross-server dedupe (§7)
    trim_start_samples BIGINT  NOT NULL DEFAULT 0, -- gapless (§5.4)
    trim_end_samples   BIGINT  NOT NULL DEFAULT 0
);
CREATE UNIQUE INDEX idx_tracks_lib_path ON tracks(library_id, root_relative_path);
CREATE INDEX idx_tracks_album  ON tracks(album_id);
CREATE INDEX idx_tracks_artist ON tracks(artist_id);

CREATE TABLE playlists (
    id                 TEXT PRIMARY KEY NOT NULL,
    name               TEXT NOT NULL,
    owner              TEXT REFERENCES accounts(id),
    root_relative_path TEXT NOT NULL             -- the m3u path from the collection root (§7)
);

CREATE TABLE nodes (
    id            TEXT PRIMARY KEY NOT NULL,
    kind          TEXT NOT NULL,                 -- core | satellite | client
    hostname      TEXT NOT NULL,
    friendly_name TEXT NOT NULL,
    token_sha256  TEXT,                          -- satellite node token hash (§6.7)
    platform      TEXT NOT NULL DEFAULT '',
    arch          TEXT NOT NULL DEFAULT '',
    audio_outputs TEXT NOT NULL DEFAULT 'none',  -- none | some (§6.1)
    last_seen     TEXT
);

CREATE TABLE output_devices (
    id           TEXT PRIMARY KEY NOT NULL,
    node_id      TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    os_device_id TEXT NOT NULL,
    friendly_name TEXT NOT NULL,
    is_default   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE players (
    id               TEXT PRIMARY KEY NOT NULL,
    kind             TEXT NOT NULL,              -- shared | private
    output_device_id TEXT REFERENCES output_devices(id) ON DELETE CASCADE,
    owner_account_id TEXT REFERENCES accounts(id) ON DELETE CASCADE,
    name             TEXT NOT NULL,
    name_suffix      TEXT                        -- client satellite mode (§6.4)
);

CREATE TABLE player_queue_items (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    player_id TEXT NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    track_id  TEXT NOT NULL,
    position  INTEGER NOT NULL
);
CREATE INDEX idx_pqi_player ON player_queue_items(player_id, position);

CREATE TABLE player_state (
    player_id     TEXT PRIMARY KEY NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    status        TEXT NOT NULL DEFAULT 'stopped', -- stopped | playing | paused
    current_index INTEGER,
    position_ms   BIGINT,
    volume        INTEGER NOT NULL DEFAULT 100
);

CREATE TABLE listens (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id TEXT NOT NULL,
    track_id   TEXT NOT NULL,
    played_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE stars (
    account_id TEXT NOT NULL,
    item_id    TEXT NOT NULL,
    item_type  TEXT NOT NULL,
    PRIMARY KEY (account_id, item_id, item_type)
);
