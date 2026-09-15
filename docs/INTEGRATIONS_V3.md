# Integrations v3

Integrations v3 makes PostgreSQL the durable source of truth for connector manifests,
installations, authorization attempts, credential versions, capability grants, health, jobs and
webhooks. Redis may wake workers or cache catalog responses, but no durable state depends on it.

## Safe rollout

All switches default to `false`:

- `ARO_INTEGRATIONS_API_V3` exposes the catalog, installation, credential and authorization APIs.
- `ARO_INTEGRATIONS_THIRD_PARTY_NETWORK` permits certified connectors to contact allowlisted hosts.
- `ARO_INTEGRATIONS_WORKERS` leases integration jobs from PostgreSQL.
- `ARO_INTEGRATIONS_WEBHOOKS` enables webhook ingress after connector verification is installed.
- `ARO_INTEGRATIONS_EXECUTE` permits integrations to be used by runtime capabilities.

Keep legacy OAuth disabled with `ARO_INTEGRATIONS_OAUTH_CONNECT=false`. Enable v3 per provider only
after its immutable manifest has `certification_status='certified'`, its client profile contains the
exact server callback URI, and the connector certification harness passes against provider-owned
development fixtures.

The canonical callback is:

`{ARO_PUBLIC_BASE_URL}/v1/integration-oauth/callback/{providerId}`

Client-provided callback URLs are never accepted. Client profiles store only a public client ID and
an opaque secret reference such as `env:ARO_INTEGRATION_GOOGLE_WORKSPACE_CLIENT_SECRET`; they never
store the client secret itself.

## Secret providers

Production v3 refuses the development `local-key` provider. Use one of:

- SaaS: `ARO_SECRET_PROVIDER=aws-kms` and `ARO_KMS_KEY_ID`; AWS credentials come from the standard
  workload/instance identity chain.
- On-premise: `ARO_SECRET_PROVIDER=vault-transit`, `ARO_VAULT_ADDR`,
  `ARO_VAULT_TRANSIT_MOUNT`, `ARO_VAULT_TRANSIT_KEY` and a workload-provided `ARO_VAULT_TOKEN`.

Every credential version receives a random DEK and AES-256-GCM nonce. AAD binds organization,
installation, provider, version and environment. PostgreSQL receives ciphertext, the wrapped DEK,
nonce and non-secret metadata only. OAuth state is stored as a SHA-256 digest; nonce and PKCE
verifier are placed in a separate encrypted transaction envelope.

## Public resources

- `GET /v1/integration-catalog[/{providerId}]`
- `POST /v1/integration-catalog/{providerId}/authorization-attempts`
- `GET|DELETE /v1/integration-authorization-attempts/{attemptId}`
- `POST /v1/integration-catalog/{providerId}/credentials`
- `GET /v1/integration-installations`
- `GET|PATCH /v1/integration-installations/{installationId}`
- `POST /v1/integration-installations/{installationId}/reauthorize`
- `POST /v1/integration-installations/{installationId}/health-check`
- `POST /v1/integration-installations/{installationId}/disconnect`
- `GET /v1/integration-installations/{installationId}/audit`

Secret-bearing and authorization-start endpoints require `Idempotency-Key` and return
`Cache-Control: no-store`. Responses never echo credentials. Browser callbacks atomically claim a
pending attempt using a fencing token; duplicate callbacks return the existing terminal state.

## Database and workers

API transactions set `aro.organization_id` and `aro.actor_id`. RLS policies on every tenant table
enforce the organization boundary even if an application predicate is omitted. The unauthenticated
provider callback can only resolve a high-entropy state digest through the narrow
`aro_claim_integration_authorization_attempt` security-definer function.

Jobs use `FOR UPDATE SKIP LOCKED`, leases, fencing tokens, exponential retry with deterministic
jitter and dead-letter status. A disconnect blocks use immediately, then a revoke job performs
cleanup and cryptographic erasure. Provider-specific refresh, remote health and webhook jobs must
remain disabled until the corresponding certified connector implementation is registered.

## Required provider activation checks

1. State, nonce, PKCE S256, exact callback, mix-up and replay tests pass.
2. OIDC signature, issuer, audience/`azp`, time, nonce and optional `at_hash` checks pass, including
   JWKS rollover.
3. Scope reduction is surfaced as `insufficient_scope` and never creates an active installation.
4. Refresh-token rotation, duplicate refresh and crash-after-token tests pass.
5. Revocation and webhook signature/timestamp/replay tests pass.
6. Database/API/log/browser snapshots contain no plaintext credential.
7. Cross-tenant direct SQL tests under `aro_app` and worker fencing tests pass.
8. Rollout and rollback are rehearsed without writing a v3 secret back to the legacy schema.
