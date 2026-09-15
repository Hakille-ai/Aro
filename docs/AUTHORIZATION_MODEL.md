# Authorization Model

ARO treats an organization as a billing and membership boundary, not as a blanket data-sharing boundary.

## Identity and membership lifecycle

An `invited` membership is not an authorization grant and cannot create an authenticated principal, issue a refresh token, or appear among the organizations available for switching. Each invitation is a single-use, expiring bearer token. PostgreSQL stores its lookup hash and a separately encrypted delivery copy.

- Issuing an invitation for an unknown e-mail creates neither a global user nor a membership. The identity is created only after the mailbox recipient presents the server-delivered token to `POST /v1/auth/invitations/accept` and chooses a password.
- If the recipient registers before accepting, the public acceptance path refuses to modify that account. The authenticated recipient uses `POST /v1/auth/invitations/accept-existing` with both the invitation and current refresh token.
- Tenant administrators never receive the raw token or acceptance URL. `ARO_INVITATION_DELIVERY_ENABLED` remains false until a trusted server-side delivery worker is attached; the API fails closed while delivery is unavailable.
- Authorized organization owners/admins can page safe invitation metadata through `GET /v1/invitations` and revoke an active invitation through `DELETE /v1/invitations/{id}`. Revocation atomically destroys the encrypted delivery token and removes any provisional `invited` membership without exposing credential material.
- Changing organization also requires and atomically rotates the current refresh token. A short-lived access token alone can never mint a long-lived session.

## Session and tenant boundary

The active organization is a signed `organizationId` claim in the access JWT. The API derives its `AuthContext` from that claim, validates that the subject still has an active membership, and does not accept a caller-controlled tenant header as a substitute. Consequently, an access token issued for organization A cannot be reused against organization B.

Changing the active organization is a session transition, not a request-level override. `POST /v1/auth/switch-organization` requires both the current access token and its refresh token, verifies membership in the target organization, and atomically returns a new access/refresh pair scoped to that organization. The predecessor refresh credential is revoked as part of the same transaction.

Every primary login starts an independent durable refresh-token family. Normal refresh, organization switch, and authenticated invitation acceptance keep the family identifier while rotating the bearer credential. Rotations are serialized by locking the family row in PostgreSQL. Reuse of a credential already marked as rotated records the replay on both the credential and family, then revokes every active token in that family atomically.

Logout also locks the family and revokes the complete family, including a successor created by a concurrent rotation before logout obtains the lock. A successful logout therefore prevents that device session from refreshing again. It does not revoke an access JWT already issued; that JWT remains valid until its short expiry unless another authorization check, such as membership validation, rejects it.

Session lifetime is bounded twice: `ARO_REFRESH_TOKEN_DAYS` is the rolling credential lifetime and `ARO_REFRESH_TOKEN_FAMILY_DAYS` is the non-extendable absolute login lifetime. A successor expires at the earlier of those two limits. The worker purges whole expired or revoked lineages only after `ARO_REFRESH_TOKEN_FAMILY_RETENTION_DAYS`; individual predecessors are never purged while the family is retained, preserving replay detection. User-facing session/device inventory and selective revocation are still required before GA.

## Client session custody

- Tauri keeps the access token and current live session in process memory and persists the refresh token only in the operating-system credential store. Refresh, switch, and logout hold a process mutex across the session transition; installing a session returned by login, registration, or invitation acceptance updates the same guarded state.
- After server-side rotation, Tauri retries writing the successor refresh token to the credential store. If persistence still fails, it clears the stale persisted predecessor, retains the valid successor only in memory for the running process, and surfaces the durability error to the caller.
- The browser client keeps both access and refresh tokens in memory, purges legacy token keys from `localStorage`, and does not offer a durable browser session. A single-flight refresh mutex coalesces concurrent `401` responses onto one rotation and serialized compare-before-apply checks prevent an older request from overwriting a newer login, switch, or logout.
- If a browser refresh has an ambiguous network failure, the in-memory session is cleared instead of retrying the predecessor: the server may already have consumed it, and replay would correctly trigger family revocation. A public browser deployment still requires an HttpOnly-cookie/BFF design.

## Current private-by-default rule

Conversations, their messages, agent lanes, agent runs, steps, artifacts, context, permission profiles, and files belong to the user that created them. An active organization member may not read, modify, delete, attach to, or execute work against another member's private resource merely by knowing its identifier.

Organization roles still govern organization-wide administration: membership, organization settings, API keys, and future shared-resource administration. They do not bypass private conversation or agent data.

## Sharing is an explicit future capability

No implicit shared workspace is implemented. Before adding shared conversations or agents, the backend must introduce a typed resource-ACL model with these properties:

- an explicit owner, audience (user/team/organization), permission, grantor, expiry and revocation state;
- atomic authorization checks for every query and mutation;
- audit events for grants, revocations and privileged reads;
- database RLS policies backed by a per-request trusted user and organization context;
- migration and compatibility tests proving no cross-user exposure.

Until those controls exist, private data remains private by default.
