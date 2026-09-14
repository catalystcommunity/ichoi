ALTER TABLE player_state ADD COLUMN repeat_mode TEXT NOT NULL DEFAULT 'off';
ALTER TABLE player_state ADD COLUMN shuffle INTEGER NOT NULL DEFAULT 0;
ALTER TABLE player_state ADD COLUMN revision BIGINT NOT NULL DEFAULT 0;
ALTER TABLE player_state ADD COLUMN playback_id TEXT;
ALTER TABLE player_state ADD COLUMN current_queue_item_id INTEGER;
ALTER TABLE player_state ADD COLUMN error TEXT;
ALTER TABLE player_state ADD COLUMN listener_account_id TEXT;

-- Keep the current item stable when queue positions change after this migration.
UPDATE player_state
SET current_queue_item_id = (
    SELECT item.id
    FROM player_queue_items AS item
    WHERE item.player_id = player_state.player_id
      AND item.position = player_state.current_index
)
WHERE current_index IS NOT NULL;

-- A playing row needs an event token so a reconnect can resume it safely.
UPDATE player_state
SET playback_id = lower(hex(randomblob(16))),
    revision = 1
WHERE status = 'playing'
  AND current_queue_item_id IS NOT NULL;

CREATE TABLE player_undo_state (
    player_id             TEXT PRIMARY KEY NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    status                TEXT NOT NULL,
    current_queue_item_id INTEGER,
    position_ms           BIGINT,
    playback_id           TEXT,
    error                 TEXT,
    listener_account_id   TEXT
);

CREATE TABLE player_undo_queue_items (
    player_id     TEXT NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    queue_item_id INTEGER NOT NULL,
    track_id      TEXT NOT NULL,
    position      INTEGER NOT NULL,
    PRIMARY KEY (player_id, queue_item_id)
);

CREATE INDEX idx_player_undo_queue_position
    ON player_undo_queue_items(player_id, position);
