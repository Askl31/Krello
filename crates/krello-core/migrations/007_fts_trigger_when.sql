-- 007_fts_trigger_when.sql — same FTS content, but the UPDATE trigger only fires
-- when title/description actually changed. Before, every card UPDATE (move,
-- archive, priority, tracking…) re-wrote the FTS index for no reason.
DROP TRIGGER IF EXISTS cards_fts_au;
CREATE TRIGGER cards_fts_au AFTER UPDATE OF title, description ON cards
WHEN new.title IS NOT old.title OR new.description IS NOT old.description
BEGIN
    INSERT INTO cards_fts(cards_fts, rowid, title, description) VALUES ('delete', old.rowid, old.title, old.description);
    INSERT INTO cards_fts(rowid, title, description) VALUES (new.rowid, new.title, new.description);
END;
