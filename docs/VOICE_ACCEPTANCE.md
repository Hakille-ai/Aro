# Voice Acceptance

This document defines the measurable gates for wake-word, speech-to-text (STT), text-to-speech (TTS), and privacy-safe voice observability. These gates are release criteria for shipping voice quality beyond the local setup milestone.

## Scope

- All voice processing must run locally unless the user explicitly chooses a remote provider in settings.
- CI verifies that the acceptance and observability contract stays documented.
- Runtime audio quality tests remain manual or lab-run until a deterministic fixture harness exists.
- Test artifacts may include scripted phrases and aggregate result tables, but they must not include captured user audio, generated audio, raw transcripts, prompts, or assistant responses.

## Reference Environment

Measure gates on a documented reference machine before release:

- Windows laptop or desktop matching the minimum supported product profile.
- Whisper.cpp with the default supported model from `scripts/setup-voice.ps1`.
- Piper with the default supported French voice from `scripts/setup-voice.ps1`.
- Built app, not only development mode.
- Quiet room and moderate-noise passes recorded separately.

Every run must record only aggregate counters, durations, success/failure statuses, runtime versions, model identifiers, and coarse hardware class. Do not record exact user phrases beyond prewritten fixture IDs.

## Wake-word Gates

Wake-word remains opt-in and must be disabled by default until the shipped implementation passes these gates:

| Gate | Required threshold |
| --- | --- |
| False accept rate | <= 1 false activation per 8 hours of quiet desk runtime |
| False reject rate | <= 5% across at least 60 spoken invocations from at least 3 speakers |
| Wake-to-recording latency | p95 <= 350 ms from wake detection to active recording state |
| Detection latency | p95 <= 250 ms from wake phrase end to detection event |
| User control | Visible setting can disable wake-word without disabling push-to-talk voice |
| Privacy | Local-only detection; no raw audio buffers, wake snippets, or transcripts are logged |

## STT Gates

STT acceptance applies to local Whisper.cpp transcription:

| Gate | Required threshold |
| --- | --- |
| Quiet-room word error rate | <= 12% on the approved short-command fixture set |
| Moderate-noise word error rate | <= 20% on the approved short-command fixture set |
| Transcription latency | p95 <= 2.5 s for clips up to 15 s on the reference machine |
| Empty-result rate | <= 2% for non-silent fixture clips |
| Temporary file cleanup | Captured WAV files are deleted within 60 s after transcription completes or fails |
| Privacy | MUST NOT log raw audio. MUST NOT log raw transcripts. MUST NOT log prompts or assistant text |

## TTS Gates

TTS acceptance applies to local Piper synthesis and playback:

| Gate | Required threshold |
| --- | --- |
| Synthesis success rate | >= 99% across the approved fixture set |
| Time to first audio | p95 <= 1.2 s for responses up to 300 characters |
| Long-response start latency | p95 <= 2.5 s for responses up to 1,000 characters |
| Interrupt latency | p95 <= 250 ms from user stop/interruption to playback silence |
| Playback cleanup | Generated WAV bytes and object URLs are released after playback or failure |
| Privacy | MUST NOT log generated audio. MUST NOT log raw transcripts. MUST NOT log prompts or assistant text |

## Privacy-safe Observability

Allowed fields:

- Event name such as `voice.stt.completed`, `voice.tts.failed`, or `voice.wake_word.activated`.
- Timestamp rounded to the minute or coarser.
- Session-scoped random ID that rotates and is not tied to a stable user fingerprint.
- Runtime kind, model ID, app version, OS family, and coarse device class.
- Duration buckets, success/failure state, error category, and aggregate counters.

Forbidden fields:

- Raw audio bytes, audio snippets, generated audio, spectrograms, or waveform dumps.
- Raw transcripts, prompts, assistant responses, memory content, or conversation titles.
- Absolute local paths that expose usernames or private directories.
- API keys, tokens, passwords, database URLs, or provider secrets.
- Stable hardware fingerprints or cross-session voiceprints.

Debug logs must be local, opt-in, time-limited, and redacted by default. Any future telemetry export must use aggregate-only observability and must pass review against this document before release.

## Model-backed Performance Harness

Run the local model-backed harness after `scripts/setup-voice.ps1` has installed Whisper.cpp and Piper:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\measure-voice.ps1 -MetricsPath vendor\voice\voice-metrics.json
```

The harness synthesizes scripted fixture phrases with Piper, transcribes the generated WAV files with Whisper.cpp, then deletes generated audio unless `-KeepArtifacts` is passed. The default JSON output contains fixture IDs, pass/fail states, duration buckets, p95 Piper synthesis time, p95 Whisper transcription time, word error rate, wake-word positive/negative fixture counts, model file names, and redaction flags. It does not capture microphone input and does not emit raw fixture text or raw transcripts unless `-IncludeTranscript` is explicitly passed for local debugging.

Default harness gates:

| Gate | Default threshold |
| --- | --- |
| Wake positive fixture | 0 false rejects across scripted positive wake-word fixtures |
| Wake negative fixture | 0 false accepts across scripted negative wake-word fixtures |
| STT/TTS word error rate | max <= 35%, with fixture-specific allowances for short wake phrases |
| Piper synthesis elapsed time | p95 <= 1.2 s for short scripted responses |
| Whisper transcription elapsed time | p95 <= 2.5 s for short generated clips |
| Privacy | `rawAudioLogged=false`, `generatedAudioLogged=false`, `transcriptsRedacted=true`, and no absolute paths in shared metrics |

This harness is a practical regression screen for model-backed wake-word/STT/TTS wiring. It is not a replacement for the release gates above: microphone permission, room-noise tolerance, real-speaker false accept rate, real-speaker false reject rate, playback interrupt latency, and true time to first audio still require manual or lab measurement on the reference machine.

## CI Contract

The CI workflow runs:

```powershell
./scripts/voice-metrics-notes.ps1 -VerifyDocs -VerifyScripts -VerifyCi
./scripts/measure-voice.ps1 -VerifyOnly
```

That check prevents removal of the measurable voice gates, required test scripts/docs, the voice unit-test entry point, the model harness contract, and the no raw audio/transcript logging rule. It does not download models, run Whisper/Piper, capture microphone input, or persist audio/transcript fixtures.

The broader CI workflow also runs the desktop unit tests with `npm run test:unit`, which covers wake-word matching, Whisper hallucination filtering, WAV encoding, and bounded audio buffers. CI verifies the harness in `-VerifyOnly` mode because hosted runners do not have local Whisper/Piper models installed.
