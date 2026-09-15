CREATE TABLE teams (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  client_id TEXT,
  name TEXT NOT NULL,
  description TEXT,
  member_ids TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

CREATE UNIQUE INDEX idx_teams_org_client_id
  ON teams(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE INDEX idx_teams_org_updated
  ON teams(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
