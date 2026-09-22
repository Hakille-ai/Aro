/**
 * Zero-dependency Web-standard ZIP Archive Reader
 * Supports extracting files from OpenXML (.xlsx, .docx) archives
 * using standard DataView and DecompressionStream ('deflate-raw').
 */

export interface ZipEntry {
  name: string;
  compressedSize: number;
  uncompressedSize: number;
  compressionMethod: number; // 0 = Stored, 8 = Deflated
  data: Uint8Array;
}

export async function decompressDeflateRaw(compressed: Uint8Array): Promise<Uint8Array> {
  if (compressed.length === 0) return new Uint8Array(0);

  if (typeof DecompressionStream !== "undefined") {
    try {
      const ds = new DecompressionStream("deflate-raw");
      const writer = ds.writable.getWriter();
      const chunk = compressed.byteOffset === 0 && compressed.byteLength === compressed.buffer.byteLength
        ? compressed
        : compressed.slice();

      const writePromise = writer.write(chunk as unknown as BufferSource).then(() => writer.close());
      const reader = ds.readable.getReader();
      const chunks: Uint8Array[] = [];
      let totalLen = 0;

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        if (value) {
          chunks.push(value);
          totalLen += value.length;
        }
      }
      await writePromise;

      const out = new Uint8Array(totalLen);
      let offset = 0;
      for (const c of chunks) {
        out.set(c, offset);
        offset += c.length;
      }
      return out;
    } catch {
      // Fallback if deflate-raw decompress fails
    }
  }
  return compressed;
}

/**
 * Extracts all files from a ZIP ArrayBuffer or Uint8Array into a filename -> Uint8Array map.
 */
export async function readZipEntries(input: ArrayBuffer | Uint8Array): Promise<Map<string, Uint8Array>> {
  const entries = new Map<string, Uint8Array>();
  const buffer = input instanceof Uint8Array
    ? (input.byteOffset === 0 && input.byteLength === input.buffer.byteLength
        ? (input.buffer as ArrayBuffer)
        : (input.slice().buffer as ArrayBuffer))
    : input;

  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);
  const len = buffer.byteLength;

  if (len < 22) return entries;

  // Search for End of Central Directory Record (EOCD): 0x06054b50
  let eocdOffset = -1;
  for (let i = len - 22; i >= Math.max(0, len - 65557); i--) {
    if (view.getUint32(i, true) === 0x06054b50) {
      eocdOffset = i;
      break;
    }
  }

  if (eocdOffset !== -1) {
    const totalEntries = view.getUint16(eocdOffset + 10, true);
    const cdOffset = view.getUint32(eocdOffset + 16, true);

    let currentCd = cdOffset;
    const decoder = new TextDecoder("utf-8");

    for (let i = 0; i < totalEntries && currentCd + 46 <= len; i++) {
      if (view.getUint32(currentCd, true) !== 0x02014b50) break;

      const compressionMethod = view.getUint16(currentCd + 10, true);
      const compressedSize = view.getUint32(currentCd + 20, true);
      const nameLen = view.getUint16(currentCd + 28, true);
      const extraLen = view.getUint16(currentCd + 30, true);
      const commentLen = view.getUint16(currentCd + 32, true);
      const localHeaderOffset = view.getUint32(currentCd + 42, true);

      const nameBytes = bytes.subarray(currentCd + 46, currentCd + 46 + nameLen);
      const filename = decoder.decode(nameBytes);

      // Read local header to get the exact data start
      if (localHeaderOffset + 30 <= len && view.getUint32(localHeaderOffset, true) === 0x04034b50) {
        const localNameLen = view.getUint16(localHeaderOffset + 26, true);
        const localExtraLen = view.getUint16(localHeaderOffset + 28, true);
        const dataStart = localHeaderOffset + 30 + localNameLen + localExtraLen;
        const compressedData = bytes.subarray(dataStart, dataStart + compressedSize);

        if (compressionMethod === 0) {
          // Stored / uncompressed
          entries.set(filename, compressedData);
        } else if (compressionMethod === 8) {
          // Deflated
          const decompressed = await decompressDeflateRaw(compressedData);
          entries.set(filename, decompressed);
        } else {
          entries.set(filename, compressedData);
        }
      }

      currentCd += 46 + nameLen + extraLen + commentLen;
    }
    return entries;
  }

  // Fallback: Local header sequential scan
  let pos = 0;
  const decoder = new TextDecoder("utf-8");
  while (pos + 30 <= len) {
    const sig = view.getUint32(pos, true);
    if (sig !== 0x04034b50) break;

    const compressionMethod = view.getUint16(pos + 8, true);
    const compressedSize = view.getUint32(pos + 18, true);
    const nameLen = view.getUint16(pos + 26, true);
    const extraLen = view.getUint16(pos + 28, true);

    const nameBytes = bytes.subarray(pos + 30, pos + 30 + nameLen);
    const filename = decoder.decode(nameBytes);
    const dataStart = pos + 30 + nameLen + extraLen;
    const compressedData = bytes.subarray(dataStart, dataStart + compressedSize);

    if (compressionMethod === 0) {
      entries.set(filename, compressedData);
    } else if (compressionMethod === 8) {
      const decompressed = await decompressDeflateRaw(compressedData);
      entries.set(filename, decompressed);
    } else {
      entries.set(filename, compressedData);
    }

    pos = dataStart + compressedSize;
  }

  return entries;
}

export function readTextFileFromZip(entries: Map<string, Uint8Array>, path: string): string | null {
  const normPath = path.startsWith("/") ? path.slice(1) : path;
  const data = entries.get(normPath) || entries.get(path);
  if (!data) return null;
  return new TextDecoder("utf-8").decode(data);
}
