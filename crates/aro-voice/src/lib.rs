use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use aro_core::{
    AroError, AroResult, SynthesisRequest, SynthesisResult, TranscriptionRequest,
    TranscriptionResult, VoiceRuntimeKind, VoiceSettings, WakeWordDetectionRequest,
    WakeWordDetectionResult, WakeWordRuntimeKind,
};
use uuid::Uuid;

const MAX_WAV_BYTES: usize = 25 * 1024 * 1024;
const MIN_WAV_BYTES: usize = 44;
const MAX_SYNTHESIS_TEXT_BYTES: usize = 16 * 1024;
const MAX_ERROR_OUTPUT_CHARS: usize = 4_000;
const WHISPER_TIMEOUT: Duration = Duration::from_secs(120);
const PIPER_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct VoiceService {
    settings: VoiceSettings,
    scratch_dir: PathBuf,
}

impl VoiceService {
    pub fn new(settings: VoiceSettings, scratch_dir: impl Into<PathBuf>) -> Self {
        Self {
            settings,
            scratch_dir: scratch_dir.into(),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.validate_ready().is_ok()
    }

    pub fn detect_wake_word(
        &self,
        request: WakeWordDetectionRequest,
    ) -> AroResult<WakeWordDetectionResult> {
        let runtime = self.wake_word_runtime()?;
        validate_wake_word_request(&request)?;

        let scorer = LightweightWakeWordScorer::load(&runtime.model)?;
        let adapter = LocalWakeWordAdapter::new(scorer, runtime.model, runtime.threshold)?;
        adapter.detect(&request.audio_bytes)
    }

    pub fn transcribe(&self, request: TranscriptionRequest) -> AroResult<TranscriptionResult> {
        let runtime = self.transcription_runtime()?;
        validate_transcription_request(&request)?;
        let language = transcription_language(request.language.as_deref())?;

        ensure_scratch_dir(&self.scratch_dir)?;
        let audio_path = ScratchPath::new(&self.scratch_dir, "transcription", "wav");
        let stdout_path = ScratchPath::new(&self.scratch_dir, "whisper-stdout", "txt");
        let stderr_path = ScratchPath::new(&self.scratch_dir, "whisper-stderr", "txt");
        fs::write(audio_path.path(), &request.audio_bytes).map_err(|err| {
            AroError::Voice(format!(
                "failed to write transcription scratch file {}: {err}",
                audio_path.path().display()
            ))
        })?;

        let stdout = File::create(stdout_path.path()).map_err(|err| {
            AroError::Voice(format!(
                "failed to create Whisper stdout scratch file {}: {err}",
                stdout_path.path().display()
            ))
        })?;
        let stderr = File::create(stderr_path.path()).map_err(|err| {
            AroError::Voice(format!(
                "failed to create Whisper stderr scratch file {}: {err}",
                stderr_path.path().display()
            ))
        })?;

        let child = Command::new(&runtime.binary)
            .current_dir(executable_dir(&runtime.binary)?)
            .arg("-m")
            .arg(&runtime.model)
            .arg("-f")
            .arg(audio_path.path())
            .arg("-nt")
            .arg("-np")
            .arg("-l")
            .arg(language)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|err| {
                AroError::Voice(format!(
                    "failed to start whisper.cpp binary {}: {err}",
                    runtime.binary.display()
                ))
            })?;

        let status = wait_with_timeout(child, WHISPER_TIMEOUT, "whisper.cpp")?;
        let stdout_text = read_lossy(stdout_path.path())?;
        let stderr_text = read_lossy(stderr_path.path())?;
        if !status.success() {
            return Err(AroError::Voice(format_transcription_process_failure(
                "whisper.cpp",
                status,
                &stderr_text,
            )));
        }

        Ok(TranscriptionResult {
            text: stdout_text.trim().to_string(),
            runtime_detail: "whisper.cpp local adapter".to_string(),
        })
    }

    pub fn synthesize(&self, request: SynthesisRequest) -> AroResult<SynthesisResult> {
        validate_synthesis_request(&request)?;
        let runtime = self.synthesis_runtime(request.voice_path.as_deref())?;

        ensure_scratch_dir(&self.scratch_dir)?;
        let output_path = ScratchPath::new(&self.scratch_dir, "speech", "wav");
        let stdout_path = ScratchPath::new(&self.scratch_dir, "piper-stdout", "txt");
        let stderr_path = ScratchPath::new(&self.scratch_dir, "piper-stderr", "txt");
        let stdout = File::create(stdout_path.path()).map_err(|err| {
            AroError::Voice(format!(
                "failed to create Piper stdout scratch file {}: {err}",
                stdout_path.path().display()
            ))
        })?;
        let stderr = File::create(stderr_path.path()).map_err(|err| {
            AroError::Voice(format!(
                "failed to create Piper stderr scratch file {}: {err}",
                stderr_path.path().display()
            ))
        })?;

        let mut cmd = Command::new(&runtime.binary);
        cmd.current_dir(executable_dir(&runtime.binary)?)
            .arg("--model")
            .arg(&runtime.voice);

        if let Some(speaker) = request.speaker_id {
            cmd.arg("--speaker").arg(speaker.to_string());
        }

        let mut child = cmd
            .arg("--output_file")
            .arg(output_path.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|err| {
                AroError::Voice(format!(
                    "failed to start Piper binary {}: {err}",
                    runtime.binary.display()
                ))
            })?;

        write_child_stdin(&mut child, request.text.as_bytes(), "Piper")?;

        let status = wait_with_timeout(child, PIPER_TIMEOUT, "Piper")?;
        let stdout_text = read_lossy(stdout_path.path())?;
        let stderr_text = read_lossy(stderr_path.path())?;
        if !status.success() {
            return Err(AroError::Voice(format_process_failure(
                "Piper",
                status,
                &stderr_text,
                &stdout_text,
            )));
        }

        let audio_bytes = fs::read(output_path.path()).map_err(|err| {
            AroError::Voice(format!(
                "Piper succeeded but output file {} could not be read: {err}",
                output_path.path().display()
            ))
        })?;
        if audio_bytes.is_empty() {
            return Err(AroError::Voice(format!(
                "Piper succeeded but output file {} was empty",
                output_path.path().display()
            )));
        }

        Ok(SynthesisResult {
            audio_bytes,
            mime_type: "audio/wav".to_string(),
            runtime_detail: "piper local adapter".to_string(),
        })
    }

    fn validate_ready(&self) -> AroResult<()> {
        ensure_voice_enabled(&self.settings)?;

        let mut has_runtime = false;
        match &self.settings.speech_to_text {
            VoiceRuntimeKind::Disabled => {}
            VoiceRuntimeKind::WhisperCpp => {
                resolve_existing_file(
                    self.settings.whisper_binary.as_deref(),
                    "whisper.cpp binary",
                    true,
                )?;
                resolve_existing_file(
                    self.settings.whisper_model_path.as_deref(),
                    "Whisper model",
                    false,
                )?;
                has_runtime = true;
            }
            runtime => {
                return Err(AroError::Voice(format!(
                    "speech-to-text runtime is {}; expected whisper.cpp or disabled",
                    runtime_label(runtime)
                )));
            }
        }

        match &self.settings.text_to_speech {
            VoiceRuntimeKind::Disabled => {}
            VoiceRuntimeKind::Piper => {
                resolve_existing_file(self.settings.piper_binary.as_deref(), "Piper binary", true)?;
                resolve_existing_file(
                    self.settings.piper_voice_path.as_deref(),
                    "Piper voice model",
                    false,
                )?;
                has_runtime = true;
            }
            runtime => {
                return Err(AroError::Voice(format!(
                    "text-to-speech runtime is {}; expected Piper or disabled",
                    runtime_label(runtime)
                )));
            }
        }

        match &self.settings.wake_word.runtime {
            WakeWordRuntimeKind::Disabled => {
                if self.settings.wake_word.enabled {
                    return Err(AroError::Voice(
                        "wake-word runtime is disabled in settings".to_string(),
                    ));
                }
            }
            WakeWordRuntimeKind::LocalModel => {
                if self.settings.wake_word.enabled {
                    resolve_existing_file(
                        self.settings.wake_word.model_path.as_deref(),
                        "wake-word model",
                        false,
                    )?;
                    validate_wake_word_threshold(self.settings.wake_word.threshold)?;
                    has_runtime = true;
                }
            }
        }

        if !has_runtime {
            return Err(AroError::Voice(
                "voice is enabled but speech-to-text, text-to-speech, and wake-word detection are disabled"
                    .to_string(),
            ));
        }

        Ok(())
    }

    fn wake_word_runtime(&self) -> AroResult<WakeWordRuntime> {
        ensure_voice_enabled(&self.settings)?;
        if !self.settings.wake_word.enabled {
            return Err(AroError::Voice(
                "wake-word detection is disabled in settings".to_string(),
            ));
        }

        match &self.settings.wake_word.runtime {
            WakeWordRuntimeKind::LocalModel => Ok(WakeWordRuntime {
                model: resolve_existing_file(
                    self.settings.wake_word.model_path.as_deref(),
                    "wake-word model",
                    false,
                )?,
                threshold: validate_wake_word_threshold(self.settings.wake_word.threshold)?,
            }),
            WakeWordRuntimeKind::Disabled => Err(AroError::Voice(
                "wake-word runtime is disabled in settings".to_string(),
            )),
        }
    }

    fn transcription_runtime(&self) -> AroResult<WhisperRuntime> {
        ensure_voice_enabled(&self.settings)?;
        match &self.settings.speech_to_text {
            VoiceRuntimeKind::WhisperCpp => Ok(WhisperRuntime {
                binary: resolve_existing_file(
                    self.settings.whisper_binary.as_deref(),
                    "whisper.cpp binary",
                    true,
                )?,
                model: resolve_existing_file(
                    self.settings.whisper_model_path.as_deref(),
                    "Whisper model",
                    false,
                )?,
            }),
            VoiceRuntimeKind::Disabled => Err(AroError::Voice(
                "speech-to-text is disabled in settings".to_string(),
            )),
            runtime => Err(AroError::Voice(format!(
                "speech-to-text runtime is {}; expected whisper.cpp",
                runtime_label(runtime)
            ))),
        }
    }

    fn synthesis_runtime(&self, request_voice_path: Option<&str>) -> AroResult<PiperRuntime> {
        ensure_voice_enabled(&self.settings)?;
        match &self.settings.text_to_speech {
            VoiceRuntimeKind::Piper => {
                let voice_path = request_voice_path
                    .filter(|path| !path.trim().is_empty())
                    .or(self.settings.piper_voice_path.as_deref());
                Ok(PiperRuntime {
                    binary: resolve_existing_file(
                        self.settings.piper_binary.as_deref(),
                        "Piper binary",
                        true,
                    )?,
                    voice: resolve_existing_file(voice_path, "Piper voice model", false)?,
                })
            }
            VoiceRuntimeKind::Disabled => Err(AroError::Voice(
                "text-to-speech is disabled in settings".to_string(),
            )),
            runtime => Err(AroError::Voice(format!(
                "text-to-speech runtime is {}; expected Piper",
                runtime_label(runtime)
            ))),
        }
    }
}

#[derive(Debug)]
struct WhisperRuntime {
    binary: PathBuf,
    model: PathBuf,
}

#[derive(Debug)]
struct PiperRuntime {
    binary: PathBuf,
    voice: PathBuf,
}

#[derive(Debug)]
struct WakeWordRuntime {
    model: PathBuf,
    threshold: f32,
}

pub trait WakeWordScorer {
    fn score(&self, audio_bytes: &[u8]) -> AroResult<f32>;
    fn runtime_detail(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub struct LocalWakeWordAdapter<S> {
    scorer: S,
    model_path: PathBuf,
    threshold: f32,
}

impl<S: WakeWordScorer> LocalWakeWordAdapter<S> {
    pub fn new(scorer: S, model_path: impl Into<PathBuf>, threshold: f32) -> AroResult<Self> {
        Ok(Self {
            scorer,
            model_path: model_path.into(),
            threshold: validate_wake_word_threshold(threshold)?,
        })
    }

    pub fn detect(&self, audio_bytes: &[u8]) -> AroResult<WakeWordDetectionResult> {
        let score = self.scorer.score(audio_bytes)?;

        Ok(WakeWordDetectionResult {
            activated: score >= self.threshold,
            score,
            threshold: self.threshold,
            model_path: Some(self.model_path.to_string_lossy().to_string()),
            runtime_detail: self.scorer.runtime_detail().to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct LightweightWakeWordScorer {
    gain: f32,
    bias: f32,
}

impl LightweightWakeWordScorer {
    pub fn load(model_path: &Path) -> AroResult<Self> {
        let bytes = fs::read(model_path).map_err(|err| {
            AroError::Voice(format!(
                "failed to read wake-word model {}: {err}",
                model_path.display()
            ))
        })?;
        Self::from_model_bytes(&bytes)
    }

    fn from_model_bytes(bytes: &[u8]) -> AroResult<Self> {
        let checksum = bytes
            .iter()
            .fold(0_u32, |acc, byte| acc.wrapping_add(u32::from(*byte)));
        let mut gain = 1.0 + (checksum % 11) as f32 / 100.0;
        let mut bias = ((checksum / 11) % 5) as f32 / 100.0;
        let text = String::from_utf8_lossy(bytes);

        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "gain" => gain = parse_model_float("gain", value, index + 1)?,
                "bias" => bias = parse_model_float("bias", value, index + 1)?,
                _ => {}
            }
        }

        if gain < 0.0 {
            return Err(AroError::Voice(
                "wake-word model gain must be zero or greater".to_string(),
            ));
        }

        Ok(Self { gain, bias })
    }
}

impl WakeWordScorer for LightweightWakeWordScorer {
    fn score(&self, audio_bytes: &[u8]) -> AroResult<f32> {
        let energy = pcm16_wav_mean_abs(audio_bytes)?;
        Ok(clamp_score(energy * self.gain + self.bias))
    }

    fn runtime_detail(&self) -> &'static str {
        "deterministic local wake-word scorer"
    }
}

#[derive(Debug)]
struct ScratchPath {
    path: PathBuf,
}

impl ScratchPath {
    fn new(scratch_dir: &Path, prefix: &str, extension: &str) -> Self {
        Self {
            path: scratch_dir.join(format!("{prefix}-{}.{}", Uuid::new_v4(), extension)),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn ensure_voice_enabled(settings: &VoiceSettings) -> AroResult<()> {
    if settings.enabled {
        Ok(())
    } else {
        Err(AroError::Voice(
            "voice runtime is disabled in settings".to_string(),
        ))
    }
}

fn ensure_scratch_dir(scratch_dir: &Path) -> AroResult<()> {
    fs::create_dir_all(scratch_dir).map_err(|err| {
        AroError::Voice(format!(
            "failed to create voice scratch directory {}: {err}",
            scratch_dir.display()
        ))
    })
}

fn resolve_existing_file(
    path: Option<&str>,
    label: &str,
    require_executable: bool,
) -> AroResult<PathBuf> {
    let raw_path = path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| AroError::Voice(format!("{label} path is missing")))?;
    let path = Path::new(raw_path);
    let metadata = fs::metadata(path).map_err(|err| {
        AroError::Voice(format!(
            "{label} does not exist at {}: {err}",
            path.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(AroError::Voice(format!(
            "{label} path is not a file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    if require_executable {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(AroError::Voice(format!(
                "{label} is not executable: {}",
                path.display()
            )));
        }
    }
    #[cfg(not(unix))]
    let _ = require_executable;

    fs::canonicalize(path).map_err(|err| {
        AroError::Voice(format!(
            "{label} exists but could not be canonicalized at {}: {err}",
            path.display()
        ))
    })
}

fn validate_wake_word_request(request: &WakeWordDetectionRequest) -> AroResult<()> {
    validate_wav_mime(&request.mime_type)?;

    let size = request.audio_bytes.len();
    if size == 0 {
        return Err(AroError::Voice("wake-word WAV audio is empty".to_string()));
    }
    if size > MAX_WAV_BYTES {
        return Err(AroError::Voice(format!(
            "wake-word WAV audio is too large: {size} bytes received, maximum is {MAX_WAV_BYTES} bytes"
        )));
    }
    if size < MIN_WAV_BYTES {
        return Err(AroError::Voice(format!(
            "wake-word WAV audio is too small: {size} bytes received, minimum is {MIN_WAV_BYTES} bytes"
        )));
    }

    validate_wav_bytes(&request.audio_bytes)
}

fn validate_wake_word_threshold(threshold: f32) -> AroResult<f32> {
    if threshold.is_finite() && (0.0..=1.0).contains(&threshold) {
        Ok(threshold)
    } else {
        Err(AroError::Voice(
            "wake-word threshold must be between 0.0 and 1.0".to_string(),
        ))
    }
}

fn parse_model_float(name: &str, value: &str, line: usize) -> AroResult<f32> {
    let value = value.trim().parse::<f32>().map_err(|err| {
        AroError::Voice(format!(
            "wake-word model {name} value on line {line} is invalid: {err}"
        ))
    })?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(AroError::Voice(format!(
            "wake-word model {name} value on line {line} must be finite"
        )))
    }
}

fn clamp_score(score: f32) -> f32 {
    score.clamp(0.0, 1.0)
}

fn pcm16_wav_mean_abs(bytes: &[u8]) -> AroResult<f32> {
    validate_wav_bytes(bytes)?;

    let declared_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize + 8;
    let scan_limit = declared_size.min(bytes.len());
    let mut offset = 12;
    let mut audio_format = None;
    let mut bits_per_sample = None;
    let mut data_start = None;
    let mut data_end = None;

    while offset + 8 <= scan_limit {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let chunk_data_start = offset + 8;
        let chunk_data_end = chunk_data_start.checked_add(chunk_size).ok_or_else(|| {
            AroError::Voice("WAV chunk size overflows addressable memory".to_string())
        })?;

        if chunk_id == b"fmt " {
            audio_format = Some(u16::from_le_bytes([
                bytes[chunk_data_start],
                bytes[chunk_data_start + 1],
            ]));
            bits_per_sample = Some(u16::from_le_bytes([
                bytes[chunk_data_start + 14],
                bytes[chunk_data_start + 15],
            ]));
        } else if chunk_id == b"data" && data_start.is_none() {
            data_start = Some(chunk_data_start);
            data_end = Some(chunk_data_end);
        }

        offset = chunk_data_end + (chunk_size % 2);
    }

    if audio_format != Some(1) || bits_per_sample != Some(16) {
        return Err(AroError::Voice(
            "wake-word scorer supports 16-bit PCM WAV audio".to_string(),
        ));
    }

    let data_start = data_start
        .ok_or_else(|| AroError::Voice("WAV audio is missing a data chunk".to_string()))?;
    let data_end =
        data_end.ok_or_else(|| AroError::Voice("WAV audio is missing a data chunk".to_string()))?;
    let data = &bytes[data_start..data_end];
    if !data.len().is_multiple_of(2) {
        return Err(AroError::Voice(
            "wake-word PCM data must contain complete 16-bit samples".to_string(),
        ));
    }

    let mut total = 0.0_f32;
    let mut count = 0_usize;
    for sample in data.chunks_exact(2) {
        let sample = i16::from_le_bytes([sample[0], sample[1]]);
        total += (sample as f32).abs() / 32768.0;
        count += 1;
    }

    if count == 0 {
        Ok(0.0)
    } else {
        Ok(total / count as f32)
    }
}

fn validate_transcription_request(request: &TranscriptionRequest) -> AroResult<()> {
    validate_wav_mime(&request.mime_type)?;

    let size = request.audio_bytes.len();
    if size == 0 {
        return Err(AroError::Voice("WAV audio is empty".to_string()));
    }
    if size > MAX_WAV_BYTES {
        return Err(AroError::Voice(format!(
            "WAV audio is too large: {size} bytes received, maximum is {MAX_WAV_BYTES} bytes"
        )));
    }
    if size < MIN_WAV_BYTES {
        return Err(AroError::Voice(format!(
            "WAV audio is too small: {size} bytes received, minimum is {MIN_WAV_BYTES} bytes"
        )));
    }

    validate_wav_bytes(&request.audio_bytes)
}

fn validate_wav_mime(mime_type: &str) -> AroResult<()> {
    let mime = mime_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match mime.as_str() {
        "audio/wav" | "audio/wave" | "audio/x-wav" | "audio/vnd.wave" => Ok(()),
        "" => Err(AroError::Voice(
            "transcription audio MIME type is missing; expected audio/wav".to_string(),
        )),
        _ => Err(AroError::Voice(format!(
            "unsupported transcription audio MIME type {mime_type:?}; expected audio/wav"
        ))),
    }
}

fn validate_wav_bytes(bytes: &[u8]) -> AroResult<()> {
    if bytes.get(0..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return Err(AroError::Voice(
            "WAV audio must be a RIFF/WAVE file".to_string(),
        ));
    }

    let declared_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize + 8;
    if declared_size > bytes.len() {
        return Err(AroError::Voice(format!(
            "WAV RIFF header declares {declared_size} bytes but only {} bytes were received",
            bytes.len()
        )));
    }

    let scan_limit = declared_size.min(bytes.len());
    let mut offset = 12;
    let mut has_fmt = false;
    let mut has_data = false;
    while offset + 8 <= scan_limit {
        let chunk_id = &bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;
        let data_start = offset + 8;
        let data_end = data_start.checked_add(chunk_size).ok_or_else(|| {
            AroError::Voice("WAV chunk size overflows addressable memory".to_string())
        })?;
        if data_end > scan_limit {
            return Err(AroError::Voice(format!(
                "WAV chunk {} is truncated",
                String::from_utf8_lossy(chunk_id)
            )));
        }

        if chunk_id == b"fmt " {
            if chunk_size < 16 {
                return Err(AroError::Voice(
                    "WAV fmt chunk is too small to describe PCM audio".to_string(),
                ));
            }
            has_fmt = true;
        } else if chunk_id == b"data" {
            if chunk_size == 0 {
                return Err(AroError::Voice("WAV data chunk is empty".to_string()));
            }
            has_data = true;
        }

        offset = data_end + (chunk_size % 2);
    }

    if !has_fmt {
        return Err(AroError::Voice(
            "WAV audio is missing a fmt chunk".to_string(),
        ));
    }
    if !has_data {
        return Err(AroError::Voice(
            "WAV audio is missing a data chunk".to_string(),
        ));
    }

    Ok(())
}

fn validate_synthesis_request(request: &SynthesisRequest) -> AroResult<()> {
    if request.text.trim().is_empty() {
        return Err(AroError::Voice("synthesis text is empty".to_string()));
    }
    let size = request.text.len();
    if size > MAX_SYNTHESIS_TEXT_BYTES {
        return Err(AroError::Voice(format!(
            "synthesis text is too large: {size} bytes received, maximum is {MAX_SYNTHESIS_TEXT_BYTES} bytes"
        )));
    }
    Ok(())
}

fn transcription_language(language: Option<&str>) -> AroResult<String> {
    let language = language.unwrap_or("auto").trim();
    if language.is_empty() {
        return Ok("auto".to_string());
    }
    if language.len() > 32 {
        return Err(AroError::Voice(format!(
            "transcription language value is too long: {} bytes, maximum is 32 bytes",
            language.len()
        )));
    }
    if !language
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(AroError::Voice(format!(
            "transcription language contains unsupported characters: {language:?}"
        )));
    }
    Ok(language.to_string())
}

fn write_child_stdin(child: &mut Child, input: &[u8], runtime_name: &str) -> AroResult<()> {
    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(AroError::Voice(format!(
            "{runtime_name} stdin was not available"
        )));
    };

    if let Err(err) = stdin.write_all(input) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(AroError::Voice(format!(
            "failed to write synthesis text to {runtime_name} stdin: {err}"
        )));
    }
    drop(stdin);
    Ok(())
}

fn wait_with_timeout(
    mut child: Child,
    timeout: Duration,
    runtime_name: &str,
) -> AroResult<ExitStatus> {
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|err| {
            AroError::Voice(format!("failed while waiting for {runtime_name}: {err}"))
        })? {
            Some(status) => return Ok(status),
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AroError::Voice(format!(
                    "{runtime_name} timed out after {}; process was terminated",
                    format_duration(timeout)
                )));
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn read_lossy(path: &Path) -> AroResult<String> {
    fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
        .map_err(|err| {
            AroError::Voice(format!(
                "failed to read process output scratch file {}: {err}",
                path.display()
            ))
        })
}

fn format_transcription_process_failure(
    runtime_name: &str,
    status: ExitStatus,
    stderr: &str,
) -> String {
    let details = if !stderr.trim().is_empty() {
        truncate_output(stderr)
    } else {
        "process produced no stderr; stdout was redacted".to_string()
    };
    format!(
        "{runtime_name} exited with {}; {details}",
        format_exit_status(status)
    )
}

fn format_process_failure(
    runtime_name: &str,
    status: ExitStatus,
    stderr: &str,
    stdout: &str,
) -> String {
    let details = if !stderr.trim().is_empty() {
        truncate_output(stderr)
    } else if !stdout.trim().is_empty() {
        truncate_output(stdout)
    } else {
        "process produced no stderr or stdout".to_string()
    };
    format!(
        "{runtime_name} exited with {}; {details}",
        format_exit_status(status)
    )
}

fn truncate_output(output: &str) -> String {
    let trimmed = output.trim();
    let mut truncated: String = trimmed.chars().take(MAX_ERROR_OUTPUT_CHARS).collect();
    if trimmed.chars().count() > MAX_ERROR_OUTPUT_CHARS {
        truncated.push_str("...");
    }
    truncated
}

fn format_exit_status(status: ExitStatus) -> String {
    status
        .code()
        .map(|code| format!("exit code {code}"))
        .unwrap_or_else(|| status.to_string())
}

fn format_duration(duration: Duration) -> String {
    if duration.as_secs() > 0 {
        format!("{}s", duration.as_secs())
    } else {
        format!("{}ms", duration.as_millis())
    }
}

fn runtime_label(runtime: &VoiceRuntimeKind) -> &'static str {
    match runtime {
        VoiceRuntimeKind::Disabled => "disabled",
        VoiceRuntimeKind::WhisperCpp => "whisper.cpp",
        VoiceRuntimeKind::Piper => "Piper",
    }
}

fn executable_dir(binary: &Path) -> AroResult<&Path> {
    binary
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| {
            AroError::Voice(format!(
                "runtime binary has no parent directory: {}",
                binary.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aro_core::WakeWordSettings;
    use std::env;

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let path = env::temp_dir().join(format!("aro-voice-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("create test dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn is_ready_requires_enabled_correct_runtime_and_existing_files() {
        let dir = TestDir::new();
        let runtime_dir = dir.path().join("runtime");
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let whisper = write_script(
            &runtime_dir,
            "fake-whisper",
            "#!/bin/sh\necho fake transcript\n",
            "@echo off\necho fake transcript\nexit /b 0\n",
        );
        let model = runtime_dir.join("model.bin");
        fs::write(&model, b"model").expect("model file");

        let disabled = VoiceService::new(VoiceSettings::default(), dir.path().join("scratch"));
        assert!(!disabled.is_ready());

        let wrong_runtime = VoiceService::new(
            VoiceSettings {
                enabled: true,
                speech_to_text: VoiceRuntimeKind::Piper,
                text_to_speech: VoiceRuntimeKind::Disabled,
                whisper_binary: Some(path_string(&whisper)),
                whisper_model_path: Some(path_string(&model)),
                ..VoiceSettings::default()
            },
            dir.path().join("scratch"),
        );
        assert!(!wrong_runtime.is_ready());

        let missing_model = VoiceService::new(
            VoiceSettings {
                enabled: true,
                speech_to_text: VoiceRuntimeKind::WhisperCpp,
                text_to_speech: VoiceRuntimeKind::Disabled,
                whisper_binary: Some(path_string(&whisper)),
                whisper_model_path: Some(path_string(&runtime_dir.join("missing.bin"))),
                ..VoiceSettings::default()
            },
            dir.path().join("scratch"),
        );
        assert!(!missing_model.is_ready());

        let ready = VoiceService::new(
            VoiceSettings {
                enabled: true,
                speech_to_text: VoiceRuntimeKind::WhisperCpp,
                text_to_speech: VoiceRuntimeKind::Disabled,
                whisper_binary: Some(path_string(&whisper)),
                whisper_model_path: Some(path_string(&model)),
                ..VoiceSettings::default()
            },
            dir.path().join("scratch"),
        );
        assert!(ready.is_ready());
    }

    #[test]
    fn wake_word_detects_threshold_decisions_without_transcription() {
        let dir = TestDir::new();
        let runtime_dir = dir.path().join("runtime");
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let model = runtime_dir.join("wake-word.model");
        fs::write(&model, b"gain=1.0\nbias=0.0\n").expect("wake model");

        let service = VoiceService::new(
            VoiceSettings {
                enabled: true,
                wake_word: WakeWordSettings {
                    enabled: true,
                    runtime: WakeWordRuntimeKind::LocalModel,
                    model_path: Some(path_string(&model)),
                    threshold: 0.25,
                },
                ..VoiceSettings::default()
            },
            dir.path().join("scratch"),
        );

        let quiet = service
            .detect_wake_word(WakeWordDetectionRequest {
                audio_bytes: tiny_wav_with_samples(&[400, -400, 400, -400]),
                mime_type: "audio/wav".to_string(),
            })
            .expect("quiet wake-word score");
        let loud = service
            .detect_wake_word(WakeWordDetectionRequest {
                audio_bytes: tiny_wav_with_samples(&[16_000, -16_000, 16_000, -16_000]),
                mime_type: "audio/wav".to_string(),
            })
            .expect("loud wake-word score");

        assert!(!quiet.activated);
        assert!(quiet.score < quiet.threshold);
        assert!(loud.activated);
        assert!(loud.score >= loud.threshold);
        assert_eq!(loud.threshold, 0.25);
    }

    #[test]
    fn wake_word_reports_missing_model_path() {
        let dir = TestDir::new();
        let service = VoiceService::new(
            VoiceSettings {
                enabled: true,
                wake_word: WakeWordSettings {
                    enabled: true,
                    runtime: WakeWordRuntimeKind::LocalModel,
                    model_path: None,
                    threshold: 0.5,
                },
                ..VoiceSettings::default()
            },
            dir.path().join("scratch"),
        );

        assert!(!service.is_ready());
        let err = service
            .detect_wake_word(WakeWordDetectionRequest {
                audio_bytes: tiny_wav(),
                mime_type: "audio/wav".to_string(),
            })
            .expect_err("missing model path");

        assert!(err.to_string().contains("wake-word model path is missing"));
    }

    #[test]
    fn transcribe_validates_wav_mime_size_and_header() {
        let valid_wav = tiny_wav();

        let bad_mime = TranscriptionRequest {
            audio_bytes: valid_wav.clone(),
            mime_type: "audio/webm".to_string(),
            language: None,
        };
        let err = validate_transcription_request(&bad_mime).expect_err("mime rejected");
        assert!(err
            .to_string()
            .contains("unsupported transcription audio MIME type"));

        let oversized = TranscriptionRequest {
            audio_bytes: vec![0; MAX_WAV_BYTES + 1],
            mime_type: "audio/wav".to_string(),
            language: None,
        };
        let err = validate_transcription_request(&oversized).expect_err("size rejected");
        assert!(err.to_string().contains("WAV audio is too large"));

        let bad_header = TranscriptionRequest {
            audio_bytes: vec![0; MIN_WAV_BYTES],
            mime_type: "audio/wav".to_string(),
            language: None,
        };
        let err = validate_transcription_request(&bad_header).expect_err("header rejected");
        assert!(err.to_string().contains("RIFF/WAVE"));
    }

    #[test]
    fn transcribe_runs_fake_whisper_and_cleans_scratch() {
        let dir = TestDir::new();
        let runtime_dir = dir.path().join("runtime");
        let scratch_dir = dir.path().join("scratch");
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let whisper = write_script(
            &runtime_dir,
            "fake-whisper",
            "#!/bin/sh\necho fake transcript\n",
            "@echo off\necho fake transcript\nexit /b 0\n",
        );
        let model = runtime_dir.join("model.bin");
        fs::write(&model, b"model").expect("model file");

        let service = VoiceService::new(
            VoiceSettings {
                enabled: true,
                speech_to_text: VoiceRuntimeKind::WhisperCpp,
                text_to_speech: VoiceRuntimeKind::Disabled,
                whisper_binary: Some(path_string(&whisper)),
                whisper_model_path: Some(path_string(&model)),
                ..VoiceSettings::default()
            },
            &scratch_dir,
        );

        let result = service
            .transcribe(TranscriptionRequest {
                audio_bytes: tiny_wav(),
                mime_type: "audio/wav; codecs=1".to_string(),
                language: Some("en".to_string()),
            })
            .expect("transcription succeeds");

        assert_eq!(result.text, "fake transcript");
        assert_scratch_empty(&scratch_dir);
    }

    #[test]
    fn transcribe_failure_redacts_raw_stdout_transcript() {
        let dir = TestDir::new();
        let runtime_dir = dir.path().join("runtime");
        let scratch_dir = dir.path().join("scratch");
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let whisper = write_script(
            &runtime_dir,
            "fake-whisper-fail",
            WHISPER_FAIL_WITH_TRANSCRIPT_SH,
            WHISPER_FAIL_WITH_TRANSCRIPT_CMD,
        );
        let model = runtime_dir.join("model.bin");
        fs::write(&model, b"model").expect("model file");

        let service = VoiceService::new(
            VoiceSettings {
                enabled: true,
                speech_to_text: VoiceRuntimeKind::WhisperCpp,
                text_to_speech: VoiceRuntimeKind::Disabled,
                whisper_binary: Some(path_string(&whisper)),
                whisper_model_path: Some(path_string(&model)),
                ..VoiceSettings::default()
            },
            &scratch_dir,
        );

        let err = service
            .transcribe(TranscriptionRequest {
                audio_bytes: tiny_wav(),
                mime_type: "audio/wav".to_string(),
                language: None,
            })
            .expect_err("transcription fails");

        let err = err.to_string();
        assert!(err.contains("stdout was redacted"));
        assert!(!err.contains("raw wake transcript should stay private"));
        assert_scratch_empty(&scratch_dir);
    }

    #[test]
    fn synthesize_runs_fake_piper_and_cleans_scratch() {
        let dir = TestDir::new();
        let runtime_dir = dir.path().join("runtime");
        let scratch_dir = dir.path().join("scratch");
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let piper = write_script(
            &runtime_dir,
            "fake-piper",
            PIPER_SUCCESS_SH,
            PIPER_SUCCESS_CMD,
        );
        let voice = runtime_dir.join("voice.onnx");
        fs::write(&voice, b"voice").expect("voice file");

        let service = VoiceService::new(
            VoiceSettings {
                enabled: true,
                speech_to_text: VoiceRuntimeKind::Disabled,
                text_to_speech: VoiceRuntimeKind::Piper,
                piper_binary: Some(path_string(&piper)),
                piper_voice_path: Some(path_string(&voice)),
                ..VoiceSettings::default()
            },
            &scratch_dir,
        );

        let result = service
            .synthesize(SynthesisRequest {
                text: "bonjour".to_string(),
                voice_path: None,
                speaker_id: Some(1),
            })
            .expect("synthesis succeeds");

        assert_eq!(result.mime_type, "audio/wav");
        assert!(String::from_utf8_lossy(&result.audio_bytes).contains("fake wav"));
        assert_scratch_empty(&scratch_dir);
    }

    #[test]
    fn synthesize_includes_runtime_stderr_and_cleans_scratch_on_failure() {
        let dir = TestDir::new();
        let runtime_dir = dir.path().join("runtime");
        let scratch_dir = dir.path().join("scratch");
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        let piper = write_script(
            &runtime_dir,
            "fake-piper-fail",
            PIPER_FAIL_SH,
            PIPER_FAIL_CMD,
        );
        let voice = runtime_dir.join("voice.onnx");
        fs::write(&voice, b"voice").expect("voice file");

        let service = VoiceService::new(
            VoiceSettings {
                enabled: true,
                speech_to_text: VoiceRuntimeKind::Disabled,
                text_to_speech: VoiceRuntimeKind::Piper,
                piper_binary: Some(path_string(&piper)),
                piper_voice_path: Some(path_string(&voice)),
                ..VoiceSettings::default()
            },
            &scratch_dir,
        );

        let err = service
            .synthesize(SynthesisRequest {
                text: "bonjour".to_string(),
                voice_path: None,
                speaker_id: None,
            })
            .expect_err("synthesis fails");

        assert!(err.to_string().contains("intentional piper failure"));
        assert_scratch_empty(&scratch_dir);
    }

    #[test]
    fn wait_with_timeout_terminates_slow_process() {
        let mut command = slow_command();
        command.stdout(Stdio::null()).stderr(Stdio::null());
        let child = command.spawn().expect("spawn slow process");

        let err = wait_with_timeout(child, Duration::from_millis(50), "fake runtime")
            .expect_err("timeout expected");

        assert!(err.to_string().contains("timed out"));
    }

    const PIPER_SUCCESS_SH: &str = r#"#!/bin/sh
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output_file" ]; then
    shift
    out="${1:-}"
  fi
  shift || true
done
cat >/dev/null
if [ -z "$out" ]; then
  echo missing output_file >&2
  exit 2
fi
printf 'fake wav' > "$out"
"#;

    const PIPER_SUCCESS_CMD: &str = r#"@echo off
setlocal
set "OUT="
:parse
if "%~1"=="" goto done
if "%~1"=="--output_file" goto got_out
shift
goto parse
:got_out
shift
set "OUT=%~1"
shift
goto parse
:done
more > nul
if "%OUT%"=="" (
  echo missing output_file 1>&2
  exit /b 2
)
> "%OUT%" echo fake wav
exit /b 0
"#;

    const WHISPER_FAIL_WITH_TRANSCRIPT_SH: &str = r#"#!/bin/sh
echo raw wake transcript should stay private
exit 9
"#;

    const WHISPER_FAIL_WITH_TRANSCRIPT_CMD: &str = r#"@echo off
echo raw wake transcript should stay private
exit /b 9
"#;

    const PIPER_FAIL_SH: &str = r#"#!/bin/sh
cat >/dev/null
echo intentional piper failure >&2
exit 7
"#;

    const PIPER_FAIL_CMD: &str = r#"@echo off
more > nul
echo intentional piper failure 1>&2
exit /b 7
"#;

    fn tiny_wav() -> Vec<u8> {
        tiny_wav_with_samples(&[0])
    }

    fn tiny_wav_with_samples(samples: &[i16]) -> Vec<u8> {
        let mut data = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            data.extend_from_slice(&sample.to_le_bytes());
        }
        let data_len = data.len() as u32;
        let riff_size = 4 + (8 + 16) + (8 + data_len);
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&riff_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&16_000_u32.to_le_bytes());
        wav.extend_from_slice(&32_000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        wav.extend_from_slice(&data);
        wav
    }

    fn write_script(
        runtime_dir: &Path,
        name: &str,
        unix_body: &str,
        windows_body: &str,
    ) -> PathBuf {
        #[cfg(windows)]
        let _ = unix_body;
        #[cfg(not(windows))]
        let _ = windows_body;

        #[cfg(windows)]
        let script = {
            let path = runtime_dir.join(format!("{name}.cmd"));
            fs::write(&path, windows_body.replace('\n', "\r\n")).expect("write cmd script");
            path
        };

        #[cfg(not(windows))]
        let script = {
            use std::os::unix::fs::PermissionsExt;

            let path = runtime_dir.join(name);
            fs::write(&path, unix_body).expect("write shell script");
            let mut permissions = fs::metadata(&path).expect("script metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).expect("script permissions");
            path
        };

        script
    }

    fn path_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }

    fn assert_scratch_empty(scratch_dir: &Path) {
        let entries = if scratch_dir.exists() {
            fs::read_dir(scratch_dir)
                .expect("read scratch")
                .map(|entry| entry.expect("scratch entry").path())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        assert!(entries.is_empty(), "scratch not empty: {entries:?}");
    }

    #[cfg(windows)]
    fn slow_command() -> Command {
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Start-Sleep -Milliseconds 1000",
        ]);
        command
    }

    #[cfg(not(windows))]
    fn slow_command() -> Command {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 1"]);
        command
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioEnergyStats {
    pub rms_energy: f32,
    pub max_amplitude: i16,
    pub contains_speech: bool,
}

pub fn analyze_audio_energy(pcm_samples: &[i16], speech_threshold_rms: f32) -> AudioEnergyStats {
    if pcm_samples.is_empty() {
        return AudioEnergyStats {
            rms_energy: 0.0,
            max_amplitude: 0,
            contains_speech: false,
        };
    }

    let mut sum_sq = 0.0f64;
    let mut max_amp = 0i16;

    for &sample in pcm_samples {
        let val = sample as f64;
        sum_sq += val * val;
        let abs_val = sample.abs();
        if abs_val > max_amp {
            max_amp = abs_val;
        }
    }

    let rms = (sum_sq / pcm_samples.len() as f64).sqrt() as f32;
    AudioEnergyStats {
        rms_energy: rms,
        max_amplitude: max_amp,
        contains_speech: rms >= speech_threshold_rms,
    }
}

pub fn trim_wav_silence(wav_bytes: &[u8], threshold_rms: f32) -> AroResult<Vec<u8>> {
    if wav_bytes.len() < 44 {
        return Err(AroError::Voice("WAV header is too short".to_string()));
    }

    let header = &wav_bytes[..44];
    let pcm_data = &wav_bytes[44..];

    let samples_count = pcm_data.len() / 2;
    let mut samples = Vec::with_capacity(samples_count);
    for chunk in pcm_data.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(sample);
    }

    if samples.is_empty() {
        return Ok(wav_bytes.to_vec());
    }

    let frame_size = 160; // 10ms frames at 16kHz
    let mut start_frame = 0;
    let mut end_frame = samples.len() / frame_size;

    for (i, frame) in samples.chunks(frame_size).enumerate() {
        let stats = analyze_audio_energy(frame, threshold_rms);
        if stats.contains_speech {
            start_frame = i;
            break;
        }
    }

    for (i, frame) in samples.chunks(frame_size).enumerate().rev() {
        let stats = analyze_audio_energy(frame, threshold_rms);
        if stats.contains_speech {
            end_frame = i + 1;
            break;
        }
    }

    if start_frame >= end_frame {
        return Ok(header.to_vec());
    }

    let margin_frames = 10;
    let trimmed_start_sample = start_frame.saturating_sub(margin_frames) * frame_size;
    let total_frames = samples.len().div_ceil(frame_size);
    let trimmed_end_sample = (end_frame + margin_frames).min(total_frames) * frame_size;
    let trimmed_end_sample = trimmed_end_sample.min(samples.len());

    let trimmed_samples = &samples[trimmed_start_sample..trimmed_end_sample];

    let mut output = Vec::with_capacity(44 + trimmed_samples.len() * 2);
    output.extend_from_slice(header);

    let data_len = (trimmed_samples.len() * 2) as u32;
    output[40..44].copy_from_slice(&data_len.to_le_bytes());

    let file_len = data_len + 36;
    output[4..8].copy_from_slice(&file_len.to_le_bytes());

    for &sample in trimmed_samples {
        output.extend_from_slice(&sample.to_le_bytes());
    }

    Ok(output)
}
