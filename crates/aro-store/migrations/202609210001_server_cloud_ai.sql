-- Sovereign AI: server-side cloud generation is default-DENY.
-- Nothing leaves the self-hosted server unless the organization admin
-- explicitly opts in AND stores an encrypted provider key.
-- Keys are pgp_sym_encrypt'ed with the server secrets key (same mechanism
-- as client-state and invitation tokens) and are never readable in clear
-- by any API except the generation path itself.
CREATE TABLE org_ai_cloud_consent (
  organization_id UUID PRIMARY KEY REFERENCES organizations(id) ON DELETE CASCADE,
  enabled BOOLEAN NOT NULL DEFAULT FALSE,
  provider_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
  data_residency TEXT,
  accepted_by UUID REFERENCES users(id) ON DELETE SET NULL,
  accepted_at TIMESTAMPTZ,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE org_provider_keys (
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  provider_id TEXT NOT NULL CHECK (provider_id ~ '^[a-z0-9-]{1,64}$'),
  enc_key BYTEA NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (organization_id, provider_id)
);
