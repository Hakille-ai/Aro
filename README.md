# ARO

ARO is a cloud-synced desktop AI product with local inference by default. The desktop app stays a secure client, while the Rust API owns authentication, multi-tenant PostgreSQL persistence, audit/outbox events, teams, settings, plugins, MCP servers, hooks, scheduler state, and usage data.

## Product Pillars

Commercial rollout: [offres, facturation et exploitation](docs/COMMERCIALIZATION.md).
Community, Cloud, Business and Enterprise are defined in the shared commercial catalog; managed AI compute is billed separately. Payments are disabled by default pending operational setup.

## License

ARO's core is source-available under [PolyForm Noncommercial 1.0.0](LICENSE). Uses outside that license's permitted purposes require a separate commercial agreement. The SDK in `packages/api-client` and API contracts in `packages/contracts` are licensed under Apache-2.0, with a LICENSE in each directory. Other components inherit the root license unless explicitly stated otherwise. Third-party software, models and assets retain their own terms. See [the licensing scope and commercial arrangements](legal/README.md) and [NOTICE](NOTICE).

The core is not described as OSI open source: PolyForm restricts commercial use. Buying hosted services or compute credits does not by itself grant commercial rights to the core. No repository publication, hosted deployment or live-payment activation is implied by this license change.

- Local inference by default: Ollama or `llama.cpp` can stream model output from the desktop, then persist results through the API.
- PostgreSQL source of truth: conversations, messages, settings, memories, organizations, memberships, skills, plugins, MCP, hooks, scheduler, and usage events live behind the API.
- Redis-backed coordination: the API can use Redis for rate limiting, short distributed locks, readiness metrics, and fast outbox fanout while PostgreSQL remains durable.
- Minimal interface: Apple Minimal direction, with the assistant centered around voice, calm status, and direct conversation.
- Scalable architecture: Rust crates separate domain models, PostgreSQL store, model runtime, voice runtime, API, and desktop integration.
- Swappable engines: local Gemma through `llama.cpp` or Ollama, with opt-in proprietary providers configured per device.
- Security controls: no direct desktop database access, tenant-scoped JWT access tokens, serialized refresh-token families, OS-keyring custody for the desktop refresh credential, hashed API keys, and encrypted sensitive integration material.

## Workspace

```text
apps/desktop        Tauri 2 + Svelte desktop app
apps/api            Axum API for PostgreSQL cloud persistence
crates/aro-core     Shared domain types and contracts
crates/aro-memory   Legacy/local memory helpers
crates/aro-store    PostgreSQL persistence and migrations
crates/aro-runtime  Local LLM orchestration
crates/aro-voice    Local STT/TTS adapters
docs/               Product, architecture, security, and model setup
scripts/            Developer setup and checks
vendor/voice        Local voice runtimes and models, ignored by Git
```

## Quick Start

Prerequisites:

- Rust 1.95+
- Node.js 24+
- A local Gemma runtime through Ollama or `llama.cpp` for real model responses

Install frontend dependencies:

```powershell
npm install
```

Run the desktop app in development:

```powershell

```

Run the PostgreSQL API in development:

```powershell
docker compose up -d postgres redis

# Or set the minimum by hand:
$env:DATABASE_URL="postgres://aro:aro@127.0.0.1:5432/aro"
$env:ARO_REDIS_URL="redis://127.0.0.1:6379/0"
$env:ARO_JWT_SECRET="dev-jwt-secret-0123456789-abcdefghijklmnopqrstuvwxyz"
$env:ARO_SECRETS_KEY="dev-secrets-key-0123456789-abcdefghijklmnopqrstuvwxyz"
$env:ARO_CORS_ALLOWED_ORIGINS="http://localhost:1420,http://127.0.0.1:1420,http://tauri.localhost,https://tauri.localhost,tauri://localhost"
$env:ARO_HTTP_MAX_BODY_BYTES="1048576"
$env:ARO_HTTP_REQUEST_TIMEOUT_SECONDS="30"
```

Migrations run automatically on API startup when `ARO_AUTO_MIGRATE=true` (set in `.env`). For a full local stack (vectors, object storage) also start `qdrant`, `minio` and `minio-init`: `docker compose up -d`.

Use random 32+ character secrets outside local development. Values containing placeholders such as `replace-with` or `change-me` are rejected at startup.
Remote API deployments should use explicit HTTPS CORS origins. Wildcard CORS is rejected.

Then start the desktop app in another terminal. The first screen is the auth gate; register a workspace or log in to sync conversations, settings, profiles, plugins, MCP servers, hooks, scheduled tasks, teams, and API keys through PostgreSQL.

Run checks:

```powershell
npm run check
npm run build
npm run doctor
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Run a full API smoke test against the local PostgreSQL stack:

```powershell
npm run smoke:api
npm run smoke:redis
```

New API integrations should use the canonical `/v1` prefix. Existing unversioned desktop routes remain temporarily available as a compatibility facade and return deprecation metadata.

See [docs/RUNBOOK.md](docs/RUNBOOK.md) for the full local backend/API/frontend runbook.

## Production And Self-Host

The production backend is packaged as the `aro-api` container image with explicit modes:

```powershell
aro-api serve
aro-api migrate
aro-api worker
aro-api admin create-owner --email owner@example.com --password "<strong password>" --organization "Example"
```

Self-host deployments should use `deploy/compose.prod.yml` or `deploy/helm/aro`, run migrations before rolling API replicas, and keep signup closed until an owner or SSO path is configured. See:

- [docs/SELF_HOST.md](docs/SELF_HOST.md)
- [docs/BACKUP_RESTORE.md](docs/BACKUP_RESTORE.md)
- [docs/UPGRADE.md](docs/UPGRADE.md)
- [docs/SECURITY_HARDENING.md](docs/SECURITY_HARDENING.md)
- [docs/AIR_GAP.md](docs/AIR_GAP.md)
- [docs/PRODUCTION_READINESS.md](docs/PRODUCTION_READINESS.md)

`docs/PRODUCTION_READINESS.md` is the release gate for a public deployment. In particular, the API refuses production startup with open signup, implicit CORS, or filesystem-backed file storage. Integrations and OAuth are deny-by-default and must be explicitly enabled only after their provider configuration is complete.

The app can run in development with the mock model provider. Configure local Gemma in Settings > Models when your model runtime is ready. Proprietary providers can also be added there, but local models remain the default and provider API keys stay in the OS credential store.

## Voice Setup

Ollama only handles text generation. For voice, ARO uses local model slots:

- `wake-word` for local wake-word metadata or future dedicated wake-word models.
- `stt-whisper` for Whisper.cpp speech-to-text.
- `tts-piper` for Piper text-to-speech.

Install and configure the voice slots with:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-voice.ps1
```

Install and immediately verify the local Piper-to-Whisper path with:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-voice.ps1 -RunSmoke
```

Download or refresh the voice slots without writing app settings:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -SkipText -Voice
```

Then restart the desktop app:

```powershell
npm run desktop:dev
```

In the app, click the microphone once to record, speak, then click it again to stop and transcribe. See [docs/VOICE_SETUP.md](docs/VOICE_SETUP.md).

Voice diagnostics:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\doctor.ps1 -VoiceSmoke
powershell -ExecutionPolicy Bypass -File scripts\smoke-voice.ps1
```

Wake-word is opt-in and currently implemented through periodic local Whisper checks, not a dedicated hotword engine. The wake-word slot manifest is checked by scripts, but a dedicated wake-word model file is not consumed by app source code yet. Push-to-talk is the reliable baseline.

Voice smoke output is privacy-safe by default: text, transcript, and absolute paths are redacted unless you pass explicit local-debug switches. Whisper/Piper binary and model paths are device-local, while user preferences such as wake-word enabled and inference mode can sync through the API.

## Local Model Options

Recommended first setup:

- Ollama with a Gemma model for the easiest start.
- `llama.cpp` OpenAI-compatible server for production control and packaging.

See [docs/LOCAL_MODELS.md](docs/LOCAL_MODELS.md).

## Status

This repository is a serious engineering foundation, not yet a generally available platform. The current milestone provides a Rust API + PostgreSQL control plane, a secure Tauri client baseline, local voice/model contracts and developer validation. Durable agent execution, RLS, full identity, malware scanning, production key management and operational release gates remain mandatory before a public launch; see [docs/PRODUCTION_READINESS.md](docs/PRODUCTION_READINESS.md), [docs/BACKEND_AUDIT_2026-07.md](docs/BACKEND_AUDIT_2026-07.md) and [docs/BACKEND_TARGET_ARCHITECTURE.md](docs/BACKEND_TARGET_ARCHITECTURE.md). Model binaries are not committed to the repo.
