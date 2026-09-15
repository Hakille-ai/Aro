# Local Models

ARO is built around local Gemma-class models.

## Option A: Ollama

This is the fastest development path.

```powershell
ollama pull gemma4:latest
ollama serve
```

In ARO settings:

- Provider: Ollama
- Endpoint: `http://127.0.0.1:11434`
- Model: the exact model name from `ollama list`, for example `gemma4:latest`

If ARO shows `Ollama returned 404 Not Found`, the most common cause is a model name mismatch. Check installed models:

```powershell
ollama list
```

Then copy the model name exactly into ARO settings.

## Option B: llama.cpp

This is the preferred production path because it gives tighter control over packaging and performance.

1. Download a Gemma GGUF model from an official or trusted source.
2. Start the server on loopback:

```powershell
llama-server -m C:\models\gemma.gguf --host 127.0.0.1 --port 8080
```

In ARO settings:

- Provider: llama.cpp
- Endpoint: `http://127.0.0.1:8080`
- Model: the model name exposed by the server

## Hardware Guidance

- Start with 1B-4B quantized models for broad consumer hardware.
- Use larger models only when memory and GPU acceleration are available.
- Keep voice and LLM models configurable; do not hardcode machine-specific paths.

## Optional Proprietary Providers

ARO can connect OpenAI, Anthropic, Google Gemini, Mistral, and custom OpenAI-compatible providers from Settings > Models.

- Local providers remain first in the selector and no proprietary model is selected by default.
- API keys are saved in the desktop OS credential store for this device.
- API keys are not stored in `settings.json`, PostgreSQL app settings, or browser `localStorage`.
- Proprietary provider requests are made directly by the desktop app in this milestone.

## Voice Is Separate

Ollama does not transcribe microphone audio and does not synthesize speech in ARO. Voice uses separate local model slots:

- `wake-word`: local gate model plus Whisper phrase verification, or a future dedicated wake-word model adapter.
- `stt-whisper`: Whisper.cpp runtime plus a Whisper GGML model.
- `tts-piper`: Piper runtime plus a Piper `.onnx` voice and `.onnx.json` config.

Prepare voice slots without changing app settings:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\download-models.ps1 -SkipText -Voice
```

Install voice slots and write device-local app settings:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-voice.ps1 -RunSmoke
```

The voice smoke output is privacy-safe by default: it redacts text, transcript, and absolute paths. See [VOICE_SETUP.md](VOICE_SETUP.md).
