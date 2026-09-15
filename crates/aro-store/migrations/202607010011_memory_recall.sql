ALTER TABLE memories
  ADD COLUMN IF NOT EXISTS scope TEXT NOT NULL DEFAULT 'user';

ALTER TABLE memories
  ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'approved';

ALTER TABLE memories
  ADD COLUMN IF NOT EXISTS source_message_ids UUID[] NOT NULL DEFAULT ARRAY[]::UUID[];

ALTER TABLE memories
  ADD COLUMN IF NOT EXISTS salience REAL NOT NULL DEFAULT 0.5;

ALTER TABLE memories
  ADD COLUMN IF NOT EXISTS last_used_at TIMESTAMPTZ;

ALTER TABLE memories
  ADD COLUMN IF NOT EXISTS search_document TSVECTOR GENERATED ALWAYS AS (
    to_tsvector('simple', coalesce(content, '') || ' ' || coalesce(category, '') || ' ' || coalesce(scope, ''))
  ) STORED;

DROP INDEX IF EXISTS idx_memories_org_client_id;

CREATE UNIQUE INDEX IF NOT EXISTS idx_memories_org_owner_client_id
  ON memories(organization_id, owner_user_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_memories_org_owner_updated
  ON memories(organization_id, owner_user_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_memories_org_owner_status_updated
  ON memories(organization_id, owner_user_id, status, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_memories_search
  ON memories USING GIN(search_document);
