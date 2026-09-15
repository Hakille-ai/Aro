export interface AudioChunkBuffer {
  chunks: readonly Float32Array[];
  totalSamples: number;
}

export interface RollingAudioBufferSnapshot extends AudioChunkBuffer {
  maxSamples: number;
  remainingSamples: number;
}

export const EMPTY_AUDIO_CHUNK_BUFFER: AudioChunkBuffer = {
  chunks: [],
  totalSamples: 0,
};

export class RollingAudioBuffer {
  #chunks: Float32Array[] = [];
  #maxSamples: number;
  #totalSamples = 0;

  constructor(maxSamples: number, chunks: readonly Float32Array[] = []) {
    assertValidMaxSamples(maxSamples);
    this.#maxSamples = maxSamples;
    this.appendMany(chunks);
  }

  get isFull(): boolean {
    return this.#totalSamples >= this.#maxSamples;
  }

  get maxSamples(): number {
    return this.#maxSamples;
  }

  get totalSamples(): number {
    return this.#totalSamples;
  }

  append(chunk: Float32Array): RollingAudioBufferSnapshot {
    return this.appendMany([chunk]);
  }

  appendMany(chunks: readonly Float32Array[]): RollingAudioBufferSnapshot {
    for (const chunk of chunks) {
      if (chunk.length === 0) {
        continue;
      }

      this.#chunks.push(new Float32Array(chunk));
      this.#totalSamples += chunk.length;
    }

    this.#trimToMaxSamples();
    return this.snapshot();
  }

  clear(): RollingAudioBufferSnapshot {
    this.#chunks = [];
    this.#totalSamples = 0;
    return this.snapshot();
  }

  resize(maxSamples: number): RollingAudioBufferSnapshot {
    assertValidMaxSamples(maxSamples);
    this.#maxSamples = maxSamples;
    this.#trimToMaxSamples();
    return this.snapshot();
  }

  snapshot(): RollingAudioBufferSnapshot {
    return {
      chunks: this.toChunks(),
      maxSamples: this.#maxSamples,
      remainingSamples: Math.max(0, this.#maxSamples - this.#totalSamples),
      totalSamples: this.#totalSamples,
    };
  }

  toChunks(): Float32Array[] {
    return this.#chunks.map((chunk) => new Float32Array(chunk));
  }

  toFloat32Array(): Float32Array {
    const samples = new Float32Array(this.#totalSamples);
    let offset = 0;

    for (const chunk of this.#chunks) {
      samples.set(chunk, offset);
      offset += chunk.length;
    }

    return samples;
  }

  #trimToMaxSamples() {
    let overflow = this.#totalSamples - this.#maxSamples;

    while (overflow > 0 && this.#chunks.length > 0) {
      const firstChunk = this.#chunks[0];

      if (firstChunk.length <= overflow) {
        this.#chunks.shift();
        this.#totalSamples -= firstChunk.length;
        overflow -= firstChunk.length;
      } else {
        this.#chunks[0] = new Float32Array(firstChunk.slice(overflow));
        this.#totalSamples -= overflow;
        overflow = 0;
      }
    }
  }
}

export function appendBoundedAudioChunk(
  buffer: AudioChunkBuffer,
  chunk: Float32Array,
  maxSamples: number,
): AudioChunkBuffer {
  assertValidMaxSamples(maxSamples);

  const nextChunks = [...buffer.chunks, new Float32Array(chunk)];
  return trimAudioChunksToMaxSamples(nextChunks, maxSamples);
}

export function trimAudioChunksToMaxSamples(
  chunks: readonly Float32Array[],
  maxSamples: number,
): AudioChunkBuffer {
  assertValidMaxSamples(maxSamples);

  let samplesToKeep = maxSamples;
  const kept: Float32Array[] = [];

  for (let index = chunks.length - 1; index >= 0 && samplesToKeep > 0; index -= 1) {
    const chunk = chunks[index];
    if (chunk.length <= samplesToKeep) {
      kept.unshift(new Float32Array(chunk));
      samplesToKeep -= chunk.length;
    } else {
      kept.unshift(new Float32Array(chunk.slice(chunk.length - samplesToKeep)));
      samplesToKeep = 0;
    }
  }

  return {
    chunks: kept,
    totalSamples: kept.reduce((total, keptChunk) => total + keptChunk.length, 0),
  };
}

function assertValidMaxSamples(maxSamples: number) {
  if (!Number.isInteger(maxSamples) || maxSamples <= 0) {
    throw new RangeError("maxSamples must be a positive integer");
  }
}
