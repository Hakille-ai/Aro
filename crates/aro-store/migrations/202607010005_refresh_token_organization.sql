ALTER TABLE refresh_tokens
  ADD COLUMN organization_id UUID REFERENCES organizations(id) ON DELETE CASCADE;

CREATE INDEX idx_refresh_tokens_user_org
  ON refresh_tokens (user_id, organization_id)
  WHERE revoked_at IS NULL;
