-- Keep the runtime worker least-privileged: it may invoke this bounded maintenance
-- operation, but it cannot read, update, or delete arbitrary idempotency records.
CREATE OR REPLACE FUNCTION aro_purge_expired_idempotency_requests(batch_limit INTEGER)
RETURNS BIGINT
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $aro_purge_idempotency$
DECLARE
  deleted_count BIGINT;
BEGIN
  WITH expired AS (
    SELECT id
    FROM public.idempotency_requests
    WHERE expires_at <= clock_timestamp()
    ORDER BY expires_at ASC, id ASC
    LIMIT LEAST(GREATEST(COALESCE(batch_limit, 1), 1), 10000)
    FOR UPDATE SKIP LOCKED
  )
  DELETE FROM public.idempotency_requests record
  USING expired
  WHERE record.id = expired.id;

  GET DIAGNOSTICS deleted_count = ROW_COUNT;
  RETURN deleted_count;
END
$aro_purge_idempotency$;

REVOKE ALL ON FUNCTION aro_purge_expired_idempotency_requests(INTEGER) FROM PUBLIC;

DO $aro_worker_idempotency_grant$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_app') THEN
    EXECUTE 'REVOKE DELETE ON TABLE public.idempotency_requests FROM aro_app';
  END IF;
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_worker') THEN
    EXECUTE 'GRANT EXECUTE ON FUNCTION aro_purge_expired_idempotency_requests(INTEGER) TO aro_worker';
  END IF;
END
$aro_worker_idempotency_grant$;
