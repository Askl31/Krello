-- v3: automation run log (bot audit trail + per-day dedupe for scheduled triggers).
CREATE TABLE IF NOT EXISTS automation_runs (
    id            TEXT PRIMARY KEY,
    automation_id TEXT NOT NULL,
    board_id      TEXT NOT NULL,               -- board owning the rule
    trigger       TEXT NOT NULL,
    card_id       TEXT,                        -- NULL for board-level runs
    ok            INTEGER NOT NULL DEFAULT 1,
    detail        TEXT NOT NULL DEFAULT '{}',  -- JSON: actions applied / error
    created_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_automation_runs_auto ON automation_runs(automation_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_automation_runs_board ON automation_runs(board_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_automation_runs_card ON automation_runs(card_id, created_at DESC);
