# ARO Architecture

ARO is organized as a product-grade monorepo with explicit boundaries.

## Layers

```text
Svelte UI
  |
Tauri commands
  |
Cloud API client / local runtime bridge
  |
Axum API
  |
PostgreSQL
  |
Redis for short-lived API coordination
```

Degraded local/runtime mode still supports local inference and short-lived desktop state:

```text
Svelte UI
  |
Tauri commands
  |
Local model / voice / memory adapters
```

## Crates

### `aro-core`

Shared domain models, settings, runtime status, voice contracts, and errors. This crate should remain dependency-light and stable.

### `aro-memory`

Legacy/local memory helpers. PostgreSQL-backed `aro-store` is the source of truth for product data.

### `aro-store`

PostgreSQL persistence for the cloud API. It owns SQLx migrations, tenant-scoped repositories, settings/preferences persistence, refresh-token storage, outbox events, and conversation/message repositories.

### `apps/api`

Axum HTTP API. It owns auth, JWT sessions, organization scoping, bootstrap, PostgreSQL-backed CRUD, SSE endpoints for cloud assistant streaming, and Redis-backed rate limits, short locks, readiness, metrics, and outbox stream fanout.

Redis is optional outside production. When configured, startup verifies it and the API uses it only for short-lived coordination:

- request rate limits keyed by IP or JWT user/organization;
- conversation and agent-lane locks with short TTLs;
- worker publication of PostgreSQL outbox rows to a Redis Stream for fast consumers.

Redis loss must not delete product data. PostgreSQL remains the durable source of truth.

### `aro-runtime`

Assistant orchestration and model providers. Local providers are:

- Mock provider for development and tests.
- Ollama provider for fast local setup.
- `llama.cpp` OpenAI-compatible provider for production local inference.

### `aro-voice`

Speech-to-text and text-to-speech adapters. The product contract assumes Whisper.cpp and Piper-compatible local binaries, with a disabled runtime for machines that have not configured voice yet. Voice setup also maintains local model slots for wake-word, STT Whisper, and TTS Piper under `vendor/voice/models`.

### `apps/desktop/src-tauri`

Tauri integration layer. It owns secure token storage, API client commands, local runtime commands, app data directories for ephemeral/cache state, and the frontend bridge.

## Data Flow

1. User authenticates with the Axum API. The returned access JWT cryptographically binds the user and active organization; each authenticated request revalidates active membership for that scope.
2. Tauri stores the refresh token in the OS credential store and keeps the active session in process memory. Its session mutex covers refresh, organization switch, and logout, and guards installation of sessions returned by primary authentication.
3. Frontend calls the Tauri/API facade for bootstrap, settings, conversations, messages, teams, and collections.
4. Tauri reads/writes PostgreSQL-backed data through the API, never directly through a database connection.
5. Local inference streams from Ollama or `llama.cpp` when selected.
6. Generated local results are posted back to `/assistant/local-result` so PostgreSQL remains the durable source of truth.
7. The API may use Redis to rate-limit or lock the short write window, but final data is still committed to PostgreSQL.

Organization switching is not implemented through a tenant header. It requires the current access and refresh tokens and atomically rotates the refresh credential before issuing a JWT for the target organization. Refresh credentials form a durable per-login family with a non-extendable absolute deadline; PostgreSQL serializes family transitions, detects replay of a rotated predecessor, and revokes the family on replay or logout. Worker maintenance deletes complete expired/revoked lineages only after the configured evidence-retention window.

The plain browser client deliberately has no persistent credential store: access and refresh tokens remain in module memory and legacy `localStorage` token keys are purged. A single-flight refresh plus serialized session mutations prevents concurrent `401` responses from consuming the same predecessor multiple times. Persistent web authentication remains a future HttpOnly-cookie/BFF capability.

## Cloud Data Flow

Cloud assistant mode uses `/v1/assistant/stream` directly from the API over SSE. Local assistant mode uses Tauri runtime streaming and then persists the final user/assistant messages through the versioned API.

## Settings Ownership

ARO has two classes of settings:

- Cloud-synced user/workspace settings: conversations, messages, memories, teams, preferences, model/provider selections, voice profile metadata, theme, language, wake-word enabled, and inference mode.
- Device-local operational settings: executable paths, model file paths, runtime caches, scratch audio files, local database/cache files, and provider API keys stored in the OS credential store.

The desktop settings file lives under the platform app data directory, for example:

```text
C:\Users\<you>\AppData\Roaming\ARO\ARO\data\settings.json
```

Whisper and Piper paths can appear in the app settings shape, but those paths are only valid on the machine that wrote them. New devices must run `scripts\setup-voice.ps1` or set their own local paths after login. Do not treat local runtime paths as portable cloud data.

The wake-word model slot is currently tracked by a local manifest under `vendor\voice\models\wake-word\...`, not by the synced settings schema. This keeps future dedicated wake-word model files device-local while the current app continues to use Whisper phrase matching.

## Voice Flow

1. Frontend records microphone input through the browser audio APIs inside the Tauri webview.
2. The app encodes recorded PCM samples as WAV bytes.
3. Frontend calls `voice_transcribe`.
4. Tauri writes the WAV to the local scratch directory.
5. `aro-voice` runs `whisper-cli.exe` from its runtime directory with the configured local model.
6. The transcription is returned to the composer.
7. When speech output is enabled, frontend calls `voice_synthesize`.
8. `aro-voice` runs `piper.exe` with the configured `.onnx` voice and returns WAV audio.
9. Frontend plays the returned WAV locally.

## Wake-word Flow

Wake-word is currently an opt-in local gate plus Whisper phrase verification, not yet a dedicated always-on neural hotword model:

1. The desktop app opens a local microphone stream only when wake-word is enabled and voice is configured.
2. The frontend keeps a short rolling PCM buffer.
3. A timer encodes the recent buffer to WAV and calls `voice_wake_word_detect`.
4. The backend runs the local wake-word gate model first to reject silence/quiet noise cheaply.
5. When the gate activates, local Whisper verifies `ARO` and simple prefixes such as `hey`, `ok`, `okay`, `bonjour`, or `salut`.
6. On a verified match, the app enters the normal recording path.

`scripts\setup-voice.ps1` and `scripts\download-models.ps1 -SkipText -VoiceSlots wake-word` create a local `wake-word` slot manifest and default gate model. Passing `-WakeWordModelUrl` can place a dedicated on-device model file in that slot, but neural model formats still need a matching adapter.

This design keeps detection local, but it is heavier and less reliable than a specialized wake-word engine. It can false-accept or false-reject and should not be used as a security or authorization boundary. Push-to-talk remains the reliable baseline mode.

## Scaling Principles

- Keep feature code out of Tauri commands when it belongs in crates.
- Keep providers replaceable behind explicit contracts.
- Keep PostgreSQL credentials server-side only.
- Keep Redis server-side only; desktop clients never receive Redis URLs or credentials.
- Treat Redis data as ephemeral coordination state, not product storage.
- Prefer local model adapters and loopback endpoint checks for default inference.
- Run local voice binaries from their own runtime directories so Windows DLL loading stays reliable.
- Keep raw audio, generated WAV files, transcripts, absolute local paths, and voice smoke artifacts out of Git and shared telemetry.
- Treat `vendor/voice/models/voice-slots.json` as a device-local inventory, not cloud state.
- Add migrations instead of destructive schema changes.
