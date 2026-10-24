-- Initial schema. Column names follow the Data Model table in docs/architecture.md.

CREATE TABLE tasks (
    ordinal INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL,
    tier TEXT NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    scope_json TEXT NOT NULL,
    flags TEXT NOT NULL
);

CREATE TABLE task_deps (
    task_id TEXT NOT NULL,
    depends_on TEXT NOT NULL,
    kind TEXT NOT NULL,
    PRIMARY KEY (task_id, depends_on)
);

CREATE INDEX task_deps_task_id ON task_deps (task_id);

CREATE TABLE reservations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL,
    path TEXT NOT NULL,
    state TEXT NOT NULL,
    reserved_at TEXT NOT NULL,
    released_at TEXT
);

CREATE INDEX reservations_path_state ON reservations (path, state);

CREATE TABLE runs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    tool_id TEXT NOT NULL,
    model TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    "exit" INTEGER,
    cost_micro_usd INTEGER NOT NULL,
    cost_estimated INTEGER NOT NULL
);

CREATE TABLE checkpoints (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    commit_sha TEXT NOT NULL,
    paths_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE barriers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    wave INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    suite_result TEXT,
    output_ref TEXT
);

CREATE TABLE verifications (
    task_id TEXT PRIMARY KEY,
    tests_pass INTEGER NOT NULL,
    new_test INTEGER NOT NULL,
    reviewer_ok INTEGER NOT NULL,
    tamper_clear INTEGER NOT NULL,
    verdict TEXT NOT NULL
);

CREATE TABLE tools (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    path TEXT,
    version TEXT,
    adapter TEXT,
    status TEXT NOT NULL,
    tier TEXT NOT NULL
);

CREATE TABLE devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    revoked_at TEXT
);

CREATE TABLE decisions (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    prompt TEXT NOT NULL,
    answer TEXT,
    actor TEXT NOT NULL,
    at TEXT NOT NULL
);

CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts TEXT NOT NULL,
    kind TEXT NOT NULL,
    task_id TEXT,
    payload_json TEXT NOT NULL
);

CREATE INDEX events_task_id ON events (task_id);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE telegram_chats (
    id TEXT PRIMARY KEY,
    telegram_user_id INTEGER NOT NULL,
    notify INTEGER NOT NULL DEFAULT 0,
    control INTEGER NOT NULL DEFAULT 0,
    code INTEGER NOT NULL DEFAULT 0,
    reports INTEGER NOT NULL DEFAULT 0,
    detail TEXT NOT NULL,
    events_json TEXT NOT NULL,
    quiet_hours TEXT,
    connected_at TEXT NOT NULL,
    revoked_at TEXT
);

CREATE TABLE telegram_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    state TEXT NOT NULL,
    tries INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE quality_notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    location TEXT,
    detail_code TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE permissions (
    agent_id TEXT NOT NULL,
    capability TEXT NOT NULL,
    allowed INTEGER NOT NULL,
    PRIMARY KEY (agent_id, capability)
);

CREATE TABLE approvals (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    task_id TEXT,
    agent_id TEXT,
    path TEXT,
    state TEXT NOT NULL,
    decided_by TEXT,
    decided_at TEXT
);
