use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{VoiceRuntimeKind, VoiceSettings, WakeWordRuntimeKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VoiceCapability {
    SpeechToText,
    TextToSpeech,
    WakeWord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VoiceReadinessIssueCode {
    VoiceDisabled,
    RuntimeDisabled,
    RuntimeUnavailable,
    UnsupportedRuntime,
    MissingBinaryPath,
    MissingModelPath,
    MissingVoicePath,
    InvalidThreshold,
    PathNotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum VoiceSettingKey {
    WhisperBinary,
    WhisperModelPath,
    PiperBinary,
    PiperVoicePath,
    WakeWordModelPath,
    WakeWordThreshold,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceReadinessIssue {
    pub code: VoiceReadinessIssueCode,
    pub capability: Option<VoiceCapability>,
    pub setting: Option<VoiceSettingKey>,
    pub message: String,
    pub path: Option<String>,
}

impl VoiceReadinessIssue {
    pub fn new(
        code: VoiceReadinessIssueCode,
        capability: Option<VoiceCapability>,
        setting: Option<VoiceSettingKey>,
        message: impl Into<String>,
        path: Option<String>,
    ) -> Self {
        Self {
            code,
            capability,
            setting,
            message: message.into(),
            path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VoiceCapabilityStatus {
    pub capability: VoiceCapability,
    pub runtime: VoiceRuntimeKind,
    pub ready: bool,
    pub issues: Vec<VoiceReadinessIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WakeWordStatus {
    pub enabled: bool,
    pub runtime: WakeWordRuntimeKind,
    pub ready: bool,
    pub model_path: Option<String>,
    pub threshold: f32,
    pub issues: Vec<VoiceReadinessIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatus {
    pub enabled: bool,
    pub ready: bool,
    pub speech_to_text: VoiceCapabilityStatus,
    pub text_to_speech: VoiceCapabilityStatus,
    pub wake_word: WakeWordStatus,
    pub issues: Vec<VoiceReadinessIssue>,
    pub checked_at: DateTime<Utc>,
}

impl VoiceStatus {
    pub fn from_settings(settings: &VoiceSettings) -> Self {
        let speech_to_text = speech_to_text_status(settings);
        let text_to_speech = text_to_speech_status(settings);
        let wake_word = wake_word_status(settings);
        let mut issues = Vec::new();

        if !settings.enabled {
            issues.push(VoiceReadinessIssue::new(
                VoiceReadinessIssueCode::VoiceDisabled,
                None,
                None,
                "Voice is disabled in settings.",
                None,
            ));
        }

        issues.extend(speech_to_text.issues.iter().cloned());
        issues.extend(text_to_speech.issues.iter().cloned());
        issues.extend(wake_word.issues.iter().cloned());

        Self {
            enabled: settings.enabled,
            ready: speech_to_text.ready || text_to_speech.ready || wake_word.ready,
            speech_to_text,
            text_to_speech,
            wake_word,
            issues,
            checked_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionRequest {
    pub audio_bytes: Vec<u8>,
    pub mime_type: String,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub runtime_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthesisRequest {
    pub text: String,
    pub voice_path: Option<String>,
    pub speaker_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthesisResult {
    pub audio_bytes: Vec<u8>,
    pub mime_type: String,
    pub runtime_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WakeWordDetectionRequest {
    pub audio_bytes: Vec<u8>,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WakeWordDetectionResult {
    pub activated: bool,
    pub score: f32,
    pub threshold: f32,
    pub model_path: Option<String>,
    pub runtime_detail: String,
}

fn speech_to_text_status(settings: &VoiceSettings) -> VoiceCapabilityStatus {
    let capability = VoiceCapability::SpeechToText;
    let runtime = settings.speech_to_text.clone();
    let mut issues = Vec::new();

    match settings.speech_to_text {
        VoiceRuntimeKind::Disabled => issues.push(VoiceReadinessIssue::new(
            VoiceReadinessIssueCode::RuntimeDisabled,
            Some(capability.clone()),
            None,
            "Speech-to-text is disabled in settings.",
            None,
        )),
        VoiceRuntimeKind::WhisperCpp => {
            if let Some(issue) = required_path_issue(
                &settings.whisper_binary,
                capability.clone(),
                VoiceSettingKey::WhisperBinary,
                VoiceReadinessIssueCode::MissingBinaryPath,
                "Whisper binary path is missing.",
            ) {
                issues.push(issue);
            }
            if let Some(issue) = required_path_issue(
                &settings.whisper_model_path,
                capability.clone(),
                VoiceSettingKey::WhisperModelPath,
                VoiceReadinessIssueCode::MissingModelPath,
                "Whisper model path is missing.",
            ) {
                issues.push(issue);
            }
        }
        VoiceRuntimeKind::Piper => issues.push(VoiceReadinessIssue::new(
            VoiceReadinessIssueCode::UnsupportedRuntime,
            Some(capability.clone()),
            None,
            "Piper cannot be used for speech-to-text.",
            None,
        )),
    }

    VoiceCapabilityStatus {
        capability,
        runtime,
        ready: settings.enabled && issues.is_empty(),
        issues,
    }
}

fn text_to_speech_status(settings: &VoiceSettings) -> VoiceCapabilityStatus {
    let capability = VoiceCapability::TextToSpeech;
    let runtime = settings.text_to_speech.clone();
    let mut issues = Vec::new();

    match settings.text_to_speech {
        VoiceRuntimeKind::Disabled => issues.push(VoiceReadinessIssue::new(
            VoiceReadinessIssueCode::RuntimeDisabled,
            Some(capability.clone()),
            None,
            "Text-to-speech is disabled in settings.",
            None,
        )),
        VoiceRuntimeKind::Piper => {
            if let Some(issue) = required_path_issue(
                &settings.piper_binary,
                capability.clone(),
                VoiceSettingKey::PiperBinary,
                VoiceReadinessIssueCode::MissingBinaryPath,
                "Piper binary path is missing.",
            ) {
                issues.push(issue);
            }
            if let Some(issue) = required_path_issue(
                &settings.piper_voice_path,
                capability.clone(),
                VoiceSettingKey::PiperVoicePath,
                VoiceReadinessIssueCode::MissingVoicePath,
                "Piper voice path is missing.",
            ) {
                issues.push(issue);
            }
        }
        VoiceRuntimeKind::WhisperCpp => issues.push(VoiceReadinessIssue::new(
            VoiceReadinessIssueCode::UnsupportedRuntime,
            Some(capability.clone()),
            None,
            "whisper.cpp cannot be used for text-to-speech.",
            None,
        )),
    }

    VoiceCapabilityStatus {
        capability,
        runtime,
        ready: settings.enabled && issues.is_empty(),
        issues,
    }
}

fn wake_word_status(settings: &VoiceSettings) -> WakeWordStatus {
    let capability = VoiceCapability::WakeWord;
    let runtime = settings.wake_word.runtime.clone();
    let mut issues = Vec::new();

    if !settings.wake_word.enabled {
        issues.push(VoiceReadinessIssue::new(
            VoiceReadinessIssueCode::RuntimeDisabled,
            Some(capability.clone()),
            None,
            "Wake-word detection is disabled in settings.",
            None,
        ));
    } else {
        match settings.wake_word.runtime {
            WakeWordRuntimeKind::Disabled => issues.push(VoiceReadinessIssue::new(
                VoiceReadinessIssueCode::RuntimeDisabled,
                Some(capability.clone()),
                None,
                "Wake-word runtime is disabled in settings.",
                None,
            )),
            WakeWordRuntimeKind::LocalModel => {
                if let Some(issue) = required_path_issue(
                    &settings.wake_word.model_path,
                    capability.clone(),
                    VoiceSettingKey::WakeWordModelPath,
                    VoiceReadinessIssueCode::MissingModelPath,
                    "Wake-word model path is missing.",
                ) {
                    issues.push(issue);
                }
                if let Some(issue) = wake_word_threshold_issue(settings.wake_word.threshold) {
                    issues.push(issue);
                }
            }
        }
    }

    WakeWordStatus {
        enabled: settings.wake_word.enabled,
        runtime,
        ready: settings.enabled && settings.wake_word.enabled && issues.is_empty(),
        model_path: settings.wake_word.model_path.clone(),
        threshold: settings.wake_word.threshold,
        issues,
    }
}

fn wake_word_threshold_issue(threshold: f32) -> Option<VoiceReadinessIssue> {
    if threshold.is_finite() && (0.0..=1.0).contains(&threshold) {
        return None;
    }

    Some(VoiceReadinessIssue::new(
        VoiceReadinessIssueCode::InvalidThreshold,
        Some(VoiceCapability::WakeWord),
        Some(VoiceSettingKey::WakeWordThreshold),
        "Wake-word threshold must be between 0.0 and 1.0.",
        None,
    ))
}

fn required_path_issue(
    value: &Option<String>,
    capability: VoiceCapability,
    setting: VoiceSettingKey,
    missing_code: VoiceReadinessIssueCode,
    missing_message: &'static str,
) -> Option<VoiceReadinessIssue> {
    let Some(path) = value
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    else {
        return Some(VoiceReadinessIssue::new(
            missing_code,
            Some(capability),
            Some(setting),
            missing_message,
            None,
        ));
    };

    if !Path::new(path).is_file() {
        return Some(VoiceReadinessIssue::new(
            VoiceReadinessIssueCode::PathNotFound,
            Some(capability),
            Some(setting),
            "Configured voice runtime file was not found.",
            Some(path.to_string()),
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue_codes(status: &VoiceStatus) -> Vec<VoiceReadinessIssueCode> {
        status
            .issues
            .iter()
            .map(|issue| issue.code.clone())
            .collect()
    }

    #[test]
    fn default_voice_status_reports_disabled_voice() {
        let status = VoiceStatus::from_settings(&VoiceSettings::default());

        assert!(!status.ready);
        assert!(!status.speech_to_text.ready);
        assert!(!status.text_to_speech.ready);
        assert!(issue_codes(&status).contains(&VoiceReadinessIssueCode::VoiceDisabled));
    }

    #[test]
    fn whisper_status_reports_missing_required_paths() {
        let settings = VoiceSettings {
            enabled: true,
            speech_to_text: VoiceRuntimeKind::WhisperCpp,
            ..VoiceSettings::default()
        };

        let status = VoiceStatus::from_settings(&settings);

        assert!(!status.speech_to_text.ready);
        assert!(status
            .speech_to_text
            .issues
            .iter()
            .any(|issue| issue.code == VoiceReadinessIssueCode::MissingBinaryPath));
        assert!(status
            .speech_to_text
            .issues
            .iter()
            .any(|issue| issue.code == VoiceReadinessIssueCode::MissingModelPath));
    }

    #[test]
    fn piper_status_is_ready_when_required_paths_exist() {
        let dir = std::env::temp_dir().join(format!("aro-voice-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp voice dir");
        let binary = dir.join("piper.exe");
        let voice = dir.join("voice.onnx");
        std::fs::write(&binary, []).expect("write piper binary placeholder");
        std::fs::write(&voice, []).expect("write piper voice placeholder");

        let settings = VoiceSettings {
            enabled: true,
            text_to_speech: VoiceRuntimeKind::Piper,
            piper_binary: Some(binary.to_string_lossy().to_string()),
            piper_voice_path: Some(voice.to_string_lossy().to_string()),
            ..VoiceSettings::default()
        };

        let status = VoiceStatus::from_settings(&settings);

        assert!(status.ready);
        assert!(status.text_to_speech.ready);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn wake_word_status_reports_missing_model_path() {
        let settings = VoiceSettings {
            enabled: true,
            wake_word: crate::WakeWordSettings {
                enabled: true,
                runtime: WakeWordRuntimeKind::LocalModel,
                model_path: None,
                threshold: 0.6,
            },
            ..VoiceSettings::default()
        };

        let status = VoiceStatus::from_settings(&settings);

        assert!(!status.wake_word.ready);
        assert_eq!(status.wake_word.threshold, 0.6);
        assert!(status.wake_word.issues.iter().any(|issue| issue.code
            == VoiceReadinessIssueCode::MissingModelPath
            && issue.setting == Some(VoiceSettingKey::WakeWordModelPath)));
    }

    #[test]
    fn wake_word_status_is_ready_when_model_and_threshold_are_valid() {
        let dir = std::env::temp_dir().join(format!("aro-wake-word-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp wake-word dir");
        let model = dir.join("wake-word.model");
        std::fs::write(&model, b"gain=1.0\nbias=0.0\n").expect("write wake model");

        let settings = VoiceSettings {
            enabled: true,
            wake_word: crate::WakeWordSettings {
                enabled: true,
                runtime: WakeWordRuntimeKind::LocalModel,
                model_path: Some(model.to_string_lossy().to_string()),
                threshold: 0.7,
            },
            ..VoiceSettings::default()
        };

        let status = VoiceStatus::from_settings(&settings);

        assert!(status.ready);
        assert!(status.wake_word.ready);
        assert_eq!(
            status.wake_word.model_path.as_deref(),
            settings.wake_word.model_path.as_deref()
        );
        assert!(status.wake_word.issues.is_empty());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn wake_word_status_reports_invalid_threshold() {
        let dir = std::env::temp_dir().join(format!("aro-wake-word-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp wake-word dir");
        let model = dir.join("wake-word.model");
        std::fs::write(&model, b"gain=1.0\nbias=0.0\n").expect("write wake model");

        let settings = VoiceSettings {
            enabled: true,
            wake_word: crate::WakeWordSettings {
                enabled: true,
                runtime: WakeWordRuntimeKind::LocalModel,
                model_path: Some(model.to_string_lossy().to_string()),
                threshold: 1.5,
            },
            ..VoiceSettings::default()
        };

        let status = VoiceStatus::from_settings(&settings);

        assert!(!status.wake_word.ready);
        assert!(status.wake_word.issues.iter().any(|issue| issue.code
            == VoiceReadinessIssueCode::InvalidThreshold
            && issue.setting == Some(VoiceSettingKey::WakeWordThreshold)));

        let _ = std::fs::remove_dir_all(dir);
    }
}
