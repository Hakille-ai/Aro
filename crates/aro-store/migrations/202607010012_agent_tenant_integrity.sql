ALTER TABLE agent_permission_profiles
  ADD CONSTRAINT agent_permission_profiles_org_id_unique UNIQUE (organization_id, id);

ALTER TABLE agent_lanes
  ADD CONSTRAINT agent_lanes_org_id_unique UNIQUE (organization_id, id);

ALTER TABLE agent_runs
  ADD CONSTRAINT agent_runs_org_id_unique UNIQUE (organization_id, id);

ALTER TABLE agent_runs
  DROP CONSTRAINT IF EXISTS agent_runs_autonomy_profile_id_fkey,
  ADD CONSTRAINT agent_runs_autonomy_profile_org_fk
    FOREIGN KEY (organization_id, autonomy_profile_id)
    REFERENCES agent_permission_profiles(organization_id, id)
    ON DELETE NO ACTION;

ALTER TABLE agent_runs
  DROP CONSTRAINT IF EXISTS agent_runs_lane_fk,
  ADD CONSTRAINT agent_runs_lane_org_fk
    FOREIGN KEY (organization_id, lane_id)
    REFERENCES agent_lanes(organization_id, id)
    ON DELETE NO ACTION;

ALTER TABLE agent_runs
  DROP CONSTRAINT IF EXISTS agent_runs_conversation_id_fkey,
  ADD CONSTRAINT agent_runs_conversation_org_fk
    FOREIGN KEY (organization_id, conversation_id)
    REFERENCES conversations(organization_id, id)
    ON DELETE NO ACTION;

ALTER TABLE agent_lanes
  DROP CONSTRAINT IF EXISTS agent_lanes_conversation_id_fkey,
  ADD CONSTRAINT agent_lanes_conversation_org_fk
    FOREIGN KEY (organization_id, conversation_id)
    REFERENCES conversations(organization_id, id)
    ON DELETE NO ACTION;

ALTER TABLE agent_steps
  DROP CONSTRAINT IF EXISTS agent_steps_run_id_fkey,
  ADD CONSTRAINT agent_steps_run_org_fk
    FOREIGN KEY (organization_id, run_id)
    REFERENCES agent_runs(organization_id, id)
    ON DELETE CASCADE;

ALTER TABLE agent_artifacts
  DROP CONSTRAINT IF EXISTS agent_artifacts_run_id_fkey,
  ADD CONSTRAINT agent_artifacts_run_org_fk
    FOREIGN KEY (organization_id, run_id)
    REFERENCES agent_runs(organization_id, id)
    ON DELETE CASCADE;

ALTER TABLE agent_context_items
  DROP CONSTRAINT IF EXISTS agent_context_items_run_id_fkey,
  ADD CONSTRAINT agent_context_items_run_org_fk
    FOREIGN KEY (organization_id, run_id)
    REFERENCES agent_runs(organization_id, id)
    ON DELETE CASCADE;

ALTER TABLE agent_context_items
  DROP CONSTRAINT IF EXISTS agent_context_items_conversation_id_fkey,
  ADD CONSTRAINT agent_context_items_conversation_org_fk
    FOREIGN KEY (organization_id, conversation_id)
    REFERENCES conversations(organization_id, id)
    ON DELETE NO ACTION;
