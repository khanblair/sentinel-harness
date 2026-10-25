-- The agent a chat uses for its messages. NULL means auto: each task picks its agent by difficulty.
ALTER TABLE chats ADD COLUMN agent TEXT;
