CREATE OR REPLACE FUNCTION aro_touch_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = now();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_users_updated_at ON users;
CREATE TRIGGER trg_users_updated_at
  BEFORE UPDATE ON users
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_organizations_updated_at ON organizations;
CREATE TRIGGER trg_organizations_updated_at
  BEFORE UPDATE ON organizations
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_memberships_updated_at ON memberships;
CREATE TRIGGER trg_memberships_updated_at
  BEFORE UPDATE ON memberships
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_conversations_updated_at ON conversations;
CREATE TRIGGER trg_conversations_updated_at
  BEFORE UPDATE ON conversations
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_memories_updated_at ON memories;
CREATE TRIGGER trg_memories_updated_at
  BEFORE UPDATE ON memories
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_user_preferences_updated_at ON user_preferences;
CREATE TRIGGER trg_user_preferences_updated_at
  BEFORE UPDATE ON user_preferences
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_device_settings_updated_at ON device_settings;
CREATE TRIGGER trg_device_settings_updated_at
  BEFORE UPDATE ON device_settings
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_app_settings_updated_at ON app_settings;
CREATE TRIGGER trg_app_settings_updated_at
  BEFORE UPDATE ON app_settings
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_personalities_updated_at ON personalities;
CREATE TRIGGER trg_personalities_updated_at
  BEFORE UPDATE ON personalities
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_system_prompts_updated_at ON system_prompts;
CREATE TRIGGER trg_system_prompts_updated_at
  BEFORE UPDATE ON system_prompts
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_voice_profiles_updated_at ON voice_profiles;
CREATE TRIGGER trg_voice_profiles_updated_at
  BEFORE UPDATE ON voice_profiles
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_skill_groups_updated_at ON skill_groups;
CREATE TRIGGER trg_skill_groups_updated_at
  BEFORE UPDATE ON skill_groups
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_skills_updated_at ON skills;
CREATE TRIGGER trg_skills_updated_at
  BEFORE UPDATE ON skills
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_plugin_connections_updated_at ON plugin_connections;
CREATE TRIGGER trg_plugin_connections_updated_at
  BEFORE UPDATE ON plugin_connections
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_mcp_servers_updated_at ON mcp_servers;
CREATE TRIGGER trg_mcp_servers_updated_at
  BEFORE UPDATE ON mcp_servers
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_hooks_updated_at ON hooks;
CREATE TRIGGER trg_hooks_updated_at
  BEFORE UPDATE ON hooks
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_scheduled_tasks_updated_at ON scheduled_tasks;
CREATE TRIGGER trg_scheduled_tasks_updated_at
  BEFORE UPDATE ON scheduled_tasks
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

DROP TRIGGER IF EXISTS trg_teams_updated_at ON teams;
CREATE TRIGGER trg_teams_updated_at
  BEFORE UPDATE ON teams
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();
