-- Durable agent execution queue. Creating this schema does not enqueue or resume any run.
-- Submission snapshots and budgets are immutable after insert; workers only mutate execution
-- state, counters, leases, and result fields.

ALTER TABLE agent_runs
  ADD CONSTRAINT agent_runs_org_id_owner_unique
  UNIQUE (organization_id, id, owner_user_id);

ALTER TABLE messages
  ADD COLUMN source_agent_run_id UUID,
  ADD CONSTRAINT messages_org_id_source_agent_run_unique
    UNIQUE (organization_id, id, source_agent_run_id),
  ADD CONSTRAINT messages_source_agent_run_org_fk
    FOREIGN KEY (organization_id, source_agent_run_id)
    REFERENCES agent_runs(organization_id, id)
    ON DELETE SET NULL (source_agent_run_id);

CREATE TABLE agent_run_jobs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  agent_run_id UUID NOT NULL,
  submitted_by_user_id UUID NOT NULL,
  submission_key TEXT NOT NULL,
  request_hash BYTEA NOT NULL,
  request_snapshot_encrypted BYTEA NOT NULL,
  request_snapshot_key_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'queued',
  priority TEXT NOT NULL DEFAULT 'normal',
  max_attempts SMALLINT NOT NULL DEFAULT 3,
  max_steps SMALLINT NOT NULL DEFAULT 8,
  max_tool_calls SMALLINT NOT NULL DEFAULT 8,
  max_input_tokens INTEGER NOT NULL DEFAULT 131072,
  max_output_tokens INTEGER NOT NULL DEFAULT 8192,
  max_wall_time_seconds INTEGER NOT NULL DEFAULT 900,
  attempts SMALLINT NOT NULL DEFAULT 0,
  steps_consumed SMALLINT NOT NULL DEFAULT 0,
  tool_calls_consumed SMALLINT NOT NULL DEFAULT 0,
  input_tokens_consumed INTEGER NOT NULL DEFAULT 0,
  output_tokens_consumed INTEGER NOT NULL DEFAULT 0,
  available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  lease_token UUID,
  lease_generation BIGINT NOT NULL DEFAULT 0,
  lease_owner TEXT,
  leased_at TIMESTAMPTZ,
  lease_expires_at TIMESTAMPTZ,
  cancel_requested_at TIMESTAMPTZ,
  pause_requested_at TIMESTAMPTZ,
  started_at TIMESTAMPTZ,
  completed_at TIMESTAMPTZ,
  state_reason TEXT,
  last_error TEXT,
  result_message_id UUID,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

  CONSTRAINT agent_run_jobs_org_id_unique
    UNIQUE (organization_id, id),
  CONSTRAINT agent_run_jobs_org_id_run_id_unique
    UNIQUE (organization_id, id, agent_run_id),
  CONSTRAINT agent_run_jobs_run_unique
    UNIQUE (organization_id, agent_run_id),
  CONSTRAINT agent_run_jobs_submission_unique
    UNIQUE (organization_id, submitted_by_user_id, submission_key),
  CONSTRAINT agent_run_jobs_owned_run_fk
    FOREIGN KEY (organization_id, agent_run_id, submitted_by_user_id)
    REFERENCES agent_runs(organization_id, id, owner_user_id)
    ON DELETE CASCADE,
  CONSTRAINT agent_run_jobs_result_message_fk
    FOREIGN KEY (organization_id, result_message_id, agent_run_id)
    REFERENCES messages(organization_id, id, source_agent_run_id)
    ON DELETE SET NULL (result_message_id)
    DEFERRABLE INITIALLY DEFERRED,

  CONSTRAINT agent_run_jobs_submission_key_valid
    CHECK (
      octet_length(submission_key) BETWEEN 1 AND 255
      AND submission_key = btrim(submission_key)
    ),
  CONSTRAINT agent_run_jobs_request_hash_valid
    CHECK (octet_length(request_hash) = 32),
  CONSTRAINT agent_run_jobs_request_snapshot_valid
    CHECK (
      octet_length(request_snapshot_encrypted) BETWEEN 1 AND 1114112
      AND octet_length(request_snapshot_key_id) BETWEEN 1 AND 128
      AND request_snapshot_key_id = btrim(request_snapshot_key_id)
      AND request_snapshot_key_id !~ '[^A-Za-z0-9._:-]'
    ),
  CONSTRAINT agent_run_jobs_status_valid
    CHECK (
      status IN (
        'queued', 'leased', 'running', 'waiting', 'retry_wait',
        'completed', 'failed', 'cancelled'
      )
    ),
  CONSTRAINT agent_run_jobs_priority_valid
    CHECK (priority IN ('low', 'normal', 'high', 'critical')),
  CONSTRAINT agent_run_jobs_budgets_valid
    CHECK (
      max_attempts BETWEEN 1 AND 20
      AND max_steps BETWEEN 1 AND 32
      AND max_tool_calls BETWEEN 0 AND 64
      AND max_input_tokens BETWEEN 1 AND 1048576
      AND max_output_tokens BETWEEN 1 AND 131072
      AND max_wall_time_seconds BETWEEN 1 AND 86400
    ),
  CONSTRAINT agent_run_jobs_counters_valid
    CHECK (
      attempts BETWEEN 0 AND max_attempts
      AND steps_consumed BETWEEN 0 AND max_steps
      AND tool_calls_consumed BETWEEN 0 AND max_tool_calls
      AND input_tokens_consumed BETWEEN 0 AND max_input_tokens
      AND output_tokens_consumed BETWEEN 0 AND max_output_tokens
      AND lease_generation = attempts
    ),
  CONSTRAINT agent_run_jobs_lease_owner_valid
    CHECK (
      lease_owner IS NULL
      OR (
        octet_length(lease_owner) BETWEEN 1 AND 200
        AND lease_owner = btrim(lease_owner)
      )
    ),
  CONSTRAINT agent_run_jobs_lease_tuple_valid
    CHECK (
      (
        status IN ('leased', 'running')
        AND lease_token IS NOT NULL
        AND lease_owner IS NOT NULL
        AND leased_at IS NOT NULL
        AND lease_expires_at IS NOT NULL
        AND lease_expires_at > leased_at
      )
      OR
      (
        status NOT IN ('leased', 'running')
        AND lease_token IS NULL
        AND lease_owner IS NULL
        AND leased_at IS NULL
        AND lease_expires_at IS NULL
      )
    ),
  CONSTRAINT agent_run_jobs_completion_valid
    CHECK (
      (
        status IN ('completed', 'failed', 'cancelled')
        AND completed_at IS NOT NULL
      )
      OR
      (
        status NOT IN ('completed', 'failed', 'cancelled')
        AND completed_at IS NULL
      )
    ),
  CONSTRAINT agent_run_jobs_result_valid
    CHECK (result_message_id IS NULL OR status = 'completed'),
  CONSTRAINT agent_run_jobs_control_requests_valid
    CHECK (cancel_requested_at IS NULL OR pause_requested_at IS NULL),
  CONSTRAINT agent_run_jobs_state_reason_valid
    CHECK (
      state_reason IS NULL
      OR (
        octet_length(state_reason) BETWEEN 1 AND 2048
        AND state_reason = btrim(state_reason)
      )
    ),
  CONSTRAINT agent_run_jobs_last_error_valid
    CHECK (last_error IS NULL OR octet_length(last_error) BETWEEN 1 AND 16384),
  CONSTRAINT agent_run_jobs_timestamps_valid
    CHECK (
      available_at >= created_at
      AND updated_at >= created_at
      AND (leased_at IS NULL OR leased_at >= created_at)
      AND (cancel_requested_at IS NULL OR cancel_requested_at >= created_at)
      AND (pause_requested_at IS NULL OR pause_requested_at >= created_at)
      AND (started_at IS NULL OR started_at >= created_at)
      AND (
        completed_at IS NULL
        OR completed_at >= COALESCE(started_at, created_at)
      )
    )
);

CREATE OR REPLACE FUNCTION aro_guard_agent_run_job_update()
RETURNS TRIGGER
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
  IF ROW(
    NEW.organization_id,
    NEW.agent_run_id,
    NEW.submitted_by_user_id,
    NEW.submission_key,
    NEW.request_hash,
    NEW.max_attempts,
    NEW.max_steps,
    NEW.max_tool_calls,
    NEW.max_input_tokens,
    NEW.max_output_tokens,
    NEW.max_wall_time_seconds,
    NEW.created_at
  ) IS DISTINCT FROM ROW(
    OLD.organization_id,
    OLD.agent_run_id,
    OLD.submitted_by_user_id,
    OLD.submission_key,
    OLD.request_hash,
    OLD.max_attempts,
    OLD.max_steps,
    OLD.max_tool_calls,
    OLD.max_input_tokens,
    OLD.max_output_tokens,
    OLD.max_wall_time_seconds,
    OLD.created_at
  ) THEN
    RAISE EXCEPTION 'agent job submission snapshot and budgets are immutable'
      USING ERRCODE = '55000';
  END IF;

  IF ROW(
    NEW.request_snapshot_encrypted,
    NEW.request_snapshot_key_id
  ) IS DISTINCT FROM ROW(
    OLD.request_snapshot_encrypted,
    OLD.request_snapshot_key_id
  ) AND NOT pg_has_role(
    current_user,
    (SELECT relowner FROM pg_class WHERE oid = TG_RELID),
    'USAGE'
  ) THEN
    RAISE EXCEPTION 'agent job encrypted snapshot can only be rotated by the schema owner'
      USING ERRCODE = '55000';
  END IF;

  IF NEW.attempts < OLD.attempts
     OR NEW.steps_consumed < OLD.steps_consumed
     OR NEW.tool_calls_consumed < OLD.tool_calls_consumed
     OR NEW.input_tokens_consumed < OLD.input_tokens_consumed
     OR NEW.output_tokens_consumed < OLD.output_tokens_consumed THEN
    RAISE EXCEPTION 'agent job execution counters cannot decrease'
      USING ERRCODE = '55000';
  END IF;

  IF NEW.lease_token IS DISTINCT FROM OLD.lease_token THEN
    IF NEW.lease_token IS NULL THEN
      IF NEW.lease_generation <> OLD.lease_generation
         OR NEW.attempts <> OLD.attempts THEN
        RAISE EXCEPTION 'releasing an agent job lease cannot change its generation'
          USING ERRCODE = '55000';
      END IF;
    ELSIF NEW.lease_generation <> OLD.lease_generation + 1
          OR NEW.attempts <> OLD.attempts + 1 THEN
      RAISE EXCEPTION 'acquiring an agent job lease must advance its generation once'
        USING ERRCODE = '55000';
    END IF;
  ELSIF NEW.lease_generation <> OLD.lease_generation
        OR NEW.attempts <> OLD.attempts THEN
    RAISE EXCEPTION 'agent job lease generation requires a new lease token'
      USING ERRCODE = '55000';
  END IF;

  IF OLD.status IN ('completed', 'failed', 'cancelled')
     AND ROW(
       NEW.status,
       NEW.priority,
       NEW.attempts,
       NEW.steps_consumed,
       NEW.tool_calls_consumed,
       NEW.input_tokens_consumed,
       NEW.output_tokens_consumed,
       NEW.available_at,
       NEW.lease_token,
       NEW.lease_generation,
       NEW.lease_owner,
       NEW.leased_at,
       NEW.lease_expires_at,
       NEW.cancel_requested_at,
       NEW.pause_requested_at,
       NEW.started_at,
       NEW.completed_at,
       NEW.state_reason,
       NEW.last_error,
       NEW.result_message_id
     ) IS DISTINCT FROM ROW(
       OLD.status,
       OLD.priority,
       OLD.attempts,
       OLD.steps_consumed,
       OLD.tool_calls_consumed,
       OLD.input_tokens_consumed,
       OLD.output_tokens_consumed,
       OLD.available_at,
       OLD.lease_token,
       OLD.lease_generation,
       OLD.lease_owner,
       OLD.leased_at,
       OLD.lease_expires_at,
       OLD.cancel_requested_at,
       OLD.pause_requested_at,
       OLD.started_at,
       OLD.completed_at,
       OLD.state_reason,
       OLD.last_error,
       OLD.result_message_id
     ) THEN
    RAISE EXCEPTION 'terminal agent job state is immutable'
      USING ERRCODE = '55000';
  END IF;

  RETURN NEW;
END;
$$;

REVOKE ALL ON FUNCTION aro_guard_agent_run_job_update() FROM PUBLIC;

CREATE TRIGGER trg_agent_run_jobs_guard
  BEFORE UPDATE ON agent_run_jobs
  FOR EACH ROW
  EXECUTE FUNCTION aro_guard_agent_run_job_update();

CREATE TRIGGER trg_agent_run_jobs_updated_at
  BEFORE UPDATE ON agent_run_jobs
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

-- Online envelope-key rotation. Runtime roles receive no EXECUTE grant; an operator invokes this
-- through the schema-owner migration identity in bounded batches while workers carry both keys.
CREATE OR REPLACE FUNCTION aro_rotate_agent_job_snapshot_key(
  old_key_id TEXT,
  new_key_id TEXT,
  old_key TEXT,
  new_key TEXT,
  batch_limit INTEGER
)
RETURNS BIGINT
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $aro_rotate_agent_snapshot_key$
DECLARE
  rotated_count BIGINT;
BEGIN
  IF old_key_id IS NULL
     OR new_key_id IS NULL
     OR old_key_id !~ '^[A-Za-z0-9._:-]{1,128}$'
     OR new_key_id !~ '^[A-Za-z0-9._:-]{1,128}$'
     OR octet_length(old_key) NOT BETWEEN 32 AND 4096
     OR octet_length(new_key) NOT BETWEEN 32 AND 4096 THEN
    RAISE EXCEPTION 'invalid agent snapshot key rotation arguments'
      USING ERRCODE = '22023';
  END IF;

  WITH candidates AS (
    SELECT id
    FROM public.agent_run_jobs
    WHERE request_snapshot_key_id = old_key_id
    ORDER BY created_at, id
    FOR UPDATE SKIP LOCKED
    LIMIT LEAST(GREATEST(COALESCE(batch_limit, 1), 1), 1000)
  )
  UPDATE public.agent_run_jobs AS job
  SET request_snapshot_encrypted = pgp_sym_encrypt(
        pgp_sym_decrypt(job.request_snapshot_encrypted, old_key),
        new_key,
        'cipher-algo=aes256,compress-algo=1'
      ),
      request_snapshot_key_id = new_key_id,
      updated_at = now()
  FROM candidates
  WHERE job.id = candidates.id;

  GET DIAGNOSTICS rotated_count = ROW_COUNT;
  RETURN rotated_count;
END
$aro_rotate_agent_snapshot_key$;

REVOKE ALL ON FUNCTION aro_rotate_agent_job_snapshot_key(TEXT, TEXT, TEXT, TEXT, INTEGER)
  FROM PUBLIC;

CREATE OR REPLACE FUNCTION aro_purge_terminal_agent_run_jobs(
  batch_limit INTEGER,
  retention_days INTEGER
)
RETURNS BIGINT
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $aro_purge_terminal_agent_jobs$
DECLARE
  deleted_count BIGINT;
BEGIN
  WITH candidates AS (
    SELECT id
    FROM public.agent_run_jobs
    WHERE status IN ('completed', 'failed', 'cancelled')
      AND completed_at <= statement_timestamp() - make_interval(
        days => LEAST(GREATEST(COALESCE(retention_days, 30), 1), 3650)
      )
    ORDER BY completed_at, id
    FOR UPDATE SKIP LOCKED
    LIMIT LEAST(GREATEST(COALESCE(batch_limit, 1), 1), 10000)
  )
  DELETE FROM public.agent_run_jobs AS job
  USING candidates
  WHERE job.id = candidates.id;

  GET DIAGNOSTICS deleted_count = ROW_COUNT;
  RETURN deleted_count;
END
$aro_purge_terminal_agent_jobs$;

REVOKE ALL ON FUNCTION aro_purge_terminal_agent_run_jobs(INTEGER, INTEGER) FROM PUBLIC;

CREATE TABLE agent_run_events (
  event_cursor BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  event_id UUID NOT NULL DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  agent_run_id UUID NOT NULL,
  job_id UUID,
  event_type TEXT NOT NULL,
  event_version SMALLINT NOT NULL DEFAULT 1,
  payload JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

  CONSTRAINT agent_run_events_org_event_unique
    UNIQUE (organization_id, event_id),
  CONSTRAINT agent_run_events_run_org_fk
    FOREIGN KEY (organization_id, agent_run_id)
    REFERENCES agent_runs(organization_id, id)
    ON DELETE CASCADE,
  CONSTRAINT agent_run_events_job_run_org_fk
    FOREIGN KEY (organization_id, job_id, agent_run_id)
    REFERENCES agent_run_jobs(organization_id, id, agent_run_id)
    ON DELETE CASCADE,
  CONSTRAINT agent_run_events_type_valid
    CHECK (
      octet_length(event_type) BETWEEN 1 AND 120
      AND event_type = btrim(event_type)
    ),
  CONSTRAINT agent_run_events_version_valid
    CHECK (event_version BETWEEN 1 AND 32767),
  CONSTRAINT agent_run_events_payload_valid
    CHECK (
      jsonb_typeof(payload) = 'object'
      AND octet_length(payload::text) <= 65536
    )
);

CREATE OR REPLACE FUNCTION aro_reject_agent_run_event_mutation()
RETURNS TRIGGER
LANGUAGE plpgsql
SET search_path = pg_catalog, public
AS $$
BEGIN
  -- Parent-table cascades are permitted for retention/privacy cleanup. Runtime roles cannot
  -- delete parent rows, so ordinary event usage remains strictly append-only.
  IF TG_OP = 'DELETE' AND pg_trigger_depth() > 1 THEN
    RETURN OLD;
  END IF;

  RAISE EXCEPTION 'agent run events are append-only'
    USING ERRCODE = '55000';
END;
$$;

REVOKE ALL ON FUNCTION aro_reject_agent_run_event_mutation() FROM PUBLIC;

CREATE TRIGGER trg_agent_run_events_append_only
  BEFORE UPDATE OR DELETE ON agent_run_events
  FOR EACH ROW
  EXECUTE FUNCTION aro_reject_agent_run_event_mutation();

CREATE INDEX idx_agent_run_jobs_claimable
  ON agent_run_jobs (
    (CASE priority
      WHEN 'critical' THEN 0
      WHEN 'high' THEN 1
      WHEN 'normal' THEN 2
      ELSE 3
    END),
    available_at,
    created_at,
    id
  )
  WHERE status IN ('queued', 'retry_wait')
    AND cancel_requested_at IS NULL
    AND pause_requested_at IS NULL;

CREATE INDEX idx_agent_run_jobs_expired_lease
  ON agent_run_jobs (lease_expires_at, id)
  WHERE status IN ('leased', 'running');

CREATE INDEX idx_agent_run_jobs_org_status_updated
  ON agent_run_jobs (organization_id, status, updated_at DESC, id);

CREATE UNIQUE INDEX idx_agent_run_jobs_result_message
  ON agent_run_jobs (organization_id, result_message_id)
  WHERE result_message_id IS NOT NULL;

CREATE INDEX idx_messages_source_agent_run
  ON messages (organization_id, source_agent_run_id, created_at, id)
  WHERE source_agent_run_id IS NOT NULL AND deleted_at IS NULL;

CREATE INDEX idx_agent_run_events_run_cursor
  ON agent_run_events (organization_id, agent_run_id, event_cursor);

CREATE INDEX idx_agent_run_events_job_cursor
  ON agent_run_events (organization_id, job_id, event_cursor)
  WHERE job_id IS NOT NULL;

REVOKE ALL ON TABLE agent_run_jobs, agent_run_events FROM PUBLIC;
REVOKE ALL ON SEQUENCE agent_run_events_event_cursor_seq FROM PUBLIC;

-- Expand phase only: installing the durable queue schema must not pause or otherwise mutate
-- existing runs, and runtime roles intentionally receive no access yet. The cutover migration
-- must grant only fenced queue functions after enqueue, claim, heartbeat, cancellation, retry,
-- completion, event streaming, and rolling-version compatibility are implemented and tested.
