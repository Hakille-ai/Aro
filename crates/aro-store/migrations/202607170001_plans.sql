CREATE TABLE plans (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  conversation_id UUID NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  description TEXT,
  tasks JSONB NOT NULL DEFAULT '[]'::JSONB,
  status TEXT NOT NULL DEFAULT 'active',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  deleted_at TIMESTAMPTZ
);

CREATE TRIGGER trg_plans_updated_at
  BEFORE UPDATE ON plans
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();
