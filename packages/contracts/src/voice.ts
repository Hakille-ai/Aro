export interface VoiceCapability {
  name: string;
  available: boolean;
  modelKind?: string;
  languages?: string[];
}

export interface VoiceStatus {
  enabled: boolean;
  recording: boolean;
  speaking: boolean;
  sttAvailable: boolean;
  ttsAvailable: boolean;
  activeLanguage: string;
  selectedVoice?: string;
}

export interface SynthesisResult {
  audioUri?: string;
  audioBase64?: string;
  durationMs?: number;
  format?: string;
}

export interface TranscriptionResult {
  text: string;
  confidence?: number;
  durationMs?: number;
  language?: string;
}
