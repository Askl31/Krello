-- Krello schema v13: manual ordering position for boards (drag & drop in the
-- sidebar). Backfill: order of appearance (created_at, then id) — only visible
-- boards matter for the display, all rows get a stable value.
ALTER TABLE boards ADD COLUMN position REAL;
UPDATE boards SET position = (
  SELECT COUNT(*) FROM boards b2
  WHERE b2.created_at < boards.created_at
     OR (b2.created_at = boards.created_at AND b2.id < boards.id)
);
CREATE INDEX IF NOT EXISTS idx_boards_order ON boards(archived, is_board_template, pinned, position);
