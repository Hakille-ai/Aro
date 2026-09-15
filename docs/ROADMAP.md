# Roadmap

## Milestone 1: Local Product Foundation

- Tauri desktop shell.
- Apple Minimal interface.
- Local provider settings.
- Mock, Ollama, and llama.cpp provider contracts.
- SQLite conversation memory.
- Local voice setup with Whisper.cpp, Piper, WAV microphone capture, and French Piper voice.
- Developer checks and docs.

## Milestone 2: Voice Quality

- Streaming microphone transcription.
- In-app voice installer and model manager.
- Piper voice selection.
- Latency measurement through `scripts/measure-voice.ps1`.
- Interruptible speech playback.
- Voice acceptance gates for wake-word, STT, and TTS quality.
- Privacy-safe voice observability with no raw audio or transcript logs.
- CI contract for required voice docs, scripts, unit tests, and harness self-validation.

## Milestone 3: Product Memory

- User-approved memory entries.
- Local embeddings.
- Semantic recall.
- Per-project memory spaces.

## Milestone 4: System Integration

- Global shortcut.
- Floating mini assistant.
- Local file context with explicit permissions.
- Packaging and auto-update pipeline.
