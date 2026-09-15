# Voice Setup

ARO voice is local-first. Text generation can use Ollama or `llama.cpp`, but microphone transcription, wake-word handling, and spoken responses are separate local voice model slots.

## Voice Model Slots

The supported local slots are:

| Slot | Purpose | Default local path | Used by app today |
| --- | --- | --- | --- |
| `wake-word` | Opt-in hands-free trigger with a local gate model before phrase verification | `vendor\voice\models\wake-word\aro-whisper-gate\slot.json` | Yes, as a lightweight local gate plus Whisper phrase matching |
| `stt-whisper` | Speech-to-text with Whisper.cpp | `vendor\voice\models\whisper\ggml-base.bin` | Yes |
| `tts-piper` | Text-to-speech with Piper | `vendor\voice\models\piper\fr_FR-upmc-medium\fr_FR-upmc-medium.onnx` | Yes |

The default wake-word slot installs a lightweight local gate model. The desktop app captures a short rolling local microphone buffer, runs the gate first to skip silence/quiet noise, then runs local Whisper only when needed to verify `ARO` with simple prefixes such as `hey`, `ok`, `okay`, `bonjour`, or `salut`. Pass `-WakeWordModelUrl` to replace the lightweight gate file with a dedicated on-device wake-word model when an adapter is available.

## Automatic Setup

From the repo root:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-voice.ps1
```

The setup script downloads and validates:

- Wake-word gate model and slot manifest under `vendor\voice\models\wake-word\aro-whisper-gate\`.
- Voice slots manifest under `vendor\voice\models\voice-slots.json`.
- Whisper.cpp Windows x64 runtime.
- Whisper `ggml-base.bin` model.
- Piper Windows x64 runtime.
- French Piper voice `fr_FR-upmc-medium`.
- Required Whisper DLLs, Piper DLLs, Piper voice config, and Piper `espeak-ng-data`.

To install and immediately run a local Piper-to-Whisper round trip:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-voice.ps1 -RunSmoke
```

To show absolute device-local paths in command output:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-voice.ps1 -RunSmoke -ShowPaths
```

By default, command output redacts absolute paths outside the repo. The settings file still receives absolute paths because runtime binaries and model files are device-local.

## Download Only

`scripts\download-models.ps1` still supports local text model guidance or Ollama pulls. It can also prepare voice slots without writing ARO settings:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -SkipText -Voice
```

Download or refresh only one voice slot:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -SkipText -VoiceSlots whisper
powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -SkipText -VoiceSlots piper
powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -SkipText -VoiceSlots wake-word
```

Place a dedicated wake-word model file in the wake-word slot:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -SkipText -VoiceSlots wake-word -WakeWordModelUrl "https://example.invalid/aro-wake.onnx" -WakeWordModelFile "aro-wake.onnx"
```

That stores the file locally and updates the slot manifest. The current desktop app can use the default lightweight gate model directly; a neural wake-word model still needs a matching source-code adapter if its file format is not the built-in gate format.

## Device-Local Settings

`setup-voice.ps1` writes absolute runtime paths into the desktop settings file:

```text
C:\Users\<you>\AppData\Roaming\ARO\ARO\data\settings.json
```

Known settings fields:

- `voice.enabled`
- `voice.speechToText`
- `voice.textToSpeech`
- `voice.whisperBinary`
- `voice.whisperModelPath`
- `voice.piperBinary`
- `voice.piperVoicePath`
- `voice.wakeWord.enabled`
- `voice.wakeWord.runtime`
- `voice.wakeWord.modelPath`
- `voice.wakeWord.threshold`
- `speakResponses`

Wake-word enabled is a user preference, but the dedicated wake-word model slot is not part of the current app settings schema. The slot manifest lives under `vendor\voice\models\wake-word\...` and is checked by `doctor.ps1`.

Runtime binary and model paths point to files on the current Windows machine and must be reconfigured on every device. User preferences such as theme, language, wake-word enabled, and inference mode can sync through the API, but copied voice paths are not portable across machines.

Provider API keys are also device-local and are stored in the OS credential store, not in `settings.json`, PostgreSQL app settings, or browser `localStorage`.

## Installed Paths

Default local paths:

```text
vendor\voice\models\wake-word\aro-whisper-gate\slot.json
vendor\voice\models\voice-slots.json
vendor\voice\runtimes\whisper.cpp\Release\whisper-cli.exe
vendor\voice\models\whisper\ggml-base.bin
vendor\voice\runtimes\piper\piper\piper.exe
vendor\voice\models\piper\fr_FR-upmc-medium\fr_FR-upmc-medium.onnx
vendor\voice\models\piper\fr_FR-upmc-medium\fr_FR-upmc-medium.onnx.json
```

`vendor/voice/downloads`, `vendor/voice/runtimes`, `vendor/voice/models`, generated WAV files, and optional smoke text outputs are ignored by Git because they contain third-party binaries, model files, or local test artifacts.

## Diagnostics

Run the general doctor without an audio round trip:

```powershell
npm run doctor
```

Run doctor plus the optional Piper-to-Whisper smoke:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\doctor.ps1 -VoiceSmoke
```

Show absolute paths only when debugging local machine setup:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\doctor.ps1 -VoiceSmoke -ShowPaths
```

Run only the voice smoke:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\smoke-voice.ps1
```

The smoke test:

1. Synthesizes `Bonjour, je suis ARO.` through Piper.
2. Writes `vendor\voice\voice-smoke.wav`.
3. Transcribes the generated WAV through Whisper.
4. Verifies the generated transcript contains stable expected tokens such as `bonjour` and `suis`.
5. Prints privacy-safe JSON with `audioBytes`, `piperMs`, `whisperMs`, redaction flags, token expectations, and repo-relative artifact paths.

By default, the smoke output does not print the text fixture, the transcript, or absolute paths. It also does not write Whisper stdout/stderr transcript artifacts. Use these switches only for local debugging:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\smoke-voice.ps1 -IncludeTranscript -KeepTranscriptArtifacts -ShowPaths
```

A passing smoke proves the configured binaries, model files, DLL loading, and runtime working directories are usable. It does not prove microphone permissions, room noise tolerance, wake-word quality, or end-user speech accuracy.

## App Settings

In ARO settings:

- Voice enabled: on.
- Speech to text: Whisper.cpp.
- Text to speech: Piper.
- Whisper binary: absolute path to `whisper-cli.exe`.
- Whisper model: absolute path to `ggml-base.bin`.
- Piper binary: absolute path to `piper.exe`.
- Piper voice: absolute path to the `.onnx` voice file.
- Speak responses: on if you want ARO to read answers aloud.
- Wake word: optional; currently uses local Whisper phrase matching rather than a dedicated wake-word model.

## Voice Modes

- Push-to-talk: click the microphone once to record, speak, then click again to stop and transcribe.
- Spoken responses: when `speakResponses` is enabled and Piper is configured, assistant replies can be synthesized locally.
- Wake word: opt-in hands-free trigger for "ARO" style invocations.
- Assistant modes: `chat`, `think`, `code`, `summarize`, and `quiet` change the system prompt and response style, not the Whisper/Piper runtime.
- Inference modes: `local` uses local model providers by default; `cloud` is an explicit connected-provider choice and is separate from voice setup.

## Wake-word Limitations

Wake word is not a dedicated hotword engine in this milestone. Known limitations:

- It is disabled unless the user turns it on.
- It only runs while the desktop app is open and voice is configured.
- It can false-accept or false-reject in noisy rooms, with accents, or with short utterances.
- It uses more CPU and battery than push-to-talk because it periodically invokes Whisper.
- The reserved wake-word model slot is checked by scripts but is not consumed by app source code yet.
- It should not be treated as a security boundary or a guaranteed hands-free control.
- Push-to-talk remains the reliable baseline for acceptance.

## Manual Runtime Test

Test Piper:

```powershell
'Bonjour, je suis ARO.' | vendor\voice\runtimes\piper\piper\piper.exe --model vendor\voice\models\piper\fr_FR-upmc-medium\fr_FR-upmc-medium.onnx --output_file vendor\voice\test-piper.wav
```

Test Whisper on that generated file:

```powershell
vendor\voice\runtimes\whisper.cpp\Release\whisper-cli.exe -m vendor\voice\models\whisper\ggml-base.bin -f vendor\voice\test-piper.wav -l fr -nt -np
```

Expected result: Whisper prints a transcription similar to `Bonjour, je suis ARO`. Variants such as `Aero` are acceptable for this synthetic smoke as long as the stable tokens are present.

## Troubleshooting

If clicking the microphone opens settings, voice is disabled or Speech to text is disabled.

If transcription is empty, check Windows microphone permission, confirm the selected input device, and speak for at least a few seconds before stopping.

If `doctor.ps1` warns that the wake-word slot is reserved, this is expected for the current milestone. It means the app uses Whisper phrase matching instead of a dedicated hotword model.

If Whisper fails to start, verify that `whisper-cli.exe`, `whisper.dll`, `ggml.dll`, and the CPU backend DLLs are in `vendor\voice\runtimes\whisper.cpp\Release\`.

If Piper fails to start, verify that `piper.exe`, `onnxruntime.dll`, `piper_phonemize.dll`, the `.onnx.json` voice config, and `espeak-ng-data` are present under the Piper runtime and model folders.

If `setup-voice.ps1` reports a tiny existing download, delete only the interrupted file under `vendor\voice\downloads\` and rerun setup.

If voice setup works but text chat fails, check the selected model provider separately:

```powershell
ollama list
```

The model name in ARO must exactly match one of the installed names.

## Acceptance Metrics

For local setup acceptance, record only aggregate results:

- `scripts\doctor.ps1 -VoiceSmoke` exits successfully.
- Voice smoke emits `ok: true`.
- `audioBytes` is greater than the configured minimum, currently 12 KB.
- Transcript contains the expected stable fixture tokens.
- `piperMs` and `whisperMs` are captured for trend comparison on the same hardware.
- `doctor.ps1` sees the three local voice slots: `wake-word`, `stt-whisper`, and `tts-piper`.

Do not log raw microphone audio, generated user audio, user transcripts, prompts, assistant responses, secrets, or absolute paths in shared telemetry.
