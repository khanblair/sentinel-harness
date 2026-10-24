-- Skills (F24) keep only their switch here. The text lives in the SKILL.md files.
CREATE TABLE skills (
    name TEXT PRIMARY KEY,
    mode TEXT NOT NULL DEFAULT 'auto'
);

-- Routines (F25): jobs at a set local time, and the briefs they built.
CREATE TABLE routines (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    minute_of_day INTEGER NOT NULL,
    days INTEGER NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 0,
    to_telegram INTEGER NOT NULL DEFAULT 0,
    text TEXT NOT NULL DEFAULT '',
    backlog TEXT NOT NULL DEFAULT '',
    last_fired_on TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE briefs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    routine_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    created_at TEXT NOT NULL,
    sent_to_telegram INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX briefs_recent ON briefs (id DESC);
