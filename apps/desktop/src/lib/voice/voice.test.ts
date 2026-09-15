import { existsSync, readFileSync } from "node:fs";

import {
  appendBoundedAudioChunk,
  buildVoiceAudioConstraints,
  buildVoiceCaptureProcessorOptions,
  DEFAULT_STT_MODEL_PROFILES,
  DEFAULT_TTS_MODEL_PROFILES,
  DEFAULT_WAKE_MODEL_PROFILE,
  EMPTY_AUDIO_CHUNK_BUFFER,
  encodeWav,
  findVoiceModelProfile,
  getVoiceModelProfiles,
  isWhisperHallucination,
  isVoiceCaptureChunkMessage,
  isVoiceCaptureErrorMessage,
  matchesWakeWord,
  mergeAudioChunks,
  RollingAudioBuffer,
  samplesForDuration,
  samplesForWakeProfile,
  trimAudioChunksToMaxSamples,
  voiceCaptureChunkFromEvent,
  VOICE_CAPTURE_PROCESSOR_NAME,
  VOICE_CAPTURE_WORKLET_URL,
} from ".";

declare const describe: (name: string, fn: () => void) => void;
declare const it: (name: string, fn: () => void) => void;
declare const expect: (actual: unknown) => {
  toBe(expected: unknown): void;
  toEqual(expected: unknown): void;
  toMatchObject(expected: unknown): void;
  toThrow(expected?: unknown): void;
};

function ascii(bytes: Uint8Array, start: number, length: number): string {
  return String.fromCharCode(...bytes.slice(start, start + length));
}

describe("voice wake word matching", () => {
  it("matches ARO variants despite case, accents, and punctuation", () => {
    expect(matchesWakeWord("Hey, ARO!")).toBe(true);
    expect(matchesWakeWord("haro tu m'entends ?")).toBe(true);
    expect(matchesWakeWord("Arrow, lance l'ecoute")).toBe(true);
    expect(matchesWakeWord("Aroo")).toBe(true);
  });

  it("avoids embedded matches unless explicitly allowed", () => {
    expect(matchesWakeWord("caroline arrive")).toBe(false);
    expect(matchesWakeWord("caroline arrive", { allowEmbeddedAro: true })).toBe(true);
  });
});

describe("voice hallucination filtering", () => {
  it("filters silence markers and common Whisper filler phrases", () => {
    expect(isWhisperHallucination("")).toBe(true);
    expect(isWhisperHallucination("[Musique]")).toBe(true);
    expect(isWhisperHallucination("(silence)")).toBe(true);
    expect(isWhisperHallucination("Merci beaucoup.")).toBe(true);
    expect(isWhisperHallucination("Sous-titres par Amara")).toBe(true);
    expect(isWhisperHallucination("...")).toBe(true);
  });

  it("keeps meaningful transcriptions", () => {
    expect(isWhisperHallucination("Merci pour le resume de la reunion")).toBe(false);
    expect(isWhisperHallucination("Lance une recherche sur le contrat")).toBe(false);
  });

  it("filters caption watermarks and lone music symbols", () => {
    expect(isWhisperHallucination("♪")).toBe(true);
    expect(isWhisperHallucination("♪ ♪")).toBe(true);
    expect(isWhisperHallucination("…")).toBe(true);
    expect(isWhisperHallucination("Thank you for watching.")).toBe(true);
    expect(isWhisperHallucination("Thanks for watching")).toBe(true);
    expect(isWhisperHallucination("Sous-titrage Société Radio-Canada")).toBe(true);
    expect(isWhisperHallucination("amara.org")).toBe(true);
  });
});

describe("voice WAV helpers", () => {
  it("merges Float32 chunks in order", () => {
    const merged = mergeAudioChunks([
      new Float32Array([0, 0.25]),
      new Float32Array([-0.5, 1]),
    ]);

    expect(Array.from(merged)).toEqual([0, 0.25, -0.5, 1]);
  });

  it("encodes mono PCM16 WAV with the expected RIFF header and clamped samples", () => {
    const wav = encodeWav([new Float32Array([-2, 0, 0.5, 2])], 16_000);
    const view = new DataView(wav.buffer);

    expect(ascii(wav, 0, 4)).toBe("RIFF");
    expect(ascii(wav, 8, 4)).toBe("WAVE");
    expect(ascii(wav, 12, 4)).toBe("fmt ");
    expect(ascii(wav, 36, 4)).toBe("data");
    expect(view.getUint32(24, true)).toBe(16_000);
    expect(view.getUint32(40, true)).toBe(8);
    expect(view.getInt16(44, true)).toBe(-32_768);
    expect(view.getInt16(46, true)).toBe(0);
    expect(view.getInt16(48, true)).toBe(16_383);
    expect(view.getInt16(50, true)).toBe(32_767);
  });

  it("rejects invalid sample rates", () => {
    expect(() => encodeWav([], 0)).toThrow(RangeError);
  });
});

describe("voice bounded audio buffers", () => {
  it("keeps only the newest samples and slices partial chunks when needed", () => {
    const buffer = trimAudioChunksToMaxSamples(
      [
        new Float32Array([1, 2, 3]),
        new Float32Array([4, 5, 6]),
        new Float32Array([7, 8]),
      ],
      5,
    );

    expect(buffer.totalSamples).toBe(5);
    expect(Array.from(mergeAudioChunks(buffer.chunks))).toEqual([4, 5, 6, 7, 8]);
  });

  it("appends chunks without mutating the previous buffer", () => {
    const first = appendBoundedAudioChunk(EMPTY_AUDIO_CHUNK_BUFFER, new Float32Array([1, 2]), 3);
    const second = appendBoundedAudioChunk(first, new Float32Array([3, 4]), 3);

    expect(Array.from(mergeAudioChunks(first.chunks))).toEqual([1, 2]);
    expect(Array.from(mergeAudioChunks(second.chunks))).toEqual([2, 3, 4]);
  });
});

describe("voice rolling audio buffers", () => {
  it("rolls over capacity while preserving the newest samples", () => {
    const buffer = new RollingAudioBuffer(4);

    buffer.append(new Float32Array([1, 2, 3]));
    const snapshot = buffer.append(new Float32Array([4, 5, 6]));

    expect(snapshot.totalSamples).toBe(4);
    expect(snapshot.maxSamples).toBe(4);
    expect(snapshot.remainingSamples).toBe(0);
    expect(buffer.isFull).toBe(true);
    expect(Array.from(buffer.toFloat32Array())).toEqual([3, 4, 5, 6]);
  });

  it("returns defensive snapshots and supports resize and clear", () => {
    const buffer = new RollingAudioBuffer(5, [
      new Float32Array([1, 2]),
      new Float32Array([3, 4]),
    ]);
    const snapshot = buffer.snapshot();

    snapshot.chunks[0][0] = 99;
    expect(Array.from(buffer.toFloat32Array())).toEqual([1, 2, 3, 4]);

    buffer.resize(2);
    expect(Array.from(buffer.toFloat32Array())).toEqual([3, 4]);

    const cleared = buffer.clear();
    expect(cleared.totalSamples).toBe(0);
    expect(cleared.remainingSamples).toBe(2);
  });
});

describe("voice audio constraints", () => {
  it("builds browser audio constraints optimized for voice capture", () => {
    expect(buildVoiceAudioConstraints()).toEqual({
      audio: {
        autoGainControl: true,
        channelCount: { ideal: 1 },
        echoCancellation: true,
        noiseSuppression: true,
        sampleRate: { ideal: 16_000 },
      },
    });
  });

  it("clamps numeric constraints and converts durations to sample counts", () => {
    expect(buildVoiceAudioConstraints({ channelCount: 8, sampleRate: 96_000 })).toMatchObject({
      audio: {
        channelCount: { ideal: 2 },
        sampleRate: { ideal: 48_000 },
      },
    });
    expect(samplesForDuration(16_000, 2.5)).toBe(40_000);
  });
});

describe("voice AudioWorklet capture helpers", () => {
  it("points to the public capture processor asset", () => {
    const asset = new URL("../../../public/voice/voice-capture-processor.js", import.meta.url);
    const source = readFileSync(asset, "utf8");

    expect(VOICE_CAPTURE_PROCESSOR_NAME).toBe("aro-voice-capture");
    expect(VOICE_CAPTURE_WORKLET_URL).toBe("/voice/voice-capture-processor.js");
    expect(existsSync(asset)).toBe(true);
    expect(source.includes(VOICE_CAPTURE_PROCESSOR_NAME)).toBe(true);
  });

  it("clamps capture processor options for browser-safe worklet batches", () => {
    expect(buildVoiceCaptureProcessorOptions({ batchFrames: 16, channelCount: 9 })).toEqual({
      batchFrames: 128,
      channelCount: 2,
    });
    expect(buildVoiceCaptureProcessorOptions({ batchFrames: 8_192, channelCount: 1 })).toEqual({
      batchFrames: 8_192,
      channelCount: 1,
    });
  });

  it("recognizes typed worklet messages", () => {
    const chunk = {
      currentTime: 1.25,
      frames: 2,
      peak: 0.5,
      rms: 0.395,
      sampleRate: 48_000,
      samples: new Float32Array([0.25, -0.5]),
      sequence: 7,
      type: "voice-chunk" as const,
    };
    const error = { message: "capture failed", type: "voice-error" as const };

    expect(isVoiceCaptureChunkMessage(chunk)).toBe(true);
    expect(voiceCaptureChunkFromEvent({ data: chunk } as MessageEvent<unknown>)).toBe(chunk);
    expect(isVoiceCaptureChunkMessage({ ...chunk, frames: 3 })).toBe(false);
    expect(isVoiceCaptureErrorMessage(error)).toBe(true);
  });
});

describe("voice model profiles", () => {
  it("exposes typed defaults for wake word, STT, and TTS", () => {
    expect(DEFAULT_WAKE_MODEL_PROFILE.capability).toBe("wake-word");
    expect(DEFAULT_WAKE_MODEL_PROFILE.runtime).toBe("browser-audio-worklet");
    expect(DEFAULT_STT_MODEL_PROFILES[0].runtime).toBe("whisper-cpp");
    expect(DEFAULT_TTS_MODEL_PROFILES[0].runtime).toBe("piper");
    expect(samplesForWakeProfile()).toBe(40_000);
  });

  it("filters and finds profiles by capability", () => {
    const sttProfiles = getVoiceModelProfiles("speech-to-text");
    const sttProfile = findVoiceModelProfile("speech-to-text", "whisper-cpp-local");

    expect(sttProfiles.length).toBe(1);
    expect(sttProfile?.binarySetting).toBe("whisperBinary");
    expect(findVoiceModelProfile("text-to-speech", "missing")).toBe(undefined);
  });
});
