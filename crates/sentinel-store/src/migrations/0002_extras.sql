-- Columns the second batch of store methods need. Part 1 created the tables;
-- these additions let approvals, decisions, checkpoints, barriers, verifications,
-- and tools carry every field their methods read and write.

ALTER TABLE tools ADD COLUMN enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE verifications ADD COLUMN reason TEXT NOT NULL DEFAULT '';
ALTER TABLE verifications ADD COLUMN receipt_json TEXT NOT NULL DEFAULT 'null';
ALTER TABLE approvals ADD COLUMN created_at TEXT NOT NULL DEFAULT '';
ALTER TABLE decisions ADD COLUMN tasks_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE decisions ADD COLUMN state TEXT NOT NULL DEFAULT 'pending';
ALTER TABLE barriers ADD COLUMN ended_at TEXT;
ALTER TABLE barriers ADD COLUMN passed INTEGER;
