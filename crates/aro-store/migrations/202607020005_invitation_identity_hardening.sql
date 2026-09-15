-- Invitations are bearer credentials proving control of the invited mailbox. A tenant
-- administrator must never be able to reserve a global user identity or choose its password.
-- Revoke every pre-hardening pending invitation because those tokens were disclosed to the
-- issuing administrator by the old API contract.
UPDATE organization_invitations
SET revoked_at = COALESCE(revoked_at, now()),
    updated_at = now()
WHERE accepted_at IS NULL
  AND revoked_at IS NULL;

-- Remove placeholder identities created by the vulnerable pre-hardening flow. They never owned
-- an active membership and their disclosed invitation credentials were just revoked above.
CREATE TEMP TABLE aro_revoked_invitation_placeholders ON COMMIT DROP AS
SELECT DISTINCT i.invited_user_id
FROM organization_invitations i
WHERE i.created_user = true
  AND i.accepted_at IS NULL
  AND NOT EXISTS (
    SELECT 1
    FROM organization_invitations accepted
    WHERE accepted.invited_user_id = i.invited_user_id
      AND accepted.accepted_at IS NOT NULL
  )
  AND NOT EXISTS (
    SELECT 1
    FROM memberships retained
    WHERE retained.user_id = i.invited_user_id
      AND retained.status <> 'invited'
      AND retained.deleted_at IS NULL
  )
  AND NOT EXISTS (
    SELECT 1 FROM refresh_tokens session WHERE session.user_id = i.invited_user_id
  );

DELETE FROM organization_invitations i
USING aro_revoked_invitation_placeholders doomed
WHERE i.invited_user_id = doomed.invited_user_id
  AND i.accepted_at IS NULL;

DELETE FROM memberships m
USING aro_revoked_invitation_placeholders doomed
WHERE m.user_id = doomed.invited_user_id
  AND m.status = 'invited';

DELETE FROM users u
USING aro_revoked_invitation_placeholders doomed
WHERE u.id = doomed.invited_user_id
  AND NOT EXISTS (
    SELECT 1 FROM memberships remaining WHERE remaining.user_id = u.id
  );

ALTER TABLE users
  ADD COLUMN email_verified_at TIMESTAMPTZ;

DO $email_canonicalization$
BEGIN
  IF EXISTS (
    SELECT 1
    FROM users
    GROUP BY lower(btrim(email))
    HAVING COUNT(*) > 1
  ) THEN
    RAISE EXCEPTION 'cannot canonicalize users.email: duplicate case/whitespace variants exist';
  END IF;
END
$email_canonicalization$;

UPDATE users
SET email = lower(btrim(email)), updated_at = now()
WHERE email <> lower(btrim(email));

ALTER TABLE users
  ADD CONSTRAINT users_email_canonical
    CHECK (email = lower(btrim(email)) AND position('@' IN email) > 1);

ALTER TABLE organization_invitations
  ADD COLUMN invited_email TEXT,
  ADD COLUMN invited_name TEXT,
  ADD COLUMN invited_role TEXT,
  ADD COLUMN account_existed_at_issue BOOLEAN,
  ADD COLUMN delivery_token_encrypted BYTEA,
  ADD COLUMN delivery_status TEXT NOT NULL DEFAULT 'pending',
  ADD COLUMN delivered_at TIMESTAMPTZ,
  ADD COLUMN delivery_attempts INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN delivery_last_error TEXT;

UPDATE organization_invitations i
SET invited_email = lower(u.email),
    invited_name = u.name,
    invited_role = m.role,
    account_existed_at_issue = NOT i.created_user
FROM users u, memberships m
WHERE u.id = i.invited_user_id
  AND m.id = i.membership_id;

ALTER TABLE organization_invitations
  DROP CONSTRAINT organization_invitations_membership_unique,
  ALTER COLUMN invited_email SET NOT NULL,
  ALTER COLUMN invited_name SET NOT NULL,
  ALTER COLUMN invited_role SET NOT NULL,
  ALTER COLUMN account_existed_at_issue SET NOT NULL,
  ALTER COLUMN membership_id DROP NOT NULL,
  ALTER COLUMN invited_user_id DROP NOT NULL,
  DROP COLUMN created_user;

ALTER TABLE organization_invitations
  ADD CONSTRAINT organization_invitations_email_normalized
    CHECK (invited_email = lower(btrim(invited_email)) AND position('@' IN invited_email) > 1),
  ADD CONSTRAINT organization_invitations_role_allowed
    CHECK (invited_role IN ('admin', 'manager', 'member', 'guest')),
  ADD CONSTRAINT organization_invitations_delivery_status_allowed
    CHECK (delivery_status IN ('pending', 'processing', 'delivered', 'failed')),
  ADD CONSTRAINT organization_invitations_delivery_attempts_nonnegative
    CHECK (delivery_attempts >= 0);

CREATE UNIQUE INDEX idx_organization_invitations_org_email_active
  ON organization_invitations (organization_id, invited_email)
  WHERE accepted_at IS NULL AND revoked_at IS NULL;

CREATE INDEX idx_organization_invitations_membership
  ON organization_invitations (membership_id)
  WHERE membership_id IS NOT NULL;

CREATE INDEX idx_organization_invitations_delivery_pending
  ON organization_invitations (created_at)
  WHERE delivery_status = 'pending' AND accepted_at IS NULL AND revoked_at IS NULL;
