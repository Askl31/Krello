-- Krello schema v10: full-text search over comment bodies (external content).
-- The FTS column MUST share its name with the source column (`body`).
CREATE VIRTUAL TABLE IF NOT EXISTS comments_fts USING fts5(
    body,
    content='comments',
    content_rowid='rowid'
);
CREATE TRIGGER IF NOT EXISTS comments_fts_ai AFTER INSERT ON comments BEGIN
    INSERT INTO comments_fts(rowid, body) VALUES (new.rowid, new.body);
END;
CREATE TRIGGER IF NOT EXISTS comments_fts_ad AFTER DELETE ON comments BEGIN
    INSERT INTO comments_fts(comments_fts, rowid, body) VALUES ('delete', old.rowid, old.body);
END;
CREATE TRIGGER IF NOT EXISTS comments_fts_au AFTER UPDATE OF body ON comments BEGIN
    INSERT INTO comments_fts(comments_fts, rowid, body) VALUES ('delete', old.rowid, old.body);
    INSERT INTO comments_fts(rowid, body) VALUES (new.rowid, new.body);
END;
-- Index the comments that already exist.
INSERT INTO comments_fts(comments_fts) VALUES('rebuild');
