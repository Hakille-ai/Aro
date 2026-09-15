# Production Readiness Gate

ARO must not be presented as generally available until every item in this document has a named owner, evidence, and a release sign-off. The API now fails closed for several dangerous configurations; these are safeguards, not a substitute for the remaining work.

## Enforced at startup

- Production requires `ARO_PUBLIC_BASE_URL` and explicit `ARO_CORS_ALLOWED_ORIGINS`.
- Production rejects `ARO_SIGNUP_MODE=open`.
- Production requires `ARO_METRICS_TOKEN`; `/metrics` requires that token as a bearer credential.
- When the Files API is enabled, production requires the S3 backend. A local/container filesystem is not accepted.
- An explicit S3 endpoint must use HTTPS in production.
- `X-Forwarded-For` and `X-Real-IP` are ignored unless `ARO_TRUST_PROXY_HEADERS=true`. Set it only when the API is reachable exclusively through a trusted proxy that rewrites those headers.
- The integrations API and OAuth connections are disabled unless their separate feature flags are enabled. OAuth never falls back to mock credentials, and the callback URI is built only from `ARO_PUBLIC_BASE_URL` plus the server provider manifest.
- Browser sessions, plugin credential fields, hook secrets, and MCP environment values are not persisted in renderer `localStorage`. Existing local values are purged on load. A full browser deployment still needs an HttpOnly-cookie/BFF session design before it can be offered as a public web client.

## Release blockers still in progress

- Identity: password reset, session/device management endpoints, MFA/SSO where required, and immediate access-token revocation where policy requires it. Refresh tokens now rotate inside durable families with a non-extendable absolute expiry; reuse atomically revokes the family, and the worker retains then purges whole lineages without destroying predecessor evidence. Invitations no longer reserve unknown identities or expose bearer tokens to tenant admins; configure an audited server-side delivery worker before enabling them. Invite acceptance marks the recipient address verified.
- Authorization: resource ownership and sharing policy, enforced ACLs, database RLS, and permission regression tests.
- Agent execution: durable job queue, leases, idempotency, cancellation, retries, DLQ, explicit tool approval, egress proxy, and execution audit trail.
- Files: antivirus/quarantine, extraction/indexing workers, tenant quotas, retention/deletion jobs, and multi-replica upload/download tests.
- Integrations: OIDC token/JWKS validation, provider-specific refresh/revocation, vault/KMS key rotation, and removal of all renderer-side secret persistence.
- API contracts: versioned schemas, pagination, idempotency keys, optimistic concurrency, generated client, and compatibility tests.
- Operations: private metrics endpoint, graceful shutdown, request/concurrency limits, backups with restore drills, SLOs, alerts, supply-chain scans/SBOM/signing, pinned images, and hardened Kubernetes/network policies.

## Controls added during backend hardening

- Access JWTs bind the user and active organization, and the API revalidates active membership for that signed tenant scope. Organization switching cannot be requested through a tenant header: it requires the current access and refresh tokens and rotates the refresh credential atomically.
- Each primary login creates an independent refresh-token family with an absolute deadline. PostgreSQL serializes rotations per family, caps every successor at that deadline, records reuse of a rotated predecessor, and revokes every active family member on replay. Logout uses the same family lock and revokes the whole family so a concurrent rotation cannot leave a refresh successor alive after successful logout. Bounded worker maintenance deletes only complete families after the configured retention window.
- Tauri holds its session mutex across refresh, switch, and logout, guards installation of primary-auth sessions, keeps the access token/session in memory, and persists refresh credentials in the OS keyring. A failed successor write is retried, then the stale predecessor is removed and the live successor remains memory-only while the error is reported.
- Browser tokens remain memory-only. A single-flight refresh and serialized session mutation queue coalesce concurrent `401` recovery and prevent stale completions from replacing a newer session; ambiguous refresh failures clear the session rather than replay a possibly consumed token. Persistent public browser authentication remains blocked on an HttpOnly-cookie/BFF design.
- Production database identities are separated: `DATABASE_URL` uses `aro_app`, `ARO_WORKER_DATABASE_URL` uses `aro_worker`, and `ARO_MIGRATION_DATABASE_URL` uses the table-owning migrator. API and worker startup reject `SUPERUSER`, `BYPASSRLS`, disabled `row_security`, and ownership of application tables.
- Compose and Helm bootstrap the two restricted runtime roles with `NOINHERIT`; migration grants are explicit, so future tables are inaccessible until reviewed.
- API and worker perform a graceful shutdown; HTTP concurrency and ordinary request latency are bounded with `ARO_HTTP_MAX_CONCURRENCY` and `ARO_HTTP_REQUEST_TIMEOUT_SECONDS`.
- Anonymous rate limits use the TCP peer address unless trusted proxy headers are explicitly enabled.
- Direct agent tool calls are disabled by default (`ARO_AGENT_DIRECT_TOOL_EXECUTION=false`). Assistant streams never perform implicit web egress; an explicit direct call requires a persisted permission profile and an allowlist.
- Error responses use Problem Details and stable machine-readable codes while retaining the legacy `error` member for desktop compatibility.
- `/v1` is the canonical API prefix. Unversioned compatibility routes remain available with `Deprecation: true` and a successor-version link while shipped clients migrate.
- Local-result persistence supports durable idempotency keys with canonical request hashes, short execution leases, conflict detection and exact response replay. The same pattern still has to be extended transactionally to every externally visible command.
- Idempotency retention cleanup is exposed to `aro_worker` through a bounded `SECURITY DEFINER` maintenance function. The API role has no arbitrary delete permission on idempotency records.
- Renderer-side API-key validation and simulated MCP success are disabled. Sensitive integration configuration is submitted once to the encrypted backend integration endpoint and is removed from renderer persistence/cloud collection payloads.
- The Tauri renderer is denied direct opener access and external network connections by capability/CSP. External URLs cross a credential-free HTTPS validator in Rust, and legacy synchronized plugin/MCP/hook data is sanitized on hydration as well as on write.
- The Helm chart defaults to externally provisioned secrets, injects only workload-specific keys, persists ClamAV signatures, and supplies probes, requests/limits, PDB/HPA support, topology spreading, NetworkPolicies, seccomp/capability drops and optional image digests. Bundled PostgreSQL, Redis and ClamAV remain single-instance components and are not the GA high-availability data plane.
- Production Compose uses separate API/worker/migrator environment files, requires digest-pinned images, makes application filesystems read-only, drops Linux capabilities and bounds CPU, memory, PIDs and local log growth. CI verifies the secret boundaries through rendered Compose configuration.
- CI uses the lockfiles for Rust/Node builds, lints and renders Helm with weak-secret negative tests, scans the API image, and publishes BuildKit SBOM/provenance attestations with release images. GitHub Action commit pinning and cryptographic image signing remain release tasks.

These are containment controls, not proof of GA readiness. Durable workers, RLS, full identity, scanning, tracing and the remaining release blockers above still apply.

## Product truthfulness

The following capabilities are not production-ready until their durable workers and security controls exist: cloud model execution, scheduled tasks, hooks, MCP execution, file indexing, and third-party agent actions. They must remain hidden or explicitly marked unavailable rather than simulated. Uploaded files remain `pending` and cannot be downloaded, attached, or passed to a model before the leased ClamAV worker explicitly approves them. Production requires `ARO_FILE_SCAN_REQUIRED=true` and a private `ARO_CLAMAV_ADDRESS`; a detection is quarantined and scanner failures have bounded retries.

The first authorization slice is documented in [AUTHORIZATION_MODEL.md](AUTHORIZATION_MODEL.md): personal conversations, agents, contexts and files are private by default. Explicit sharing and PostgreSQL RLS remain release blockers.

The Redis outbox publisher now uses PostgreSQL leases, lease tokens and retries. This prevents concurrent workers from acknowledging each other’s work. The same ownership pattern now protects file scanning; durable agent execution, file indexing, hooks and scheduling still require their own job handlers and release gates.

Migration `202607020012_agent_execution_queue.sql` is an expand-only, dormant schema reservation. It deliberately mutates no existing run and grants neither the API nor the worker access to the new tables. It is not a queue cutover: runtime grants must remain behind a later migration that exposes only fenced claim/renew/retry/complete functions after the full worker path and rolling-version tests exist.
