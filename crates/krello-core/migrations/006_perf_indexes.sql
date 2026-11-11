-- 006_perf_indexes.sql — additive perf indexes only (no schema change).
-- (1) Per-card activity feed: boards.rs card_activity filters board_id + card_id.
CREATE INDEX IF NOT EXISTS idx_activity_board_card ON activity(board_id, card_id, created_at DESC);
-- (2) Reminder sweep (automations.rs fire_due_reminders): partial index on due
--     for cards that actually carry an active reminder.
CREATE INDEX IF NOT EXISTS idx_cards_reminder_active
  ON cards(due)
  WHERE reminder IS NOT NULL AND reminder != '' AND due_complete = 0 AND archived = 0 AND is_template = 0;
