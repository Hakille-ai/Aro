DELETE FROM refresh_tokens
WHERE organization_id IS NULL;

ALTER TABLE refresh_tokens
  ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE conversations
  ADD CONSTRAINT conversations_org_id_unique UNIQUE (organization_id, id);

ALTER TABLE skill_groups
  ADD CONSTRAINT skill_groups_org_id_unique UNIQUE (organization_id, id);

ALTER TABLE scheduled_tasks
  ADD CONSTRAINT scheduled_tasks_org_id_unique UNIQUE (organization_id, id);

ALTER TABLE messages
  ADD CONSTRAINT messages_org_conversation_fk
  FOREIGN KEY (organization_id, conversation_id)
  REFERENCES conversations(organization_id, id)
  ON DELETE CASCADE;

ALTER TABLE memories
  ADD CONSTRAINT memories_org_source_conversation_fk
  FOREIGN KEY (organization_id, source_conversation_id)
  REFERENCES conversations(organization_id, id)
  ON DELETE NO ACTION;

ALTER TABLE skills
  ADD CONSTRAINT skills_org_group_fk
  FOREIGN KEY (organization_id, group_id)
  REFERENCES skill_groups(organization_id, id)
  ON DELETE NO ACTION;

ALTER TABLE usage_events
  ADD CONSTRAINT usage_events_org_conversation_fk
  FOREIGN KEY (organization_id, conversation_id)
  REFERENCES conversations(organization_id, id)
  ON DELETE NO ACTION;

ALTER TABLE task_runs
  ADD CONSTRAINT task_runs_org_scheduled_task_fk
  FOREIGN KEY (organization_id, scheduled_task_id)
  REFERENCES scheduled_tasks(organization_id, id)
  ON DELETE NO ACTION;

CREATE INDEX IF NOT EXISTS idx_personalities_org_updated
  ON personalities(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_system_prompts_org_updated
  ON system_prompts(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_voice_profiles_org_updated
  ON voice_profiles(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_skill_groups_org_updated
  ON skill_groups(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_skills_org_updated
  ON skills(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_plugin_connections_org_updated
  ON plugin_connections(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_mcp_servers_org_updated
  ON mcp_servers(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_hooks_org_updated
  ON hooks(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_scheduled_tasks_org_updated
  ON scheduled_tasks(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_teams_org_updated
  ON teams(organization_id, updated_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_memberships_org_status
  ON memberships(organization_id, status)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_api_keys_org_created
  ON api_keys(organization_id, created_at DESC)
  WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_audit_events_org_created
  ON audit_events(organization_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_task_runs_org_task_started
  ON task_runs(organization_id, scheduled_task_id, started_at DESC);
