-- Krello schema v1
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS calendars (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    weekends      TEXT NOT NULL DEFAULT '[6,0]',  -- 0=Sunday .. 6=Saturday
    holidays      TEXT NOT NULL DEFAULT '[]',     -- ["YYYY-MM-DD", ...]
    hours_per_day REAL NOT NULL DEFAULT 8.0,
    created_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS boards (
    id          TEXT PRIMARY KEY,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    background  TEXT NOT NULL DEFAULT '{"type":"color","value":"#101828"}',
    calendar_id TEXT REFERENCES calendars(id) ON DELETE SET NULL,
    start_date  TEXT,                 -- YYYY-MM-DD, project start used by scheduler
    deadline    TEXT,                 -- YYYY-MM-DD optional project deadline
    settings    TEXT NOT NULL DEFAULT '{}',
    archived    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS lists (
    id         TEXT PRIMARY KEY,
    board_id   TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    position   REAL NOT NULL,
    wip_limit  INTEGER,
    archived   INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_lists_board ON lists(board_id, archived, position);

CREATE TABLE IF NOT EXISTS labels (
    id         TEXT PRIMARY KEY,
    board_id   TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name       TEXT NOT NULL DEFAULT '',
    color      TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_labels_board ON labels(board_id);

CREATE TABLE IF NOT EXISTS cards (
    id             TEXT PRIMARY KEY,
    board_id       TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    list_id        TEXT NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    title          TEXT NOT NULL,
    description    TEXT NOT NULL DEFAULT '',
    position       REAL NOT NULL,
    archived       INTEGER NOT NULL DEFAULT 0,
    cover          TEXT,                                   -- JSON {"type":"color"|"attachment","value":...}
    start          TEXT,                                   -- date or datetime (ISO)
    due            TEXT,
    due_complete   INTEGER NOT NULL DEFAULT 0,
    reminder       TEXT,                                   -- "15m"|"1h"|"1d"|"2d" or ISO datetime
    recurrence     TEXT,                                   -- JSON {"freq":"daily|weekly|monthly","interval":n}
    priority       TEXT NOT NULL DEFAULT 'none',           -- none|low|medium|high|urgent
    is_milestone   INTEGER NOT NULL DEFAULT 0,
    duration_days  REAL,                                   -- working days; NULL = derived from dates
    work_hours     REAL,
    pct_complete   INTEGER NOT NULL DEFAULT 0,            -- 0..100
    task_type      TEXT NOT NULL DEFAULT 'fixed_duration', -- fixed_duration|fixed_work|fixed_units
    units          REAL NOT NULL DEFAULT 1.0,
    hours_per_day  REAL NOT NULL DEFAULT 8.0,
    constraint_type TEXT NOT NULL DEFAULT 'asap',          -- asap|deadline|start_not_before
    constraint_date TEXT,
    wbs            TEXT NOT NULL DEFAULT '',
    cost           REAL NOT NULL DEFAULT 0.0,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    completed_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_cards_board ON cards(board_id, archived);
CREATE INDEX IF NOT EXISTS idx_cards_list  ON cards(list_id, archived, position);
CREATE INDEX IF NOT EXISTS idx_cards_due   ON cards(board_id, due);
CREATE INDEX IF NOT EXISTS idx_cards_start ON cards(board_id, start);

CREATE TABLE IF NOT EXISTS card_labels (
    card_id  TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    PRIMARY KEY (card_id, label_id)
);
CREATE INDEX IF NOT EXISTS idx_card_labels_label ON card_labels(label_id);

CREATE TABLE IF NOT EXISTS checklists (
    id         TEXT PRIMARY KEY,
    card_id    TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    title      TEXT NOT NULL,
    position   REAL NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_checklists_card ON checklists(card_id, position);

CREATE TABLE IF NOT EXISTS checklist_items (
    id           TEXT PRIMARY KEY,
    checklist_id TEXT NOT NULL REFERENCES checklists(id) ON DELETE CASCADE,
    title        TEXT NOT NULL,
    checked      INTEGER NOT NULL DEFAULT 0,
    position     REAL NOT NULL,
    due          TEXT,
    created_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_cl_items ON checklist_items(checklist_id, position);

CREATE TABLE IF NOT EXISTS custom_field_defs (
    id         TEXT PRIMARY KEY,
    board_id   TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    type       TEXT NOT NULL,            -- text|number|date|checkbox|select
    options    TEXT NOT NULL DEFAULT '[]', -- for select
    position   REAL NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_cfd_board ON custom_field_defs(board_id, position);

CREATE TABLE IF NOT EXISTS custom_field_values (
    card_id  TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    field_id TEXT NOT NULL REFERENCES custom_field_defs(id) ON DELETE CASCADE,
    value    TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (card_id, field_id)
);

CREATE TABLE IF NOT EXISTS attachments (
    id          TEXT PRIMARY KEY,
    card_id     TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    filename    TEXT NOT NULL,
    stored_path TEXT NOT NULL,
    mime        TEXT NOT NULL DEFAULT 'application/octet-stream',
    size        INTEGER NOT NULL DEFAULT 0,
    is_cover    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_att_card ON attachments(card_id);

CREATE TABLE IF NOT EXISTS comments (
    id         TEXT PRIMARY KEY,
    card_id    TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    body       TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_comments_card ON comments(card_id, created_at);

CREATE TABLE IF NOT EXISTS activity (
    id         TEXT PRIMARY KEY,
    board_id   TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    card_id    TEXT REFERENCES cards(id) ON DELETE SET NULL,
    kind       TEXT NOT NULL,
    payload    TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_activity_board ON activity(board_id, created_at DESC);

CREATE TABLE IF NOT EXISTS dependencies (
    id             TEXT PRIMARY KEY,
    board_id       TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    predecessor_id TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    successor_id   TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    type           TEXT NOT NULL DEFAULT 'FS',  -- FS|SS|FF|SF
    lag_days       REAL NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL,
    UNIQUE (predecessor_id, successor_id, type)
);
CREATE INDEX IF NOT EXISTS idx_deps_board ON dependencies(board_id);
CREATE INDEX IF NOT EXISTS idx_deps_succ  ON dependencies(successor_id);
CREATE INDEX IF NOT EXISTS idx_deps_pred  ON dependencies(predecessor_id);

CREATE TABLE IF NOT EXISTS schedule_cache (
    card_id            TEXT PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
    es                 TEXT,
    ef                 TEXT,
    ls                 TEXT,
    lf                 TEXT,
    total_float_days   REAL,
    is_critical        INTEGER NOT NULL DEFAULT 0,
    computed_at        TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS baselines (
    id         TEXT PRIMARY KEY,
    board_id   TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS baseline_snapshots (
    baseline_id   TEXT NOT NULL REFERENCES baselines(id) ON DELETE CASCADE,
    card_id       TEXT NOT NULL REFERENCES cards(id) ON DELETE CASCADE,
    start         TEXT,
    due           TEXT,
    duration_days REAL,
    work_hours    REAL,
    PRIMARY KEY (baseline_id, card_id)
);

CREATE TABLE IF NOT EXISTS automations (
    id         TEXT PRIMARY KEY,
    board_id   TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    enabled    INTEGER NOT NULL DEFAULT 1,
    trigger    TEXT NOT NULL,          -- card_created|card_moved|card_labeled|card_due_soon|button|scheduled_daily|scheduled_weekly
    conditions TEXT NOT NULL DEFAULT '{}',
    actions    TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_automations_board ON automations(board_id, enabled);

CREATE TABLE IF NOT EXISTS board_views (
    board_id TEXT NOT NULL REFERENCES boards(id) ON DELETE CASCADE,
    view     TEXT NOT NULL,            -- board|calendar|timeline|table|dashboard
    settings TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (board_id, view)
);

-- Full-text search over cards (external content on cards.rowid)
CREATE VIRTUAL TABLE IF NOT EXISTS cards_fts USING fts5(
    title, description,
    content='cards', content_rowid='rowid'
);
CREATE TRIGGER IF NOT EXISTS cards_fts_ai AFTER INSERT ON cards BEGIN
    INSERT INTO cards_fts(rowid, title, description) VALUES (new.rowid, new.title, new.description);
END;
CREATE TRIGGER IF NOT EXISTS cards_fts_ad AFTER DELETE ON cards BEGIN
    INSERT INTO cards_fts(cards_fts, rowid, title, description) VALUES ('delete', old.rowid, old.title, old.description);
END;
CREATE TRIGGER IF NOT EXISTS cards_fts_au AFTER UPDATE OF title, description ON cards BEGIN
    INSERT INTO cards_fts(cards_fts, rowid, title, description) VALUES ('delete', old.rowid, old.title, old.description);
    INSERT INTO cards_fts(rowid, title, description) VALUES (new.rowid, new.title, new.description);
END;
