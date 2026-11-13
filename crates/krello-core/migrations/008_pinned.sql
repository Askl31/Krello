-- Krello schema v8: pinned (favorite) boards, shown first in the sidebar.
ALTER TABLE boards ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
ALTER TABLE boards ADD COLUMN pinned_at TEXT;
