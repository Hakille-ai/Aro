-- Runtime roles are provisioned by the deployment layer, not by application migrations.
-- Grants are deliberately explicit: a new table remains inaccessible until a reviewed
-- migration adds it to the appropriate role.
DO $aro_roles$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app') THEN
    EXECUTE 'GRANT USAGE ON SCHEMA public TO aro_app';
    EXECUTE 'GRANT SELECT ON TABLE _sqlx_migrations TO aro_app';
    EXECUTE 'GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE
      users, organizations, memberships, devices, refresh_tokens, api_keys,
      conversations, messages, memories, user_preferences, device_settings, app_settings,
      personalities, system_prompts, voice_profiles, skill_groups, skills,
      plugin_connections, mcp_servers, hooks, scheduled_tasks, task_runs,
      usage_events, audit_events, outbox_events, client_state, teams,
      agent_permission_profiles, agent_runs, agent_lanes, agent_steps, agent_artifacts,
      agent_context_items, integration_accounts,
      integration_connections, integration_credentials, integration_oauth_states,
      file_objects, file_upload_sessions, file_links, file_chunks, file_index_jobs,
      file_scan_jobs, organization_invitations, idempotency_requests
      TO aro_app';
    EXECUTE 'GRANT SELECT ON TABLE integration_providers TO aro_app';
  END IF;

  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_worker') THEN
    EXECUTE 'GRANT USAGE ON SCHEMA public TO aro_worker';
    EXECUTE 'GRANT SELECT ON TABLE _sqlx_migrations TO aro_worker';
    EXECUTE 'GRANT SELECT, INSERT, UPDATE ON TABLE
      file_objects, file_scan_jobs, file_index_jobs, audit_events, outbox_events
      TO aro_worker';
  END IF;
END
$aro_roles$;
