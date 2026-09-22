# Security And Privacy

ARO is cloud-persisted with local inference by default. Security decisions should preserve the split: PostgreSQL and secrets stay behind the API, while the desktop app can run local model and voice runtimes safely.

## Rules

- No remote LLM provider is allowed in the default product path. Proprietary providers are opt-in per user and only run after the user explicitly selects one of their models.
- Model endpoints must be loopback hosts such as `127.0.0.1`, `localhost`, or `::1`.
- Proprietary provider endpoints must use clean HTTPS URLs without embedded credentials, query strings, or fragments.
- Proprietary provider API keys stay in the desktop OS credential store by default. They must not be written to `settings.json`, PostgreSQL `app_settings`, browser `localStorage`, logs, or audit payloads.
- Exception (governed server cloud opt-in): an organization admin may explicitly allow server-side generation for selected providers. Then provider keys are stored in `org_provider_keys` encrypted at rest with `ARO_SECRETS_KEY` (`pgp_sym_encrypt`), readable only through the generation path, never returned by any GET/status/log. Revocation is immediate via key delete or consent toggle.
- Desktop clients never receive PostgreSQL credentials. They authenticate to the Axum API with tenant-scoped JWT access tokens and refresh tokens whose durable desktop copy lives in the OS credential store.
- Desktop clients never receive Redis URLs or credentials. Redis is a server-side API dependency only.
- PostgreSQL rows are scoped by active organization membership. Tenant-sensitive writes are guarded in repositories and foreign keys.
- Passwords use Argon2id. API keys and refresh tokens are stored as hashes.
- The active organization is cryptographically bound to the access JWT and revalidated against active membership. A caller-controlled header cannot change tenant scope.
- Refreshes and organization switches rotate the current refresh token atomically; switching requires the current access and refresh tokens, so an access token alone cannot change tenant or create a long-lived session.
- Every login creates an independent refresh-token family with a non-extendable absolute deadline. Rotations are serialized per family and every successor is capped at that deadline. Reuse of a credential that has already been rotated records the detection and atomically revokes every still-active family member. Maintenance retains then deletes complete lineages; it never purges individual predecessors early.
- Logout is serialized against rotation and revokes the complete refresh family. It prevents future refreshes but does not invalidate an already-issued access JWT before that JWT expires.
- Tenant administrators never receive invitation bearer tokens. Unknown invited e-mails are not inserted into `users` until the mailbox recipient accepts through the server-side delivery channel.
- Sensitive plugin, MCP, and client-state material is encrypted at rest with `ARO_SECRETS_KEY`.
- Redis must store only ephemeral coordination data: rate-limit counters, short lock tokens, and already-redacted outbox payloads. Do not put raw secrets, prompts, transcripts, audio paths, file contents, or raw memory content in Redis.
- Browser `localStorage` must not store business data or secrets in authenticated cloud mode.
- Tauri holds its session mutex across refresh, switch, and logout, and uses the same guarded state when installing sessions returned by primary authentication. It keeps access credentials in process memory and stores the refresh credential in the OS keyring. If a rotated successor cannot be persisted after retries, the stale predecessor is removed from the keyring and the live successor remains memory-only while the error is surfaced.
- Browser credentials are memory-only. Concurrent `401` responses share one refresh operation, and login/switch/logout/session updates are serialized so stale requests cannot overwrite a newer credential pair. Ambiguous refresh transport failures clear the session rather than replaying a possibly consumed token.
- The renderer must never validate third-party credentials by calling SaaS APIs directly. API-key material is submitted once through the Tauri bridge to the backend credential endpoint; generic plugin, MCP and hook collection payloads are always secret-free.
- The packaged desktop renderer has no general internet `connect-src` capability and no direct opener permission. External navigation goes through the Rust `open_url` command, which accepts only bounded, credential-free HTTPS URLs. Synced plugin, MCP and hook records are sanitized again when they are hydrated so legacy secret-bearing records cannot re-enter renderer state.
- Model files are not committed to Git.
- Voice runtimes and models are downloaded under `vendor/voice/` and ignored by Git.

## Sovereign AI (Zones Of Trust)

Principle: by default, no conversation content ever leaves the self-hosted server. No third-party key lives on the server. Cloud is a governed exception, never the default path.

- **Zone 1 — Device (desktop):** OS keyring keys, personal credentials, local inference. Unchanged.
- **Zone 2 — Self-hosted server / LAN:** PostgreSQL, Ollama/llama.cpp (loopback only, enforced by `ensure_loopback_url` on every local attempt including discovery probes). Serves mobile + browser clients. Secrets at rest encrypted with `ARO_SECRETS_KEY`.
- **Zone 3 — Third parties:** default-DENY. Opening requires BOTH an explicit org-admin opt-in row (`org_ai_cloud_consent`: enabled + provider allow-list + declared data residency + accepting admin + timestamp) AND a deposited encrypted provider key. Either switch off re-closes the zone immediately.
- Every server-cloud generation is audit-logged with org, provider, model and source — never content, never keys. Clients display the source (`local` / `server-cloud` / `demo`) so users always know where their words were processed.
- Mobile drafts live in the OS keystore (`flutter_secure_storage`), with one-time migration from the legacy cleartext slot and a cleartext fallback only if the keystore is unavailable (never a crash for a draft).
- Unavailable generations are never persisted as assistant messages; clients show an ephemeral notice + retry instead of fake history.
- The `Mock` provider is demo-only: clients must surface it as demo (factice) and never present its echo as model output.

## Data Controls

The product must expose:

- Conversation deletion.
- Memory reset.
- Provider visibility.
- Local-first model selection and clear remote-provider status.
- Voice runtime visibility.
- Session expiry and logout.
- Offline/read-only state when the API is unavailable.

## Voice Runtime Notes

- Whisper.cpp and Piper are executed as local binaries.
- Audio is written temporarily to the app scratch directory for transcription.
- Speech synthesis returns local WAV bytes to the frontend for playback.
- Do not add downloaded model binaries, generated WAV files, or user audio captures to Git.

## Future Hardening

- Immediate access-token revocation where required, asymmetric JWT/JWKS rotation, and user-facing device/session management. Refresh families already have a non-extendable absolute deadline and whole-lineage retention/purge.
- HttpOnly-cookie/BFF authentication before offering a persistent public browser session.
- Signed update pipeline.
- Production key management and rotation for `ARO_SECRETS_KEY`.
- Redaction layer for logs.
- Runtime permission audit for file access and shell adapters.
