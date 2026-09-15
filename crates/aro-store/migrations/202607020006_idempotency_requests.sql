CREATE TABLE idempotency_requests (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  organization_id UUID NOT NULL,
  actor_id UUID NOT NULL,
  scope TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  request_hash BYTEA NOT NULL,
  status TEXT NOT NULL DEFAULT 'in_progress',
  execution_token UUID NOT NULL DEFAULT gen_random_uuid(),
  lease_expires_at TIMESTAMPTZ NOT NULL,
  response_status SMALLINT,
  response_headers JSONB,
  response_body BYTEA,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  completed_at TIMESTAMPTZ,
  expires_at TIMESTAMPTZ NOT NULL,
  CONSTRAINT idempotency_requests_actor_membership_fk
    FOREIGN KEY (actor_id, organization_id)
    REFERENCES memberships(user_id, organization_id)
    ON DELETE CASCADE,
  CONSTRAINT idempotency_requests_identity_unique
    UNIQUE (organization_id, actor_id, scope, idempotency_key),
  CONSTRAINT idempotency_requests_scope_valid
    CHECK (
      octet_length(scope) BETWEEN 1 AND 200
      AND scope = btrim(scope)
    ),
  CONSTRAINT idempotency_requests_key_valid
    CHECK (
      octet_length(idempotency_key) BETWEEN 1 AND 255
      AND idempotency_key = btrim(idempotency_key)
    ),
  CONSTRAINT idempotency_requests_hash_valid
    CHECK (octet_length(request_hash) = 32),
  CONSTRAINT idempotency_requests_status_valid
    CHECK (status IN ('in_progress', 'completed')),
  CONSTRAINT idempotency_requests_response_status_valid
    CHECK (response_status IS NULL OR response_status BETWEEN 200 AND 599),
  CONSTRAINT idempotency_requests_response_headers_valid
    CHECK (
      response_headers IS NULL
      OR (
        jsonb_typeof(response_headers) = 'object'
        AND octet_length(response_headers::text) <= 32768
      )
    ),
  CONSTRAINT idempotency_requests_response_body_valid
    CHECK (response_body IS NULL OR octet_length(response_body) <= 1048576),
  CONSTRAINT idempotency_requests_expiration_valid
    CHECK (lease_expires_at > created_at AND expires_at >= lease_expires_at),
  CONSTRAINT idempotency_requests_completion_valid
    CHECK (
      (
        status = 'in_progress'
        AND response_status IS NULL
        AND response_headers IS NULL
        AND response_body IS NULL
        AND completed_at IS NULL
      )
      OR
      (
        status = 'completed'
        AND response_status IS NOT NULL
        AND response_headers IS NOT NULL
        AND response_body IS NOT NULL
        AND completed_at IS NOT NULL
        AND completed_at <= expires_at
      )
    )
);

CREATE INDEX idx_idempotency_requests_expires_at
  ON idempotency_requests(expires_at, id);

CREATE INDEX idx_idempotency_requests_in_progress_expires_at
  ON idempotency_requests(expires_at, id)
  WHERE status = 'in_progress';

CREATE TRIGGER trg_idempotency_requests_updated_at
  BEFORE UPDATE ON idempotency_requests
  FOR EACH ROW
  EXECUTE FUNCTION aro_touch_updated_at();
