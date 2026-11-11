-- Krello schema v5: time tracking.
-- time_spent_minutes accumulates tracked work; tracking_started_at marks a
-- running session (NULL = stopped).
ALTER TABLE cards ADD COLUMN time_spent_minutes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE cards ADD COLUMN tracking_started_at TEXT;
