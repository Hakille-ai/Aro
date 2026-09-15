-- FK validation uses a lock mode compatible with ordinary INSERT/UPDATE/DELETE. Keep acquisition
-- bounded so an unrelated DDL operation cannot turn this deployment into an unbounded wait.
SET LOCAL lock_timeout = '5s';

ALTER TABLE refresh_tokens
  VALIDATE CONSTRAINT refresh_tokens_family_fk;

CREATE TRIGGER trg_refresh_token_families_updated_at
  BEFORE UPDATE ON refresh_token_families
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();

-- Delete complete lineages only after a bounded retention window. Separate candidate branches
-- allow PostgreSQL to use the expiry and revocation indexes instead of scanning every session.
CREATE OR REPLACE FUNCTION aro_purge_refresh_token_families(
  batch_limit INTEGER,
  retention_days INTEGER
)
RETURNS BIGINT
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $aro_purge_refresh_families$
DECLARE
  deleted_count BIGINT;
  retention_cutoff TIMESTAMPTZ;
  bounded_batch_limit INTEGER;
BEGIN
  bounded_batch_limit := LEAST(GREATEST(COALESCE(batch_limit, 1), 1), 10000);
  retention_cutoff := statement_timestamp() - make_interval(
    days => LEAST(GREATEST(COALESCE(retention_days, 30), 1), 365)
  );

  WITH expired_candidates AS MATERIALIZED (
    SELECT id, absolute_expires_at AS purge_at
    FROM public.refresh_token_families
    WHERE absolute_expires_at <= retention_cutoff
    ORDER BY absolute_expires_at ASC, id ASC
    LIMIT bounded_batch_limit
    FOR UPDATE SKIP LOCKED
  ), revoked_candidates AS MATERIALIZED (
    SELECT family.id, family.revoked_at AS purge_at
    FROM public.refresh_token_families family
    WHERE family.revoked_at <= retention_cutoff
      AND NOT EXISTS (
        SELECT 1
        FROM expired_candidates expired
        WHERE expired.id = family.id
      )
    ORDER BY family.revoked_at ASC, family.id ASC
    LIMIT bounded_batch_limit
    FOR UPDATE OF family SKIP LOCKED
  ), purgeable AS (
    SELECT candidate.id
    FROM (
      SELECT id, purge_at FROM expired_candidates
      UNION ALL
      SELECT id, purge_at FROM revoked_candidates
    ) candidate
    ORDER BY candidate.purge_at ASC, candidate.id ASC
    LIMIT bounded_batch_limit
  )
  DELETE FROM public.refresh_token_families family
  USING purgeable
  WHERE family.id = purgeable.id;

  GET DIAGNOSTICS deleted_count = ROW_COUNT;
  RETURN deleted_count;
END
$aro_purge_refresh_families$;

REVOKE ALL ON FUNCTION aro_purge_refresh_token_families(INTEGER, INTEGER) FROM PUBLIC;

DO $refresh_family_runtime_grants$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app') THEN
    EXECUTE 'GRANT SELECT, INSERT, UPDATE ON TABLE public.refresh_token_families TO aro_app';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_worker') THEN
    EXECUTE 'GRANT EXECUTE ON FUNCTION aro_purge_refresh_token_families(INTEGER, INTEGER) TO aro_worker';
  END IF;
END
$refresh_family_runtime_grants$;

DROP PROCEDURE aro_backfill_refresh_token_families(INTEGER);
DROP TABLE refresh_token_family_backfill_progress;

-- The family_id default and compatibility trigger are intentionally retained. Remove both only
-- in a later contract release after old replicas and the rollback window have been eliminated.
