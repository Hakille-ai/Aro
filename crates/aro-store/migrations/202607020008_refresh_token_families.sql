-- Each login starts an independent refresh-token family. Successive rotations keep the
-- same family identifier so that reuse of an already-rotated credential can invalidate
-- every descendant atomically.
ALTER TABLE refresh_tokens
  ADD COLUMN family_id UUID,
  ADD COLUMN rotated_at TIMESTAMPTZ,
  ADD COLUMN reuse_detected_at TIMESTAMPTZ;

-- Pre-existing tokens become roots of independent families. In particular, an old
-- revoked token is not assumed to have been rotated: that distinction did not exist
-- before this migration and guessing would create false-positive family revocations.
UPDATE refresh_tokens
SET family_id = id
WHERE family_id IS NULL;

ALTER TABLE refresh_tokens
  ALTER COLUMN family_id SET DEFAULT gen_random_uuid(),
  ALTER COLUMN family_id SET NOT NULL,
  ADD CONSTRAINT refresh_tokens_rotation_requires_revocation
    CHECK (rotated_at IS NULL OR revoked_at IS NOT NULL),
  ADD CONSTRAINT refresh_tokens_reuse_requires_rotation
    CHECK (reuse_detected_at IS NULL OR rotated_at IS NOT NULL);

CREATE INDEX idx_refresh_tokens_family
  ON refresh_tokens (family_id);

CREATE INDEX idx_refresh_tokens_family_active
  ON refresh_tokens (family_id, expires_at)
  WHERE revoked_at IS NULL;
