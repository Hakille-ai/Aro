# Integrations Platform Plan

ARO currently has useful plugin, MCP, hook, scheduler, API key, and encrypted collection primitives. The next product step is to turn those primitives into a single integrations control plane: users should click "Connect account", approve scopes at the provider, return to ARO, and see a reliable connected account with permissions, health, reconnect, disconnect, and revocation controls.

## Current Issues

- Plugins are cataloged, configured, validated, and rendered inside `apps/desktop/src/App.svelte`.
- Manual credentials are submitted once through the Tauri bridge to the encrypted backend endpoint; unsupported providers now fail closed instead of validating against SaaS APIs in the renderer.
- The renderer still owns a large provider catalog and connection form that should move to versioned server manifests.
- `/collections/plugins` and `/plugins` coexist as generic JSON CRUD without a typed provider/account/credential model.
- MCP, hooks, and scheduler controls are useful UI foundations but are not yet backed by durable workers and a unified capability registry.

## Target Model

Use a typed domain shared between Rust and TypeScript:

- `IntegrationManifest`: versioned provider definition, auth mode, config schema, scopes, permissions, and capabilities.
- `IntegrationConnection`: an org/user installation of a manifest with status, connected account metadata, public config, granted scopes, and credential references.
- `IntegrationCredentialRef`: opaque handle to a server vault, device keyring, or external vault entry. The UI must not receive raw secrets after connection.
- `IntegrationCapability`: read or action capability that can later feed the agent tool registry.

## OAuth/OIDC Flow

1. `POST /v1/integrations/{provider}/connect/start`
   - Validate ARO session, organization access, requested scopes, provider manifest, and redirect strategy.
   - Create short-lived `state`, `nonce`, and PKCE verifier.
   - Return the provider authorization URL.

2. Desktop opens the system browser or a strict allowlisted deep link flow.

3. `GET /v1/integrations/{provider}/oauth/callback`
   - Validate `state` and PKCE.
   - Exchange authorization code for provider tokens.
   - Validate OIDC ID token when present: issuer, audience, expiry, nonce, signature/JWKS.
   - Fetch minimal profile/userinfo.
   - Store tokens only through the credential vault.

4. Desktop polls `GET /v1/integrations/connect/status?state=...` or receives a deep-link completion event.

5. Reconnect and disconnect
   - Reconnect can refresh scopes or repair expired credentials.
   - Disconnect revokes at the provider when supported, soft-deletes vault material, emits audit/outbox events, and clears cached profile data.

## Credential Rules

- No plugin token, OAuth token, service account secret, MCP env secret, or webhook secret in `localStorage`, app settings JSON, logs, audit payloads, or frontend error messages.
- Secrets are declared by manifest schema, not guessed only by field name.
- Server-side credentials need key metadata: `key_id`, `credential_id`, `organization_id`, `created_at`, `expires_at`, `last_used_at`, and `revoked_at`.
- Production should move from a single symmetric `ARO_SECRETS_KEY` model toward envelope encryption with key rotation.
- API responses return public config, profile metadata, status, scopes, and credential handles only.

## API Surface

Additive v2 endpoints should coexist with current collections until migration is complete:

- `GET /v1/integration-providers`
- `GET /v1/integrations`
- `POST /v1/integrations/{provider}/connect/start`
- `GET /v1/integrations/{provider}/oauth/callback`
- `GET /v1/integrations/connect/status`
- `POST /v1/integrations/{provider}/api-key`
- `POST /v1/integrations/{connection_id}/validate`
- `PATCH /v1/integrations/{connection_id}`
- `DELETE /v1/integrations/{connection_id}`

The existing `/v1/plugins`, `/v1/mcp`, `/v1/hooks`, and `/v1/scheduled-tasks` routes remain compatibility facades during migration; unversioned aliases are deprecated.

## Migration Order

1. Add shared contracts and frontend registry helpers.
2. Extract plugin catalog data and mappers out of `App.svelte`.
3. Secret persistence and direct renderer validation are now removed; keep regression tests around one-shot Tauri/API submission.
4. The `/v1/integrations` API and store tables are present behind feature flags; finish provider validation and lifecycle workers.
5. Dual-write old plugin rows and new integration connections.
6. Shadow-read and compare old/new payloads.
7. Switch UI reads to v2 once metrics are clean.
8. Connect integration capabilities to the agent tool registry, initially read-only.
9. Add durable workers for webhook delivery, scheduled runs, MCP sessions, and sync jobs.

## Product UX

The target settings screen should feel like a calm account center:

- Primary action: `Connect account`.
- Connected card: account avatar/name, provider health, granted permissions, last checked time.
- Actions: `Manage`, `Reconnect`, `Disconnect`, `Revoke local data`.
- Errors: inline and accessible, not browser `alert()`.
- Accessibility: real labels, focus trap, Escape to close, `role="dialog"`, `aria-modal`, `role="status"` for progress, `role="alert"` for failures.

## Feature Flags

Recommended flags:

- `integrations.registry_v2`
- `integrations.api_v2`
- `integrations.dual_write`
- `integrations.read_v2`
- `integrations.oauth_connect`
- `integrations.execute_tools`
- `mcp.worker`
- `hooks.worker`
- `scheduler.worker`

Keep kill switches for all third-party network execution, all write actions, and webhook delivery.
