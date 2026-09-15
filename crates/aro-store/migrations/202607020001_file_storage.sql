CREATE TABLE file_objects (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  client_id TEXT,
  original_name TEXT NOT NULL,
  mime_type TEXT NOT NULL,
  size_bytes BIGINT NOT NULL DEFAULT 0 CHECK (size_bytes >= 0),
  sha256 TEXT NOT NULL DEFAULT '',
  storage_backend TEXT NOT NULL,
  bucket TEXT,
  object_key TEXT NOT NULL,
  etag TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  scan_status TEXT NOT NULL DEFAULT 'pending',
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  CONSTRAINT file_objects_org_id_unique UNIQUE (organization_id, id)
);

CREATE UNIQUE INDEX idx_file_objects_storage_key_unique
  ON file_objects(storage_backend, COALESCE(bucket, ''), object_key);

CREATE UNIQUE INDEX idx_file_objects_org_client
  ON file_objects(organization_id, client_id)
  WHERE client_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX idx_file_objects_org_status_created
  ON file_objects(organization_id, status, created_at DESC)
  WHERE deleted_at IS NULL;

CREATE INDEX idx_file_objects_org_owner_created
  ON file_objects(organization_id, owner_user_id, created_at DESC)
  WHERE deleted_at IS NULL;

CREATE TABLE file_upload_sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  file_id UUID NOT NULL,
  expected_name TEXT NOT NULL,
  expected_mime_type TEXT NOT NULL,
  expected_size_bytes BIGINT NOT NULL CHECK (expected_size_bytes >= 0),
  expected_sha256 TEXT,
  status TEXT NOT NULL DEFAULT 'pending',
  expires_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at TIMESTAMPTZ,
  deleted_at TIMESTAMPTZ,
  CONSTRAINT file_upload_sessions_file_fk
    FOREIGN KEY (organization_id, file_id)
    REFERENCES file_objects(organization_id, id)
    ON DELETE CASCADE
);

CREATE INDEX idx_file_upload_sessions_org_status_expires
  ON file_upload_sessions(organization_id, status, expires_at)
  WHERE deleted_at IS NULL;

CREATE TABLE file_links (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  file_id UUID NOT NULL,
  conversation_id UUID,
  message_id UUID,
  run_id UUID,
  artifact_id UUID,
  link_kind TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  CONSTRAINT file_links_file_fk
    FOREIGN KEY (organization_id, file_id)
    REFERENCES file_objects(organization_id, id)
    ON DELETE CASCADE,
  CONSTRAINT file_links_conversation_fk
    FOREIGN KEY (organization_id, conversation_id)
    REFERENCES conversations(organization_id, id)
    ON DELETE CASCADE,
  CONSTRAINT file_links_message_fk
    FOREIGN KEY (message_id)
    REFERENCES messages(id)
    ON DELETE CASCADE
);

CREATE INDEX idx_file_links_org_message
  ON file_links(organization_id, message_id)
  WHERE deleted_at IS NULL;

CREATE INDEX idx_file_links_org_file
  ON file_links(organization_id, file_id, created_at DESC)
  WHERE deleted_at IS NULL;

CREATE TABLE file_chunks (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  file_id UUID NOT NULL,
  chunk_index INTEGER NOT NULL CHECK (chunk_index >= 0),
  content_text TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  char_start INTEGER NOT NULL DEFAULT 0 CHECK (char_start >= 0),
  char_end INTEGER NOT NULL DEFAULT 0 CHECK (char_end >= 0),
  token_estimate INTEGER,
  search_document TSVECTOR GENERATED ALWAYS AS (to_tsvector('simple', content_text)) STORED,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  CONSTRAINT file_chunks_file_fk
    FOREIGN KEY (organization_id, file_id)
    REFERENCES file_objects(organization_id, id)
    ON DELETE CASCADE,
  CONSTRAINT file_chunks_unique_index UNIQUE (organization_id, file_id, chunk_index)
);

CREATE INDEX idx_file_chunks_org_file
  ON file_chunks(organization_id, file_id, chunk_index)
  WHERE deleted_at IS NULL;

CREATE INDEX idx_file_chunks_search
  ON file_chunks USING GIN (search_document)
  WHERE deleted_at IS NULL;

CREATE TABLE file_index_jobs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  file_id UUID NOT NULL,
  status TEXT NOT NULL DEFAULT 'queued',
  attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  lease_until TIMESTAMPTZ,
  last_error TEXT,
  embedding_model TEXT,
  content_hash TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  CONSTRAINT file_index_jobs_file_fk
    FOREIGN KEY (organization_id, file_id)
    REFERENCES file_objects(organization_id, id)
    ON DELETE CASCADE
);

CREATE INDEX idx_file_index_jobs_org_status_created
  ON file_index_jobs(organization_id, status, created_at)
  WHERE deleted_at IS NULL;

CREATE TRIGGER trg_file_objects_updated_at
  BEFORE UPDATE ON file_objects
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

CREATE TRIGGER trg_file_index_jobs_updated_at
  BEFORE UPDATE ON file_index_jobs
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();
