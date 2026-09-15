CREATE TABLE custom_agent_definitions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  description TEXT,
  system_prompt TEXT,
  model_provider_id TEXT,
  model_id TEXT,
  autonomy_profile_id UUID REFERENCES agent_permission_profiles(id) ON DELETE SET NULL,
  enabled_tools TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

CREATE TRIGGER trg_custom_agent_definitions_updated_at
  BEFORE UPDATE ON custom_agent_definitions
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

CREATE TABLE custom_model_definitions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  provider_kind TEXT NOT NULL,
  provider_id TEXT NOT NULL,
  model_id TEXT NOT NULL,
  label TEXT NOT NULL,
  endpoint TEXT NOT NULL,
  api_key_vault_key TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

CREATE TRIGGER trg_custom_model_definitions_updated_at
  BEFORE UPDATE ON custom_model_definitions
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

ALTER TABLE agent_run_jobs ADD COLUMN parent_job_id UUID;
