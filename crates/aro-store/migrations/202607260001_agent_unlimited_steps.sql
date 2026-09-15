-- Remove the 8/32 model-step cap: agents run until final/pause.
-- Postgres SMALLINT max (32767) is used as effectively-unlimited.

ALTER TABLE agent_run_jobs DROP CONSTRAINT IF EXISTS agent_run_jobs_budgets_valid;
ALTER TABLE agent_run_jobs ADD CONSTRAINT agent_run_jobs_budgets_valid
  CHECK (
    max_attempts BETWEEN 1 AND 20
    AND max_steps BETWEEN 1 AND 32767
    AND max_tool_calls BETWEEN 0 AND 32767
    AND max_input_tokens BETWEEN 1 AND 1048576
    AND max_output_tokens BETWEEN 1 AND 131072
    AND max_wall_time_seconds BETWEEN 1 AND 86400
  );

ALTER TABLE agent_run_jobs ALTER COLUMN max_steps SET DEFAULT 32767;
ALTER TABLE agent_run_jobs ALTER COLUMN max_tool_calls SET DEFAULT 32767;
