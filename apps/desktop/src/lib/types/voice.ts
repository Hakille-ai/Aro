export type VoiceRuntimeKind = "disabled" | "whisper-cpp" | "piper";

export type WakeWordRuntimeKind = "disabled" | "local-model";

export type VoiceModelRuntimeKind = VoiceRuntimeKind | WakeWordRuntimeKind;

export type VoiceCapability = "speech-to-text" | "text-to-speech" | "wake-word";

export type VoiceModelCapability = VoiceCapability | "wake-word";

export type VoiceReadinessIssueCode =
  | "voice-disabled"
  | "runtime-disabled"
  | "runtime-unavailable"
  | "unsupported-runtime"
  | "missing-binary-path"
  | "missing-model-path"
  | "missing-voice-path"
  | "invalid-threshold"
  | "path-not-found";

export type VoiceSettingKey =
  | "whisperBinary"
  | "whisperModelPath"
  | "piperBinary"
  | "piperVoicePath"
  | "wakeWordModelPath"
  | "wakeWordThreshold";

export interface VoiceSettings {
  enabled: boolean;
  speechToText: VoiceRuntimeKind;
  textToSpeech: VoiceRuntimeKind;
  whisperBinary?: string | null;
  whisperModelPath?: string | null;
  piperBinary?: string | null;
  piperVoicePath?: string | null;
  wakeWord: WakeWordSettings;
}

export interface WakeWordSettings {
  enabled: boolean;
  runtime: WakeWordRuntimeKind;
  modelPath?: string | null;
  threshold: number;
}

export interface VoiceReadinessIssue {
  code: VoiceReadinessIssueCode;
  capability?: VoiceCapability | null;
  setting?: VoiceSettingKey | null;
  message: string;
  path?: string | null;
}

export interface VoiceCapabilityStatus {
  capability: "speech-to-text" | "text-to-speech";
  runtime: VoiceRuntimeKind;
  ready: boolean;
  issues: VoiceReadinessIssue[];
}

export interface WakeWordStatus {
  enabled: boolean;
  runtime: WakeWordRuntimeKind;
  ready: boolean;
  modelPath?: string | null;
  threshold: number;
  issues: VoiceReadinessIssue[];
}

export interface VoiceStatus {
  enabled: boolean;
  ready: boolean;
  speechToText: VoiceCapabilityStatus;
  textToSpeech: VoiceCapabilityStatus;
  wakeWord: WakeWordStatus;
  issues: VoiceReadinessIssue[];
  checkedAt: string;
}

export interface VoiceModelReadiness {
  capability: VoiceModelCapability;
  runtime: VoiceModelRuntimeKind;
  ready: boolean;
  optional: boolean;
  binaryPath?: string | null;
  modelPath?: string | null;
  detail: string;
  issues: VoiceReadinessIssue[];
}

export interface VoiceModelsStatus {
  enabled: boolean;
  ready: boolean;
  voiceStatus: VoiceStatus;
  speechToText: VoiceModelReadiness;
  textToSpeech: VoiceModelReadiness;
  wakeWord: VoiceModelReadiness;
  models: VoiceModelReadiness[];
  checkedAt: string;
}

export interface WakeWordDetectionOptions {
  language?: string | null;
  variants?: string[] | null;
  allowEmbeddedAro?: boolean | null;
}

export interface WakeWordDetectionResult {
  detected: boolean;
  text: string;
  matchedVariant?: string | null;
  runtimeDetail: string;
  score?: number | null;
  threshold?: number | null;
  checkedAt: string;
}

export interface VoiceProfile {
  id: string;
  cloudId?: string;
  name: string;
  description: string;
  path: string;
  speakerId?: number | null;
  language: string;
  avatarColor: string;
  isDefault?: boolean;
}

export interface TranscriptionResult {
  text: string;
  runtimeDetail: string;
}

export interface SynthesisResult {
  audioBytes: number[];
  mimeType: string;
  runtimeDetail: string;
}

// â”€â”€â”€ Arena Mode â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
