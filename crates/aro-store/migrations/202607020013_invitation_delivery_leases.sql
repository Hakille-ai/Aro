-- Durable, fenced delivery state for invitation mail. SMTP is at-least-once: a worker can crash
-- after relay acceptance and before acknowledgement, so every retry uses the same deterministic
-- Message-ID while database writes remain strictly lease-fenced.
UPDATE organization_invitations
SET delivery_status = 'pending',
    delivery_last_error = left(delivery_last_error, 500),
    updated_at = now()
WHERE delivery_status = 'processing';

ALTER TABLE organization_invitations
  ADD COLUMN delivery_available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  ADD COLUMN delivery_lease_token UUID,
  ADD COLUMN delivery_lease_until TIMESTAMPTZ,
  ADD COLUMN delivery_last_attempt_at TIMESTAMPTZ,
  ADD CONSTRAINT organization_invitations_delivery_error_bounded
    CHECK (delivery_last_error IS NULL OR char_length(delivery_last_error) <= 500),
  ADD CONSTRAINT organization_invitations_delivery_lease_complete
    CHECK (
      (delivery_lease_token IS NULL AND delivery_lease_until IS NULL)
      OR (delivery_lease_token IS NOT NULL AND delivery_lease_until IS NOT NULL)
    ),
  ADD CONSTRAINT organization_invitations_processing_has_lease
    CHECK (
      (delivery_status = 'processing')
      = (delivery_lease_token IS NOT NULL AND delivery_lease_until IS NOT NULL)
    );

-- Final and administratively-closed states no longer need a decryptable bearer credential.
-- The one-way token hash remains available for a recipient whose relay-accepted message races
-- acknowledgement, but the worker copy is destroyed as soon as it cannot be sent again.
UPDATE organization_invitations
SET delivery_status = CASE
      WHEN accepted_at IS NOT NULL THEN 'delivered'
      WHEN revoked_at IS NOT NULL THEN 'failed'
      ELSE delivery_status
    END,
    delivered_at = CASE
      WHEN accepted_at IS NOT NULL OR delivery_status = 'delivered'
        THEN COALESCE(delivered_at, accepted_at, updated_at)
      ELSE delivered_at
    END,
    delivery_last_error = CASE
      WHEN accepted_at IS NOT NULL OR delivery_status = 'delivered' THEN NULL
      WHEN revoked_at IS NOT NULL THEN COALESCE(delivery_last_error, 'invitation_revoked')
      ELSE delivery_last_error
    END,
    delivery_token_encrypted = NULL,
    delivery_lease_token = NULL,
    delivery_lease_until = NULL,
    updated_at = now()
WHERE accepted_at IS NOT NULL
   OR revoked_at IS NOT NULL
   OR delivery_status IN ('delivered', 'failed');

-- A legacy/corrupt active row without encrypted delivery material can never be claimed. Surface
-- that terminal state instead of leaving an invitation permanently displayed as pending.
UPDATE organization_invitations
SET delivery_status = 'failed',
    delivery_last_error = 'delivery_token_unavailable',
    updated_at = now()
WHERE accepted_at IS NULL
  AND revoked_at IS NULL
  AND delivery_status = 'pending'
  AND delivery_token_encrypted IS NULL;

ALTER TABLE organization_invitations
  ADD CONSTRAINT organization_invitations_delivery_token_lifecycle
    CHECK (
      (delivery_status IN ('pending', 'processing'))
      = (delivery_token_encrypted IS NOT NULL)
    );

DROP INDEX idx_organization_invitations_delivery_pending;

CREATE INDEX idx_organization_invitations_delivery_claim
  ON organization_invitations (delivery_available_at, created_at, id)
  WHERE delivery_status IN ('pending', 'processing')
    AND accepted_at IS NULL
    AND revoked_at IS NULL
    AND delivery_token_encrypted IS NOT NULL;

DO $invitation_delivery_worker_grant$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'aro_worker') THEN
    EXECUTE 'GRANT SELECT (
      id, organization_id, invited_email, invited_name, invited_role,
      expires_at, accepted_at, revoked_at, created_at,
      delivery_token_encrypted, delivery_status, delivered_at, delivery_attempts,
      delivery_last_error, delivery_available_at, delivery_lease_token,
      delivery_lease_until, delivery_last_attempt_at
    ) ON TABLE public.organization_invitations TO aro_worker';
    EXECUTE 'GRANT UPDATE (
      delivery_status, delivered_at, delivery_attempts, delivery_last_error,
      delivery_token_encrypted, delivery_available_at, delivery_lease_token,
      delivery_lease_until, delivery_last_attempt_at, updated_at
    ) ON TABLE public.organization_invitations TO aro_worker';
    -- Claims include the tenant display name in the email. Keep this narrower than a table-wide
    -- grant so the delivery worker cannot read unrelated organization metadata.
    EXECUTE 'GRANT SELECT (id, name, deleted_at) ON TABLE public.organizations TO aro_worker';
  END IF;
END
$invitation_delivery_worker_grant$;
