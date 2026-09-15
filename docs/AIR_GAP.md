# Air-Gap Preparation

Air-gap installs require all artifacts to be mirrored before entering the target network.

## Mirror These Artifacts

- `aro-api` container image and digest.
- Helm chart or Compose bundle.
- Rust crates and npm packages if building inside the air-gapped network.
- Tauri desktop installers and signatures.
- Whisper.cpp, Piper, `llama.cpp` or Ollama runtimes.
- GGUF/GGML/ONNX model files approved by the customer.
- SBOM, checksums, and scan reports.

## Install Pattern

1. Import container images into the internal registry.
2. Place model/runtime binaries on internal file shares or package repositories.
3. Configure desktop clients with internal model paths and `ARO_API_BASE_URL`.
4. Disable third-party integration execution unless the customer allowlists explicit egress.
5. Run `doctor`, voice smoke, API smoke, backup, and restore checks inside the target network.
