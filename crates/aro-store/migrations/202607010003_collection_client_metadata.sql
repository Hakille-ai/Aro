ALTER TABLE memories ADD COLUMN client_id TEXT;
ALTER TABLE personalities ADD COLUMN client_id TEXT;
ALTER TABLE system_prompts ADD COLUMN client_id TEXT;
ALTER TABLE voice_profiles ADD COLUMN client_id TEXT;
ALTER TABLE skill_groups ADD COLUMN client_id TEXT;
ALTER TABLE skills ADD COLUMN client_id TEXT;
ALTER TABLE skills ADD COLUMN client_group_id TEXT;
ALTER TABLE plugin_connections ADD COLUMN client_id TEXT;
ALTER TABLE mcp_servers ADD COLUMN client_id TEXT;
ALTER TABLE hooks ADD COLUMN client_id TEXT;
ALTER TABLE scheduled_tasks ADD COLUMN client_id TEXT;

CREATE UNIQUE INDEX idx_memories_org_client_id
  ON memories(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE UNIQUE INDEX idx_personalities_org_client_id
  ON personalities(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE UNIQUE INDEX idx_system_prompts_org_client_id
  ON system_prompts(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE UNIQUE INDEX idx_voice_profiles_org_client_id
  ON voice_profiles(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE UNIQUE INDEX idx_skill_groups_org_client_id
  ON skill_groups(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE UNIQUE INDEX idx_skills_org_client_id
  ON skills(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE UNIQUE INDEX idx_plugin_connections_org_client_id
  ON plugin_connections(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE UNIQUE INDEX idx_mcp_servers_org_client_id
  ON mcp_servers(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE UNIQUE INDEX idx_hooks_org_client_id
  ON hooks(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;

CREATE UNIQUE INDEX idx_scheduled_tasks_org_client_id
  ON scheduled_tasks(organization_id, client_id)
  WHERE deleted_at IS NULL AND client_id IS NOT NULL;
