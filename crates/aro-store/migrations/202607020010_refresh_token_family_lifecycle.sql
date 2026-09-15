-- A refresh-token family is the durable security boundary for one authenticated device
-- session. Token rows are rotating credentials; the family owns absolute lifetime,
-- compromise/revocation state, and safe whole-family retention.
-- CREATE TRIGGER takes SHARE ROW EXCLUSIVE on refresh_tokens. Fail quickly behind a long writer
-- instead of creating a lock queue; the transactional migration can be retried safely.
SET LOCAL lock_timeout = '5s';

DO $refresh_family_isolation$
BEGIN
  IF current_setting('transaction_isolation') <> 'read committed' THEN
    RAISE EXCEPTION 'refresh-token rolling compatibility requires READ COMMITTED isolation';
  END IF;
END
$refresh_family_isolation$;

CREATE TABLE refresh_token_families (
  id UUID PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  device_id UUID REFERENCES devices(id) ON DELETE SET NULL,
  absolute_expires_at TIMESTAMPTZ NOT NULL,
  revoked_at TIMESTAMPTZ,
  reuse_detected_at TIMESTAMPTZ,
  last_rotated_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CONSTRAINT refresh_token_families_expiry_after_creation
    CHECK (absolute_expires_at > created_at),
  CONSTRAINT refresh_token_families_reuse_requires_revocation
    CHECK (reuse_detected_at IS NULL OR revoked_at IS NOT NULL)
);

-- Build indexes while the table is still empty. This is faster and safer than attempting a
-- multi-statement CREATE INDEX CONCURRENTLY migration through SQLx's single-query executor.
CREATE INDEX idx_refresh_token_families_user
  ON refresh_token_families (user_id, created_at DESC);

CREATE INDEX idx_refresh_token_families_purge_expired
  ON refresh_token_families (absolute_expires_at, id);

CREATE INDEX idx_refresh_token_families_purge_revoked
  ON refresh_token_families (revoked_at, id)
  WHERE revoked_at IS NOT NULL;

-- Expand/deploy compatibility bridge. Replicas from the previous release omit `family_id` on a
-- primary login and rely on its UUID default. Install this trigger before the backfill so every
-- concurrent old-style INSERT creates its parent row before the FK is added. The trigger remains
-- until a later contract migration runs after all old replicas have drained.
CREATE OR REPLACE FUNCTION aro_ensure_refresh_token_family_compat()
RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $aro_refresh_family_compat$
DECLARE
  family_user_id UUID;
  family_device_id UUID;
  family_absolute_expires_at TIMESTAMPTZ;
  family_revoked_at TIMESTAMPTZ;
BEGIN
  IF NEW.family_id IS NULL THEN
    NEW.family_id := gen_random_uuid();
  END IF;

  INSERT INTO public.refresh_token_families (
    id, user_id, device_id, absolute_expires_at, revoked_at, reuse_detected_at,
    last_rotated_at, created_at, updated_at
  )
  VALUES (
    NEW.family_id,
    NEW.user_id,
    NEW.device_id,
    NEW.expires_at,
    NEW.revoked_at,
    NEW.reuse_detected_at,
    NEW.rotated_at,
    LEAST(COALESCE(NEW.created_at, statement_timestamp()), NEW.expires_at - INTERVAL '1 microsecond'),
    statement_timestamp()
  )
  ON CONFLICT (id) DO NOTHING;

  SELECT user_id, device_id, absolute_expires_at, revoked_at
  INTO STRICT family_user_id, family_device_id, family_absolute_expires_at, family_revoked_at
  FROM public.refresh_token_families
  WHERE id = NEW.family_id
  FOR SHARE;

  IF family_user_id <> NEW.user_id
     OR family_device_id IS DISTINCT FROM NEW.device_id THEN
    RAISE EXCEPTION 'refresh token family identity mismatch'
      USING ERRCODE = '23514';
  END IF;

  IF family_revoked_at IS NOT NULL
     OR family_absolute_expires_at <= statement_timestamp() THEN
    RAISE EXCEPTION 'refresh token family is revoked or expired'
      USING ERRCODE = '23514';
  END IF;

  -- An old replica may still calculate a fresh per-token TTL on rotation. Never let that extend
  -- the absolute family lifetime established by the first post-expand insert/backfill.
  NEW.expires_at := LEAST(NEW.expires_at, family_absolute_expires_at);
  RETURN NEW;
END
$aro_refresh_family_compat$;

REVOKE ALL ON FUNCTION aro_ensure_refresh_token_family_compat() FROM PUBLIC;

CREATE TRIGGER trg_refresh_tokens_family_compat
  BEFORE INSERT ON refresh_tokens
  FOR EACH ROW
  EXECUTE FUNCTION aro_ensure_refresh_token_family_compat();

-- A previous-release replica detects rotated-token reuse by updating refresh_tokens only; it
-- cannot lock the new family row. A statement-level transition-table trigger bridges that update.
-- Its final UPDATE runs as a new VOLATILE-function statement, so it sees descendants committed
-- while the legacy UPDATE was waiting and revokes them before the transaction can commit.
CREATE OR REPLACE FUNCTION aro_bridge_legacy_refresh_reuse()
RETURNS TRIGGER
LANGUAGE plpgsql
VOLATILE
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $aro_legacy_refresh_reuse$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM legacy_refresh_old old_token
    JOIN legacy_refresh_new new_token USING (id)
    WHERE old_token.reuse_detected_at IS NULL
      AND new_token.reuse_detected_at IS NOT NULL
  ) THEN
    RETURN NULL;
  END IF;

  -- The regular backfill may not have run yet. Materialize any compromised parent first, with a
  -- revoked state from birth so the family constraint cannot transiently accept a successor.
  INSERT INTO public.refresh_token_families (
    id, user_id, device_id, absolute_expires_at, revoked_at, reuse_detected_at,
    last_rotated_at, created_at, updated_at
  )
  SELECT
    token.family_id,
    MIN(token.user_id::text)::uuid,
    MIN(token.device_id::text)::uuid,
    MAX(token.expires_at),
    statement_timestamp(),
    MIN(compromised.detected_at),
    MAX(token.rotated_at),
    LEAST(MIN(token.created_at), MAX(token.expires_at) - INTERVAL '1 microsecond'),
    statement_timestamp()
  FROM public.refresh_tokens token
  JOIN (
    SELECT new_token.family_id, MIN(new_token.reuse_detected_at) AS detected_at
    FROM legacy_refresh_old old_token
    JOIN legacy_refresh_new new_token USING (id)
    WHERE old_token.reuse_detected_at IS NULL
      AND new_token.reuse_detected_at IS NOT NULL
    GROUP BY new_token.family_id
  ) compromised ON compromised.family_id = token.family_id
  GROUP BY token.family_id
  ON CONFLICT (id) DO NOTHING;

  -- Updating the family acquires the same row lock used by new replicas before rotating tokens.
  -- This may deadlock with a legacy token-first transaction; PostgreSQL aborts one participant,
  -- which is a safe transient failure rather than a surviving credential.
  UPDATE public.refresh_token_families family
  SET revoked_at = COALESCE(family.revoked_at, statement_timestamp()),
      reuse_detected_at = CASE
        WHEN family.reuse_detected_at IS NULL THEN compromised.detected_at
        ELSE LEAST(family.reuse_detected_at, compromised.detected_at)
      END,
      updated_at = statement_timestamp()
  FROM (
    SELECT new_token.family_id, MIN(new_token.reuse_detected_at) AS detected_at
    FROM legacy_refresh_old old_token
    JOIN legacy_refresh_new new_token USING (id)
    WHERE old_token.reuse_detected_at IS NULL
      AND new_token.reuse_detected_at IS NOT NULL
    GROUP BY new_token.family_id
  ) compromised
  WHERE family.id = compromised.family_id;

  UPDATE public.refresh_tokens token
  SET revoked_at = COALESCE(token.revoked_at, statement_timestamp())
  WHERE token.family_id IN (
    SELECT new_token.family_id
    FROM legacy_refresh_old old_token
    JOIN legacy_refresh_new new_token USING (id)
    WHERE old_token.reuse_detected_at IS NULL
      AND new_token.reuse_detected_at IS NOT NULL
  )
    AND token.revoked_at IS NULL;

  RETURN NULL;
END
$aro_legacy_refresh_reuse$;

REVOKE ALL ON FUNCTION aro_bridge_legacy_refresh_reuse() FROM PUBLIC;

CREATE TRIGGER trg_refresh_tokens_legacy_reuse_bridge
  AFTER UPDATE ON refresh_tokens
  REFERENCING OLD TABLE AS legacy_refresh_old NEW TABLE AS legacy_refresh_new
  FOR EACH STATEMENT
  EXECUTE FUNCTION aro_bridge_legacy_refresh_reuse();

-- Backfill, FK validation, maintenance function and runtime grants are split
-- into later migrations. Committing this short expand phase promptly keeps old replicas writable
-- and closes the only window in which a token could be inserted without a parent family.
