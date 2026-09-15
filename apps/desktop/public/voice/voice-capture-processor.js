const PROCESSOR_NAME = "aro-voice-capture";
const DEFAULT_BATCH_FRAMES = 4096;
const MIN_BATCH_FRAMES = 128;
const MAX_BATCH_FRAMES = 65536;
const MIN_CHANNEL_COUNT = 1;
const MAX_CHANNEL_COUNT = 2;

function clampInteger(value, min, max) {
  if (!Number.isFinite(value)) {
    return min;
  }

  return Math.min(max, Math.max(min, Math.round(value)));
}

class AroVoiceCaptureProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();

    const processorOptions = options.processorOptions || {};
    this.channelCount = clampInteger(
      processorOptions.channelCount ?? MIN_CHANNEL_COUNT,
      MIN_CHANNEL_COUNT,
      MAX_CHANNEL_COUNT,
    );
    this.batchFrames = clampInteger(
      processorOptions.batchFrames ?? DEFAULT_BATCH_FRAMES,
      MIN_BATCH_FRAMES,
      MAX_BATCH_FRAMES,
    );
    this.pending = new Float32Array(this.batchFrames);
    this.pendingFrames = 0;
    this.sequence = 0;

    this.port.onmessage = (event) => {
      const data = event.data || {};

      if (data.type === "configure") {
        this.configure(data);
      } else if (data.type === "flush") {
        this.flush();
      }
    };
  }

  configure(data) {
    const nextChannelCount = clampInteger(
      data.channelCount ?? this.channelCount,
      MIN_CHANNEL_COUNT,
      MAX_CHANNEL_COUNT,
    );
    const nextBatchFrames = clampInteger(
      data.batchFrames ?? this.batchFrames,
      MIN_BATCH_FRAMES,
      MAX_BATCH_FRAMES,
    );

    this.channelCount = nextChannelCount;

    if (nextBatchFrames !== this.batchFrames) {
      this.flush();
      this.batchFrames = nextBatchFrames;
      this.pending = new Float32Array(this.batchFrames);
      this.pendingFrames = 0;
    }
  }

  process(inputs, outputs) {
    for (const output of outputs) {
      for (const channel of output) {
        channel.fill(0);
      }
    }

    const input = inputs[0];
    if (!input || input.length === 0 || !input[0]) {
      return true;
    }

    const frames = input[0].length;
    const channelCount = Math.min(input.length, this.channelCount);
    if (frames === 0 || channelCount === 0) {
      return true;
    }

    for (let frame = 0; frame < frames; frame += 1) {
      let sample = 0;

      for (let channel = 0; channel < channelCount; channel += 1) {
        sample += input[channel][frame] || 0;
      }

      this.pushSample(sample / channelCount);
    }

    return true;
  }

  pushSample(sample) {
    this.pending[this.pendingFrames] = sample;
    this.pendingFrames += 1;

    if (this.pendingFrames >= this.batchFrames) {
      this.flush();
    }
  }

  flush() {
    if (this.pendingFrames === 0) {
      return;
    }

    const samples = new Float32Array(this.pendingFrames);
    samples.set(this.pending.subarray(0, this.pendingFrames));
    this.pendingFrames = 0;
    this.postChunk(samples);
  }

  postChunk(samples) {
    let peak = 0;
    let sumSquares = 0;

    for (let index = 0; index < samples.length; index += 1) {
      const sample = samples[index];
      const abs = Math.abs(sample);
      peak = Math.max(peak, abs);
      sumSquares += sample * sample;
    }

    this.port.postMessage(
      {
        type: "voice-chunk",
        samples,
        rms: Math.sqrt(sumSquares / samples.length),
        peak,
        sampleRate,
        frames: samples.length,
        sequence: this.sequence,
        currentTime,
      },
      [samples.buffer],
    );
    this.sequence += 1;
  }
}

registerProcessor(PROCESSOR_NAME, AroVoiceCaptureProcessor);
