# ARO Backend Runbook

This runbook is the shortest path to run and verify the PostgreSQL-backed ARO stack locally. Redis is used for API coordination, rate limiting, short locks, and outbox fanout; PostgreSQL remains the source of truth.

## 1. Start PostgreSQL And Redis

```powershell
docker compose up -d postgres redis
docker exec aro-postgres pg_isready -U aro -d aro
docker exec aro-redis redis-cli ping
```

Expected result:

```text
/var/run/postgresql:5432 - accepting connections
PONG
```

## 2. Start the API

```powershell
$env:DATABASE_URL = "postgres://aro:aro@127.0.0.1:5432/aro"
$env:ARO_REDIS_URL = "redis://127.0.0.1:6379/0"
$env:ARO_JWT_SECRET = "dev-jwt-secret-0123456789-abcdefghijklmnopqrstuvwxyz"
$env:ARO_SECRETS_KEY = "dev-secrets-key-0123456789-abcdefghijklmnopqrstuvwxyz"
$env:ARO_API_BIND = "127.0.0.1:8710"
$env:ARO_CORS_ALLOWED_ORIGINS = "http://localhost:1420,http://127.0.0.1:1420,http://tauri.localhost,https://tauri.localhost,tauri://localhost"
$env:ARO_HTTP_MAX_BODY_BYTES = "1048576"
$env:ARO_HTTP_REQUEST_TIMEOUT_SECONDS = "30"
$env:ARO_REFRESH_TOKEN_DAYS = "30"
$env:ARO_REFRESH_TOKEN_FAMILY_DAYS = "90"
cargo run -p aro-api
```

Use random 32+ character secrets outside local development. Startup rejects placeholder values such as `replace-with` and `change-me`.
For production, set `ARO_CORS_ALLOWED_ORIGINS` to explicit HTTPS desktop/web origins only. The API rejects wildcard CORS and rejects origin values with paths, credentials, query strings, or fragments.

`ARO_FILE_SCAN_BATCH_SIZE` limits the amount of scan work attempted in one worker cycle. It is not a pre-leased batch: the worker claims each file immediately before scanning it, so slow earlier scans cannot consume the lease of files still waiting.

Health check:

```powershell
Invoke-RestMethod http://127.0.0.1:8710/health
Invoke-RestMethod http://127.0.0.1:8710/live
Invoke-RestMethod http://127.0.0.1:8710/ready
```

Expected result:

```json
{ "status": "ok" }
```

`/live` confirms the process is running. `/ready` checks PostgreSQL, embedded SQLx migrations, and Redis when configured. `/metrics` exposes a small Prometheus-compatible baseline including Redis readiness and rate-limit status.

## 2b. Local AI Engines (Mobile + Browser Chat)

Mobile and browser clients generate exclusively server-side, so the server needs a local engine. Nothing leaves the machine (sovereign by default).

```powershell
# Install Ollama from https://ollama.com/download, then:
ollama pull gemma3:1b
curl.exe -s http://127.0.0.1:11434/api/tags
```

Expected: a JSON list containing a chat model (embedding-only models such as `nomic-embed-text` do not count). The API honors `ARO_OLLAMA_ENDPOINT` (default `http://127.0.0.1:11434`) and `ARO_LLAMA_CPP_ENDPOINT` (default `http://127.0.0.1:8080`, OpenAI-compatible `/v1/models`) — loopback only.

Server generation order for `/assistant/stream` (no explicit model requested): explicit local override, active local provider, same model id found on local Ollama, first local Ollama chat model, llama.cpp model, then server cloud (only under org opt-in), otherwise an honest actionable error (never persisted as an assistant message).

Explicit selection (`modelId` + `provider`, sent per message by web and mobile composers) is STRICT: the server serves exactly that model (local engine or consented server cloud key) or fails honestly naming it — never a silent substitution. Thin clients send `promptScope: "personal"` so the server completes the prompt like the desktop harness (mode instructions + recalled memories); desktop web sends `"full"` (pre-compiled prompt used as-is).

Check what a client would see (needs a user JWT in `Authorization`):

```powershell
Invoke-RestMethod http://127.0.0.1:8710/v1/assistant/status -Headers @{ Authorization = "Bearer <access-token>" }
```

`canGenerate: true` with `source: local` means mobile and browser chat work.

## 3. Start the Desktop Client

In a second terminal:

```powershell
$env:ARO_API_BASE_URL = "http://127.0.0.1:8710"
npm run desktop:dev
```

The first screen should be the ARO Cloud login/register gate. Registering creates a user, organization, owner membership, default settings, default preferences, and the root of an independent refresh-token family. In Tauri, the access token/session is kept in process memory and the refresh token is stored in the OS credential store; refresh, switch, and logout hold the session mutex across their transitions. If a rotated successor cannot be persisted after bounded retries, ARO removes the stale keyring predecessor, retains the successor only for the running process, and reports the error.

Web-debug credentials are memory-only: legacy token keys are purged from browser `localStorage`. Concurrent `401` responses share one refresh rotation and session-changing operations are serialized. Closing/reloading the page signs the user out by design; do not treat this mode as a persistent public browser session.
Tauri accepts `http://` only for loopback API URLs. Remote API URLs must use `https://` and must not include credentials, query strings, or fragments.
`ARO_TRUSTED_WORKSPACE_ROOT` is optional. If it is not set, the desktop app does not create a trusted workspace permission profile automatically.

For quick web-only UI debugging, you can run:

```powershell
npm run dev:web
```

Then open `http://127.0.0.1:1420`.

## 4. Run the API Smoke Test

With PostgreSQL, Redis, and the API running:

```powershell
npm run doctor
npm run smoke:api
npm run smoke:redis
```

Equivalent direct command:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\smoke-api.ps1
powershell -ExecutionPolicy Bypass -File scripts\smoke-redis.ps1
```

The script verifies:

- `/health`
- `/auth/register`
- `/bootstrap`
- organization creation and switch
- preferences persistence
- memberships
- API key one-time secret behavior
- memories and teams
- assistant SSE stream and message persistence
- admin event stats for audit/outbox through the API
- optional direct audit/outbox comparison with the local Docker database

The Redis smoke verifies the local Redis container, `/ready`, `/metrics`, and the configured outbox stream key.

An organization switch must send both the current access token and refresh token. The response replaces the complete session with a new access/refresh pair whose signed tenant scope is the selected organization. Reusing the predecessor is treated as replay and revokes that login family. Logout accepts the current refresh token and revokes the complete family; an already-issued access JWT can remain usable until its short expiry.

`ARO_REFRESH_TOKEN_DAYS` bounds one rotating credential. `ARO_REFRESH_TOKEN_FAMILY_DAYS` is the absolute login deadline and must be greater than or equal to it. The worker calls the restricted family-maintenance function with `ARO_REFRESH_TOKEN_FAMILY_PURGE_BATCH` and retains expired/revoked lineages for `ARO_REFRESH_TOKEN_FAMILY_RETENTION_DAYS` before deleting the family and all of its token rows.

The refresh-family rollout is intentionally split into expand, batched online backfill, short FK metadata, and finalize migrations. Its three indexes are created while the new family table is still empty. The backfill captures a fixed start snapshot, then uses one top-level procedure call that commits every 1,000 families, logs progress, and checkpoints the last committed family so an interrupted Job resumes instead of rescanning the same prefix or chasing new logins indefinitely. The historical `family_id` default plus the insert/reuse compatibility triggers must remain for at least one full release and through the rollback window: they let an older replica keep inserting tokens, serialize inserts against family revocation, bridge token-only reuse detection, and enforce the absolute deadline. Remove them only in a later contract migration after proving that no previous-version replica can return.

All ARO database pools force `READ COMMITTED`, and production startup rejects another isolation level. The mixed-version refresh bridges rely on each statement observing newly committed descendants after lock waits; do not override these auth transactions to `REPEATABLE READ` or `SERIALIZABLE` during the compatibility window. A legacy token-first transaction and a new family-first transaction can still deadlock safely; PostgreSQL aborts one participant, so monitor SQLSTATE `40P01` and drain old replicas promptly.

## 5. Voice Setup And Smoke

Voice is local and device-specific. Run this once per Windows machine that needs wake-word metadata, microphone transcription, or spoken responses:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-voice.ps1 -RunSmoke
```

Download or refresh voice model slots without writing ARO settings:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -SkipText -Voice
```

For diagnostics after setup:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\doctor.ps1 -VoiceSmoke
powershell -ExecutionPolicy Bypass -File scripts\smoke-voice.ps1
```

Acceptance for the local voice setup pass:

- `setup-voice.ps1` validates the `wake-word`, `stt-whisper`, and `tts-piper` local model slots.
- `setup-voice.ps1` validates the Whisper and Piper binaries, models, required DLLs, Piper voice config, and `espeak-ng-data`.
- `smoke-voice.ps1` synthesizes a short fixture with Piper and transcribes it with Whisper.
- The smoke JSON reports `ok: true`, a generated WAV larger than 12 KB, redaction flags, repo-relative artifact paths, and `piperMs`/`whisperMs` durations.
- The smoke output does not print text, transcript, or absolute paths by default. Use `-IncludeTranscript`, `-KeepTranscriptArtifacts`, and `-ShowPaths` only during local debugging.

A passing smoke does not validate the physical microphone, wake-word reliability, noisy-room accuracy, a dedicated wake-word neural model, or final app playback. Check those manually in the desktop app after restarting it.

## 6. Required Checks

```powershell
cargo fmt --all --check
$env:DATABASE_URL = "postgres://aro:aro@127.0.0.1:5432/aro"
$env:ARO_REDIS_URL = "redis://127.0.0.1:6379/0"
$env:ARO_JWT_SECRET = "dev-jwt-secret-0123456789-abcdefghijklmnopqrstuvwxyz"
$env:ARO_SECRETS_KEY = "dev-secrets-key-0123456789-abcdefghijklmnopqrstuvwxyz"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check
npm run build
npm run smoke:api
npm run smoke:redis
```

Run `powershell -ExecutionPolicy Bypass -File scripts\doctor.ps1 -VoiceSmoke` when voice setup, paths, model slots, or runtime packaging changed.

`npm run build` may warn about the current large frontend chunk and mixed static/dynamic Tauri imports. Those warnings are not runtime failures.

## 7. Useful Inspection Queries

```powershell
docker exec aro-postgres psql -U aro -d aro -c "SELECT email, created_at FROM users ORDER BY created_at DESC LIMIT 5;"
docker exec aro-postgres psql -U aro -d aro -c "SELECT event_type, created_at FROM outbox_events ORDER BY created_at DESC LIMIT 10;"
docker exec aro-postgres psql -U aro -d aro -c "SELECT action, target_type, created_at FROM audit_events ORDER BY created_at DESC LIMIT 10;"
```

## 8. Reset Local Test Database

Only use this for the local Docker development database:

```powershell
docker compose down -v
docker compose up -d postgres redis
```

This removes the local PostgreSQL, Redis, Qdrant, and MinIO volumes and all local test data/cache state managed by Docker Compose.

## PostgreSQL production identities

Production uses three separate PostgreSQL login roles:

- `aro_migrator` owns schema objects and is used only by the migration job through `ARO_MIGRATION_DATABASE_URL`;
- `aro_app` is the API role used by `DATABASE_URL`;
- `aro_worker` is the restricted background-worker role used by `ARO_WORKER_DATABASE_URL`.

`aro_app` and `aro_worker` must be `NOSUPERUSER NOBYPASSRLS NOINHERIT` and must own no application table. A production API or worker refuses to start if these invariants are false. For a fresh bundled Compose/Helm PostgreSQL, the init scripts provision the runtime roles. Existing database volumes are not reinitialized: provision the roles manually, apply migrations as `aro_migrator`, then rotate the API and worker connection strings before deploying hardened replicas.

For Compose, set `deploy/secrets/postgres_user` to `aro_migrator`, and create independent high-entropy `postgres_password`, `aro_app_password`, and `aro_worker_password` secret files. Keep the matching URLs separated across `.env.production.api`, `.env.production.worker`, and `.env.production.migrate`; `.env.production` contains only shared non-secret configuration.
