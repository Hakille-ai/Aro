# ARO Product Spec

## Vision

ARO is a private local voice assistant for people who want to think, write, code, and organize ideas without sending data to cloud AI providers.

## Target User

- Builders, students, founders, and power users who want AI help on their machine.
- Privacy-conscious users who do not want transcripts or memory uploaded.
- Users who prefer voice-first interaction but still need a readable conversation history.

## Core Experience

1. Open ARO.
2. Press the microphone or type.
3. ARO transcribes locally when voice is used.
4. Gemma answers through a local runtime.
5. ARO speaks back locally when voice output is enabled.
6. The conversation and memory stay on the device.

## Modes

- Chat: balanced conversational assistant.
- Think: expands rough ideas into structured thinking.
- Code: favors precise technical answers.
- Summarize: converts messy speech into clean notes.
- Quiet: disables speech output.

## Non-Goals

- Cloud account system in the first product milestone.
- Uploading user content to remote LLM providers.
- Shipping large model binaries inside Git.

## Success Criteria

- The app can run entirely offline once local models are installed.
- The interface stays minimal and fast.
- Provider, memory, voice, and UI layers can evolve independently.
- Users can understand and control where data is stored.
- Wake-word, STT, and TTS regressions are screened by CI contracts plus the local model-backed voice harness before release.

## Privacy And Observability

- No raw audio or transcript logs. Captured microphone audio, generated speech, raw STT transcripts, prompts, assistant responses, and memory content must not be written to app logs, audit events, metrics, CI artifacts, or crash reports.
- Voice metrics use aggregate-only observability: counts, duration buckets, runtime/model identifiers, success/failure states, and coarse error categories.
- Local debug logs are opt-in, time-limited, and redacted by default.
- Wake-word, STT, and TTS quality are release-gated by measurable thresholds in [VOICE_ACCEPTANCE.md](VOICE_ACCEPTANCE.md).
- The model-backed voice harness emits privacy-safe fixture IDs, duration metrics, redaction flags, and pass/fail gates by default; raw fixture text or transcripts are local debug-only.
