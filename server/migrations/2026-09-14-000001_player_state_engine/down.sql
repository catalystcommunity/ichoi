DROP INDEX IF EXISTS idx_player_undo_queue_position;
DROP TABLE IF EXISTS player_undo_queue_items;
DROP TABLE IF EXISTS player_undo_state;

ALTER TABLE player_state DROP COLUMN listener_account_id;
ALTER TABLE player_state DROP COLUMN error;
ALTER TABLE player_state DROP COLUMN current_queue_item_id;
ALTER TABLE player_state DROP COLUMN playback_id;
ALTER TABLE player_state DROP COLUMN revision;
ALTER TABLE player_state DROP COLUMN shuffle;
ALTER TABLE player_state DROP COLUMN repeat_mode;
