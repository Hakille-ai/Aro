CREATE TABLE organization_invitations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  membership_id UUID NOT NULL REFERENCES memberships(id) ON DELETE CASCADE,
  invited_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  invited_by_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
  token_hash TEXT NOT NULL UNIQUE,
  created_user BOOLEAN NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  accepted_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT organization_invitations_membership_unique UNIQUE (membership_id)
);

CREATE INDEX idx_organization_invitations_token_active
  ON organization_invitations(token_hash, expires_at)
  WHERE accepted_at IS NULL AND revoked_at IS NULL;

CREATE INDEX idx_organization_invitations_org_active
  ON organization_invitations(organization_id, expires_at)
  WHERE accepted_at IS NULL AND revoked_at IS NULL;

CREATE TRIGGER trg_organization_invitations_updated_at
  BEFORE UPDATE ON organization_invitations
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();
