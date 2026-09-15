ALTER TABLE outbox_events
  ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  ADD COLUMN lease_token UUID,
  ADD COLUMN lease_until TIMESTAMPTZ,
  ADD COLUMN last_error TEXT,
  ADD COLUMN last_attempt_at TIMESTAMPTZ;

CREATE INDEX idx_outbox_claimable
  ON outbox_events(available_at, created_at)
  WHERE processed_at IS NULL;

CREATE INDEX idx_outbox_expired_lease
  ON outbox_events(lease_until)
  WHERE processed_at IS NULL AND lease_until IS NOT NULL;
