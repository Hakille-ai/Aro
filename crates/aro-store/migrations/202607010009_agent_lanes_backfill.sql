ALTER TABLE agent_runs
  ADD COLUMN IF NOT EXISTS lane_id UUID;

ALTER TABLE agent_runs
  ADD COLUMN IF NOT EXISTS priority TEXT NOT NULL DEFAULT 'normal';

CREATE TABLE IF NOT EXISTS agent_lanes (
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

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'agent_runs_lane_fk'
      AND conrelid = 'agent_runs'::regclass
  ) THEN
    ALTER TABLE agent_runs
      ADD CONSTRAINT agent_runs_lane_fk
      FOREIGN KEY (lane_id) REFERENCES agent_lanes(id) ON DELETE SET NULL;
  END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_agent_runs_lane_status
  ON agent_runs(lane_id, status, priority)
  WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_agent_lanes_org_updated
  ON agent_lanes(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_lanes_conversation
  ON agent_lanes(organization_id, conversation_id)
  WHERE conversation_id IS NOT NULL AND deleted_at IS NULL;
