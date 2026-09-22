-- Persist agent execution traces alongside chat messages.
-- Cloud sync of local results (`POST /v1/assistant/local-result`) carries the
-- full assistant message including `steps`; without this column the trace was
-- silently dropped and the sync payload served no purpose once large.
-- NULL = no trace (all historical rows, plain chat messages).
ALTER TABLE messages ADD COLUMN steps JSONB;
