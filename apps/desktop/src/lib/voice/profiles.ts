import { VOICE_CAPTURE_WORKLET_URL } from "./capture";
import {
  DEFAULT_VOICE_SAMPLE_RATE,
  DEFAULT_WAKE_WORD_WINDOW_SECONDS,
  samplesForDuration,
} from "./constraints";

export type VoiceModelCapability = "wake-word" | "speech-to-text" | "text-to-speech";
export type VoiceModelRuntime = "browser-audio-worklet" | "whisper-cpp" | "piper";
export type VoiceModelLanguage = "fr" | "en" | "multi";
export type VoiceModelSettingKey =
  | "whisperBinary"
  | "whisperModelPath"
  | "piperBinary"
  | "piperVoicePath";

interface BaseVoiceModelProfile<
  Capability extends VoiceModelCapability,
  Runtime extends VoiceModelRuntime,
> {
  capability: Capability;
  description: string;
  id: string;
  label: string;
  languages: readonly VoiceModelLanguage[];
  local: boolean;
  recommended: boolean;
  runtime: Runtime;
  sampleRate: number;
}

export interface WakeWordModelProfile
  extends BaseVoiceModelProfile<"wake-word", "browser-audio-worklet"> {
  checkIntervalMs: number;
  minBufferedChunks: number;
  variants: readonly string[];
  windowSeconds: number;
  workletUrl: string;
}

export interface SpeechToTextModelProfile
  extends BaseVoiceModelProfile<"speech-to-text", "whisper-cpp"> {
  audioMimeType: "audio/wav";
  binarySetting: Extract<VoiceModelSettingKey, "whisperBinary">;
  defaultModelFilename: string;
  maxRecordingSeconds: number;
  modelPathSetting: Extract<VoiceModelSettingKey, "whisperModelPath">;
}

export interface TextToSpeechModelProfile
  extends BaseVoiceModelProfile<"text-to-speech", "piper"> {
  binarySetting: Extract<VoiceModelSettingKey, "piperBinary">;
  outputMimeType: "audio/wav";
  supportsSpeakerId: boolean;
  voicePathSetting: Extract<VoiceModelSettingKey, "piperVoicePath">;
}

export type VoiceModelProfile =
  | WakeWordModelProfile
  | SpeechToTextModelProfile
  | TextToSpeechModelProfile;

export const DEFAULT_WAKE_MODEL_PROFILE: WakeWordModelProfile = {
  capability: "wake-word",
  checkIntervalMs: 1_500,
  description: "Browser-side rolling microphone window used before local STT wake matching.",
  id: "aro-browser-wake",
  label: "ARO wake window",
  languages: ["fr", "en"],
  local: true,
  minBufferedChunks: 5,
  recommended: true,
  runtime: "browser-audio-worklet",
  sampleRate: DEFAULT_VOICE_SAMPLE_RATE,
  variants: ["aro", "haro", "arrow", "aero"],
  windowSeconds: DEFAULT_WAKE_WORD_WINDOW_SECONDS,
  workletUrl: VOICE_CAPTURE_WORKLET_URL,
};

export const DEFAULT_STT_MODEL_PROFILES = [
  {
    audioMimeType: "audio/wav",
    binarySetting: "whisperBinary",
    capability: "speech-to-text",
    defaultModelFilename: "ggml-base.bin",
    description: "Local whisper.cpp speech recognition from mono PCM WAV capture.",
    id: "whisper-cpp-local",
    label: "Whisper.cpp local STT",
    languages: ["multi"],
    local: true,
    maxRecordingSeconds: 45,
    modelPathSetting: "whisperModelPath",
    recommended: true,
    runtime: "whisper-cpp",
    sampleRate: DEFAULT_VOICE_SAMPLE_RATE,
  },
] as const satisfies readonly SpeechToTextModelProfile[];

export const DEFAULT_TTS_MODEL_PROFILES = [
  {
    binarySetting: "piperBinary",
    capability: "text-to-speech",
    description: "Local Piper synthesis from the configured ONNX voice.",
    id: "piper-local",
    label: "Piper local TTS",
    languages: ["fr", "en"],
    local: true,
    outputMimeType: "audio/wav",
    recommended: true,
    runtime: "piper",
    sampleRate: DEFAULT_VOICE_SAMPLE_RATE,
    supportsSpeakerId: true,
    voicePathSetting: "piperVoicePath",
  },
] as const satisfies readonly TextToSpeechModelProfile[];

export const DEFAULT_VOICE_MODEL_PROFILES = [
  DEFAULT_WAKE_MODEL_PROFILE,
  ...DEFAULT_STT_MODEL_PROFILES,
  ...DEFAULT_TTS_MODEL_PROFILES,
] as const satisfies readonly VoiceModelProfile[];

export function getVoiceModelProfiles(): readonly VoiceModelProfile[];
export function getVoiceModelProfiles(
  capability: "wake-word",
): readonly WakeWordModelProfile[];
export function getVoiceModelProfiles(
  capability: "speech-to-text",
): readonly SpeechToTextModelProfile[];
export function getVoiceModelProfiles(
  capability: "text-to-speech",
): readonly TextToSpeechModelProfile[];
export function getVoiceModelProfiles(
  capability?: VoiceModelCapability,
): readonly VoiceModelProfile[] {
  if (!capability) {
    return DEFAULT_VOICE_MODEL_PROFILES;
  }

  return DEFAULT_VOICE_MODEL_PROFILES.filter((profile) => profile.capability === capability);
}

export function findVoiceModelProfile(
  capability: "wake-word",
  id: string,
): WakeWordModelProfile | undefined;
export function findVoiceModelProfile(
  capability: "speech-to-text",
  id: string,
): SpeechToTextModelProfile | undefined;
export function findVoiceModelProfile(
  capability: "text-to-speech",
  id: string,
): TextToSpeechModelProfile | undefined;
export function findVoiceModelProfile(
  capability: VoiceModelCapability,
  id: string,
): VoiceModelProfile | undefined {
  return DEFAULT_VOICE_MODEL_PROFILES.find(
    (profile) => profile.capability === capability && profile.id === id,
  );
}

export function samplesForWakeProfile(
  profile: WakeWordModelProfile = DEFAULT_WAKE_MODEL_PROFILE,
): number {
  return samplesForDuration(profile.sampleRate, profile.windowSeconds);
}
