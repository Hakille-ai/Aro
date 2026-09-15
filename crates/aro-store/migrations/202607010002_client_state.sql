CREATE TABLE client_state (
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value JSONB,
  encrypted_value BYTEA,
  sensitive BOOLEAN NOT NULL DEFAULT false,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ,
  CONSTRAINT client_state_value_check CHECK (
    (sensitive = false AND value IS NOT NULL AND encrypted_value IS NULL)
    OR
    (sensitive = true AND value IS NULL AND encrypted_value IS NOT NULL)
  ),
  PRIMARY KEY (organization_id, user_id, key)
);

CREATE INDEX idx_client_state_org_user
  ON client_state(organization_id, user_id)
  WHERE deleted_at IS NULL;
