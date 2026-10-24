-- Chats (docs/product-scope.md, F23): conversations in a project. Events and attachments belong
-- to a chat. An event of no chat keeps a null chat_id.

CREATE TABLE chats (
    id TEXT PRIMARY KEY,
    project TEXT NOT NULL,
    title TEXT NOT NULL,
    title_set INTEGER NOT NULL DEFAULT 0,
    pinned INTEGER NOT NULL DEFAULT 0,
    archived_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

ALTER TABLE events ADD COLUMN chat_id TEXT;
CREATE INDEX events_chat ON events (chat_id, id);

CREATE TABLE attachments (
    id TEXT PRIMARY KEY,
    chat_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    path TEXT,
    text TEXT,
    size INTEGER NOT NULL DEFAULT 0,
    sent INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE INDEX attachments_chat ON attachments (chat_id, sent);
