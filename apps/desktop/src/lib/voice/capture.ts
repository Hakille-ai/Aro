import {
  DEFAULT_SCRIPT_PROCESSOR_BUFFER_SIZE,
  DEFAULT_VOICE_CHANNEL_COUNT,
} from "./constraints";

export const VOICE_CAPTURE_PROCESSOR_NAME = "aro-voice-capture";
export const VOICE_CAPTURE_WORKLET_URL = "/voice/voice-capture-processor.js";

const MIN_WORKLET_BATCH_FRAMES = 128;
const MAX_WORKLET_BATCH_FRAMES = 65_536;
const MIN_WORKLET_CHANNEL_COUNT = 1;
const MAX_WORKLET_CHANNEL_COUNT = 2;

export interface VoiceCaptureProcessorOptions {
  batchFrames?: number;
  channelCount?: number;
}

export interface VoiceCaptureProcessorConfig {
  batchFrames: number;
  channelCount: number;
}

export interface VoiceCaptureChunkMessage {
  type: "voice-chunk";
  samples: Float32Array;
  rms: number;
  peak: number;
  sampleRate: number;
  frames: number;
  sequence: number;
  currentTime: number;
}

export interface VoiceCaptureErrorMessage {
  type: "voice-error";
  message: string;
}

export type VoiceCaptureWorkletMessage =
  | VoiceCaptureChunkMessage
  | VoiceCaptureErrorMessage;

export interface VoiceCaptureSessionOptions extends VoiceCaptureProcessorOptions {
  audioContext?: AudioContext;
  autoResume?: boolean;
  closeContextOnStop?: boolean;
  monitorGain?: number;
  onChunk?: (chunk: VoiceCaptureChunkMessage) => void;
  onError?: (error: Error) => void;
  processorUrl?: string;
  stopTracksOnStop?: boolean;
}

export interface VoiceCaptureSession {
  context: AudioContext;
  flush(): void;
  monitor: GainNode;
  processor: AudioWorkletNode;
  sampleRate: number;
  source: MediaStreamAudioSourceNode;
  stop(): Promise<void>;
  updateOptions(options: VoiceCaptureProcessorOptions): void;
}

export function buildVoiceCaptureProcessorOptions(
  options: VoiceCaptureProcessorOptions = {},
): VoiceCaptureProcessorConfig {
  return {
    batchFrames: clampInteger(
      options.batchFrames ?? DEFAULT_SCRIPT_PROCESSOR_BUFFER_SIZE,
      MIN_WORKLET_BATCH_FRAMES,
      MAX_WORKLET_BATCH_FRAMES,
    ),
    channelCount: clampInteger(
      options.channelCount ?? DEFAULT_VOICE_CHANNEL_COUNT,
      MIN_WORKLET_CHANNEL_COUNT,
      MAX_WORKLET_CHANNEL_COUNT,
    ),
  };
}

export function isVoiceCaptureChunkMessage(
  value: unknown,
): value is VoiceCaptureChunkMessage {
  if (!isRecord(value) || value.type !== "voice-chunk") {
    return false;
  }

  return (
    value.samples instanceof Float32Array &&
    value.frames === value.samples.length &&
    isFiniteNonNegativeNumber(value.rms) &&
    isFiniteNonNegativeNumber(value.peak) &&
    isPositiveInteger(value.sampleRate) &&
    isPositiveInteger(value.frames) &&
    isNonNegativeInteger(value.sequence) &&
    isFiniteNonNegativeNumber(value.currentTime)
  );
}

export function isVoiceCaptureErrorMessage(
  value: unknown,
): value is VoiceCaptureErrorMessage {
  return isRecord(value) && value.type === "voice-error" && typeof value.message === "string";
}

export function voiceCaptureChunkFromEvent(
  event: MessageEvent<unknown>,
): VoiceCaptureChunkMessage | null {
  return isVoiceCaptureChunkMessage(event.data) ? event.data : null;
}

export async function createVoiceCaptureSession(
  stream: MediaStream,
  options: VoiceCaptureSessionOptions = {},
): Promise<VoiceCaptureSession> {
  const context = options.audioContext ?? new AudioContext();
  const ownsContext = options.audioContext === undefined;
  const closeContextOnStop = options.closeContextOnStop ?? ownsContext;
  const processorUrl = options.processorUrl ?? VOICE_CAPTURE_WORKLET_URL;
  const processorOptions = buildVoiceCaptureProcessorOptions(options);

  try {
    if (!context.audioWorklet) {
      throw new Error("AudioWorklet is not available in this browser context");
    }

    await context.audioWorklet.addModule(processorUrl);

    if (options.autoResume !== false && context.state === "suspended") {
      await context.resume();
    }

    const source = context.createMediaStreamSource(stream);
    const processor = new AudioWorkletNode(context, VOICE_CAPTURE_PROCESSOR_NAME, {
      numberOfInputs: 1,
      numberOfOutputs: 1,
      outputChannelCount: [1],
      processorOptions,
    });
    const monitor = context.createGain();
    monitor.gain.value = options.monitorGain ?? 0;

    processor.port.onmessage = (event: MessageEvent<unknown>) => {
      if (isVoiceCaptureChunkMessage(event.data)) {
        options.onChunk?.(event.data);
      } else if (isVoiceCaptureErrorMessage(event.data)) {
        options.onError?.(new Error(event.data.message));
      }
    };

    source.connect(processor);
    processor.connect(monitor);
    monitor.connect(context.destination);

    let stopped = false;

    return {
      context,
      flush: () => {
        processor.port.postMessage({ type: "flush" });
      },
      monitor,
      processor,
      sampleRate: context.sampleRate,
      source,
      stop: async () => {
        if (stopped) {
          return;
        }

        stopped = true;
        processor.port.onmessage = null;
        processor.port.postMessage({ type: "flush" });
        processor.disconnect();
        source.disconnect();
        monitor.disconnect();

        if (options.stopTracksOnStop === true) {
          stream.getTracks().forEach((track) => track.stop());
        }

        if (closeContextOnStop && context.state !== "closed") {
          await context.close();
        }
      },
      updateOptions: (nextOptions: VoiceCaptureProcessorOptions) => {
        processor.port.postMessage({
          type: "configure",
          ...buildVoiceCaptureProcessorOptions(nextOptions),
        });
      },
    };
  } catch (error) {
    if (ownsContext && context.state !== "closed") {
      await context.close().catch(() => undefined);
    }

    throw error;
  }
}

function clampInteger(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }

  return Math.min(max, Math.max(min, Math.round(value)));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isPositiveInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0;
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0;
}

function isFiniteNonNegativeNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value) && value >= 0;
}
