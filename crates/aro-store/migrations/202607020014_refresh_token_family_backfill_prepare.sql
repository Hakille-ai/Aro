-- Preflight and install the resumable online backfill procedure. The next migration invokes this
-- procedure as one top-level CALL so it can commit after each bounded batch.
DO $refresh_family_consistency$
BEGIN
  IF EXISTS (
    SELECT 1
    FROM refresh_tokens
    GROUP BY family_id
    HAVING COUNT(DISTINCT user_id) > 1
       OR COUNT(DISTINCT device_id) FILTER (WHERE device_id IS NOT NULL) > 1
       OR (BOOL_OR(device_id IS NULL) AND BOOL_OR(device_id IS NOT NULL))
  ) THEN
    RAISE EXCEPTION 'cannot create refresh-token families from inconsistent token lineage';
  END IF;
END
$refresh_family_consistency$;

CREATE TABLE refresh_token_family_backfill_progress (
  migration_key TEXT PRIMARY KEY,
  snapshot_started_at TIMESTAMPTZ NOT NULL,
  last_family_id UUID,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT refresh_token_family_backfill_progress_key
    CHECK (migration_key = 'refresh_token_families_v1')
);

REVOKE ALL ON TABLE refresh_token_family_backfill_progress FROM PUBLIC;

INSERT INTO refresh_token_family_backfill_progress (migration_key, snapshot_started_at)
VALUES ('refresh_token_families_v1', statement_timestamp())
ON CONFLICT (migration_key) DO NOTHING;

CREATE OR REPLACE PROCEDURE aro_backfill_refresh_token_families(batch_limit INTEGER)
LANGUAGE plpgsql
AS $aro_refresh_family_backfill$
DECLARE
  bounded_batch_limit INTEGER := LEAST(GREATEST(COALESCE(batch_limit, 1000), 1), 10000);
  previous_family_id UUID := NULL;
  backfill_started_at TIMESTAMPTZ;
  batch_last_family_id UUID;
  processed_batches BIGINT := 0;
BEGIN
  LOOP
    -- Transaction-local settings are reset by each COMMIT below.
    PERFORM set_config('lock_timeout', '5s', true);

    SELECT progress.last_family_id, progress.snapshot_started_at
    INTO previous_family_id, backfill_started_at
    FROM public.refresh_token_family_backfill_progress progress
    WHERE progress.migration_key = 'refresh_token_families_v1'
    FOR UPDATE;

    SELECT batch.family_id
    INTO batch_last_family_id
    FROM (
      SELECT DISTINCT token.family_id
      FROM public.refresh_tokens token
      WHERE (previous_family_id IS NULL OR token.family_id > previous_family_id)
        AND token.created_at <= backfill_started_at
      ORDER BY token.family_id ASC
      LIMIT bounded_batch_limit
    ) batch
    ORDER BY batch.family_id DESC
    LIMIT 1;

    IF batch_last_family_id IS NULL THEN
      EXIT;
    END IF;

    -- If the legacy token-only implementation ever recorded reuse without revoking a concurrent
    -- descendant, repair that lineage before the family constraint is materialized.
    WITH compromised_families AS (
      SELECT token.family_id
      FROM public.refresh_tokens token
      WHERE (previous_family_id IS NULL OR token.family_id > previous_family_id)
        AND token.family_id <= batch_last_family_id
      GROUP BY token.family_id
      HAVING BOOL_OR(token.reuse_detected_at IS NOT NULL)
    )
    UPDATE public.refresh_tokens token
    SET revoked_at = COALESCE(token.revoked_at, statement_timestamp())
    FROM compromised_families compromised
    WHERE token.family_id = compromised.family_id
      AND token.revoked_at IS NULL;

    -- Existing sessions keep, but cannot extend beyond, their already-issued maximum expiry.
    INSERT INTO public.refresh_token_families (
      id, user_id, device_id, absolute_expires_at, revoked_at, reuse_detected_at,
      last_rotated_at, created_at, updated_at
    )
    SELECT
      token.family_id,
      MIN(token.user_id::text)::uuid,
      MIN(token.device_id::text)::uuid,
      MAX(token.expires_at),
      CASE WHEN BOOL_AND(token.revoked_at IS NOT NULL) THEN MAX(token.revoked_at) END,
      MIN(token.reuse_detected_at),
      MAX(token.rotated_at),
      LEAST(MIN(token.created_at), MAX(token.expires_at) - INTERVAL '1 microsecond'),
      statement_timestamp()
    FROM public.refresh_tokens token
    WHERE (previous_family_id IS NULL OR token.family_id > previous_family_id)
      AND token.family_id <= batch_last_family_id
    GROUP BY token.family_id
    ON CONFLICT (id) DO UPDATE
    SET absolute_expires_at = GREATEST(
          refresh_token_families.absolute_expires_at,
          EXCLUDED.absolute_expires_at
        ),
        revoked_at = CASE
          WHEN refresh_token_families.revoked_at IS NULL THEN EXCLUDED.revoked_at
          WHEN EXCLUDED.revoked_at IS NULL THEN refresh_token_families.revoked_at
          ELSE GREATEST(refresh_token_families.revoked_at, EXCLUDED.revoked_at)
        END,
        reuse_detected_at = CASE
          WHEN refresh_token_families.reuse_detected_at IS NULL THEN EXCLUDED.reuse_detected_at
          WHEN EXCLUDED.reuse_detected_at IS NULL THEN refresh_token_families.reuse_detected_at
          ELSE LEAST(refresh_token_families.reuse_detected_at, EXCLUDED.reuse_detected_at)
        END,
        last_rotated_at = GREATEST(
          refresh_token_families.last_rotated_at,
          EXCLUDED.last_rotated_at
        ),
        created_at = LEAST(refresh_token_families.created_at, EXCLUDED.created_at),
        updated_at = statement_timestamp();

    -- Take a fresh snapshot after the family upsert/locks. A legacy INSERT that committed after
    -- the first repair pass is now visible; future inserts for these compromised families wait on
    -- the family lock and are rejected by the BEFORE INSERT bridge.
    UPDATE public.refresh_tokens token
    SET revoked_at = COALESCE(token.revoked_at, statement_timestamp())
    FROM public.refresh_token_families family
    WHERE token.family_id = family.id
      AND (previous_family_id IS NULL OR family.id > previous_family_id)
      AND family.id <= batch_last_family_id
      AND family.reuse_detected_at IS NOT NULL
      AND token.revoked_at IS NULL;

    IF EXISTS (
      SELECT 1
      FROM public.refresh_tokens token
      JOIN public.refresh_token_families family ON family.id = token.family_id
      WHERE (previous_family_id IS NULL OR token.family_id > previous_family_id)
        AND token.family_id <= batch_last_family_id
        AND (
          family.user_id <> token.user_id
          OR family.device_id IS DISTINCT FROM token.device_id
        )
    ) THEN
      RAISE EXCEPTION 'refresh-token family backfill produced an identity mismatch';
    END IF;

    UPDATE public.refresh_token_family_backfill_progress
    SET last_family_id = batch_last_family_id,
        updated_at = statement_timestamp()
    WHERE migration_key = 'refresh_token_families_v1';

    previous_family_id := batch_last_family_id;
    processed_batches := processed_batches + 1;
    RAISE NOTICE 'refresh-token family backfill committed batch %, through family %',
      processed_batches, previous_family_id;
    COMMIT;
  END LOOP;
END
$aro_refresh_family_backfill$;

REVOKE ALL ON PROCEDURE aro_backfill_refresh_token_families(INTEGER) FROM PUBLIC;
