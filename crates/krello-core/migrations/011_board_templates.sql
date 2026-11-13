-- Krello schema v11: saved board templates ("Enregistrer comme modèle").
-- Template boards are hidden from the boards list / sidebar and feed the
-- "Nouveau board" gallery; instantiation deep-copies via duplicate_board.
ALTER TABLE boards ADD COLUMN is_board_template INTEGER NOT NULL DEFAULT 0;
ALTER TABLE boards ADD COLUMN template_description TEXT;
