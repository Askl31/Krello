-- Krello schema v12: hierarchical sub-cards. A sub-card lives in the SAME
-- list as its parent; deleting a parent detaches (never deletes) children.
ALTER TABLE cards ADD COLUMN parent_id TEXT REFERENCES cards(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_cards_parent ON cards(parent_id);
