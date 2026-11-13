-- Krello schema v9: multiple reminders per card (JSON list) replacing the
-- single `reminder` column. `reminders_sent` keys "<idx>|<due>" prevent
-- duplicate fires and are cleared whenever due/reminders change.
ALTER TABLE cards ADD COLUMN reminders TEXT;
ALTER TABLE cards ADD COLUMN reminders_sent TEXT;

-- Port legacy single reminders: relative offsets stay relative, any other
-- non-empty value is treated as an absolute datetime.
UPDATE cards SET reminders = CASE
  WHEN reminder IN ('15m','30m','1h','2h','1d','2d','1w')
    THEN json_array(json_object('kind','relative','offset',reminder))
  ELSE json_array(json_object('kind','absolute','at',reminder))
END
WHERE reminder IS NOT NULL AND reminder != '';

-- Sweep partial index now targets the new column.
DROP INDEX IF EXISTS idx_cards_reminder_active;
CREATE INDEX IF NOT EXISTS idx_cards_reminders_active
  ON cards(due)
  WHERE reminders IS NOT NULL AND reminders != '[]' AND reminders != 'null'
    AND due_complete = 0 AND archived = 0 AND is_template = 0;
