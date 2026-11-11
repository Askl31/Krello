-- Krello schema v2: card templates.
-- A template is an ordinary card flagged is_template=1 (Trello model: no
-- dedicated table). It stays in its list, is edited like any card, and acts
-- as a deep-copy source for instantiation. Dates, completion status,
-- comments and activity are never copied to instances.
ALTER TABLE cards ADD COLUMN is_template INTEGER NOT NULL DEFAULT 0;
ALTER TABLE cards ADD COLUMN template_description TEXT; -- optional gallery help text
CREATE INDEX IF NOT EXISTS idx_cards_is_template ON cards(board_id, is_template);
