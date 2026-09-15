export const DEFAULT_VOICE_SAMPLE_RATE = 16_000;
export const DEFAULT_VOICE_CHANNEL_COUNT = 1;
export const DEFAULT_SCRIPT_PROCESSOR_BUFFER_SIZE = 4_096;
export const DEFAULT_WAKE_WORD_WINDOW_SECONDS = 2.5;

const MIN_SAMPLE_RATE = 8_000;
const MAX_SAMPLE_RATE = 48_000;
const MIN_CHANNEL_COUNT = 1;
const MAX_CHANNEL_COUNT = 2;

export interface VoiceAudioConstraintOptions {
  autoGainControl?: boolean;
  channelCount?: number;
  echoCancellation?: boolean;
  noiseSuppression?: boolean;
  sampleRate?: number;
}

export function buildVoiceAudioConstraints(
  options: VoiceAudioConstraintOptions = {},
): MediaStreamConstraints {
  const sampleRate = clampInteger(
    options.sampleRate ?? DEFAULT_VOICE_SAMPLE_RATE,
    MIN_SAMPLE_RATE,
    MAX_SAMPLE_RATE,
  );
  const channelCount = clampInteger(
    options.channelCount ?? DEFAULT_VOICE_CHANNEL_COUNT,
    MIN_CHANNEL_COUNT,
    MAX_CHANNEL_COUNT,
  );

  return {
    audio: {
      autoGainControl: options.autoGainControl ?? true,
      channelCount: { ideal: channelCount },
      echoCancellation: options.echoCancellation ?? true,
      noiseSuppression: options.noiseSuppression ?? true,
      sampleRate: { ideal: sampleRate },
    },
  };
}

export function samplesForDuration(sampleRate: number, seconds: number): number {
  if (!Number.isFinite(seconds) || seconds <= 0) {
    throw new RangeError("seconds must be a positive number");
  }

  return Math.ceil(
    clampInteger(sampleRate, MIN_SAMPLE_RATE, MAX_SAMPLE_RATE) * seconds,
  );
}

function clampInteger(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }

  return Math.min(max, Math.max(min, Math.round(value)));
}
