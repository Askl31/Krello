-- Krello schema v4: effective reminders.
-- reminder_sent_at tracks the last fired reminder for a card (NULL = armed).
ALTER TABLE cards ADD COLUMN reminder_sent_at TEXT;
