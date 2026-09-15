CREATE TABLE agent_permission_profiles (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  trusted_roots TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  allowed_domains TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  allow_read BOOLEAN NOT NULL DEFAULT true,
  allow_write BOOLEAN NOT NULL DEFAULT false,
  allow_shell BOOLEAN NOT NULL DEFAULT false,
  allow_network BOOLEAN NOT NULL DEFAULT true,
  command_approval TEXT NOT NULL DEFAULT 'always',
  redact_secrets BOOLEAN NOT NULL DEFAULT true,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

CREATE TABLE agent_runs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  lane_id UUID,
  conversation_id UUID REFERENCES conversations(id) ON DELETE SET NULL,
  goal TEXT NOT NULL,
  mode TEXT NOT NULL,
  status TEXT NOT NULL,
  priority TEXT NOT NULL DEFAULT 'normal',
  model_provider_id TEXT,
  model_id TEXT,
  autonomy_profile_id UUID REFERENCES agent_permission_profiles(id) ON DELETE SET NULL,
  checkpoint_summary TEXT,
  last_error TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  heartbeat_at TIMESTAMPTZ,
  completed_at TIMESTAMPTZ,
  deleted_at TIMESTAMPTZ
);

CREATE TABLE agent_lanes (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  conversation_id UUID REFERENCES conversations(id) ON DELETE SET NULL,
  title TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active',
  priority TEXT NOT NULL DEFAULT 'normal',
  max_concurrent_runs INTEGER NOT NULL DEFAULT 1,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

ALTER TABLE agent_runs
  ADD CONSTRAINT agent_runs_lane_fk
  FOREIGN KEY (lane_id) REFERENCES agent_lanes(id) ON DELETE SET NULL;

CREATE TABLE agent_steps (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  run_id UUID NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  sequence INTEGER NOT NULL,
  kind TEXT NOT NULL,
  status TEXT NOT NULL,
  title TEXT NOT NULL,
  input JSONB NOT NULL DEFAULT '{}'::jsonb,
  output JSONB NOT NULL DEFAULT '{}'::jsonb,
  error TEXT,
  started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  finished_at TIMESTAMPTZ,
  UNIQUE (run_id, sequence)
);

CREATE TABLE agent_artifacts (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  run_id UUID NOT NULL REFERENCES agent_runs(id) ON DELETE CASCADE,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  uri TEXT,
  content TEXT,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE agent_context_items (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  run_id UUID REFERENCES agent_runs(id) ON DELETE CASCADE,
  conversation_id UUID REFERENCES conversations(id) ON DELETE SET NULL,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  uri TEXT,
  metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  search_document TSVECTOR GENERATED ALWAYS AS (
    to_tsvector('simple', coalesce(title, '') || ' ' || coalesce(content, ''))
  ) STORED
);

CREATE INDEX idx_agent_permission_profiles_org
  ON agent_permission_profiles(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE INDEX idx_agent_runs_org_updated
  ON agent_runs(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE INDEX idx_agent_runs_lane_status
  ON agent_runs(lane_id, status, priority)
  WHERE deleted_at IS NULL;

CREATE INDEX idx_agent_lanes_org_updated
  ON agent_lanes(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX idx_agent_lanes_conversation
  ON agent_lanes(organization_id, conversation_id)
  WHERE conversation_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX idx_agent_runs_status_heartbeat
  ON agent_runs(status, heartbeat_at)
  WHERE deleted_at IS NULL;

CREATE INDEX idx_agent_steps_run_sequence
  ON agent_steps(run_id, sequence);

CREATE INDEX idx_agent_artifacts_run_created
  ON agent_artifacts(run_id, created_at);

CREATE INDEX idx_agent_context_items_org_created
  ON agent_context_items(organization_id, created_at DESC);

CREATE INDEX idx_agent_context_items_search
  ON agent_context_items USING GIN(search_document);
