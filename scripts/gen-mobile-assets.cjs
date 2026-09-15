// Génère des assets Expo placeholder (couleurs unies) — remplace par ton vrai design avant release.
const zlib = require("zlib");
const fs = require("fs");
const path = require("path");

const crcTable = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();

function crc32(buf) {
  let c = -1;
  for (let i = 0; i < buf.length; i++) c = crcTable[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}

function solidPng(size, [r, g, b]) {
  const raw = Buffer.alloc(size * (1 + size * 3));
  for (let y = 0; y < size; y++) {
    raw[y * (1 + size * 3)] = 0;
    for (let x = 0; x < size; x++) {
      const o = y * (1 + size * 3) + 1 + x * 3;
      raw[o] = r;
      raw[o + 1] = g;
      raw[o + 2] = b;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  const png = Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", zlib.deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
  return png;
}

const dir = path.join(__dirname, "..", "apps", "mobile", "assets");
fs.mkdirSync(dir, { recursive: true });
// Bleu ARO #0a84ff pour l'icône, fond clair pour le splash, blanc pour la notif.
fs.writeFileSync(path.join(dir, "icon.png"), solidPng(1024, [10, 132, 255]));
fs.writeFileSync(path.join(dir, "splash-icon.png"), solidPng(512, [247, 248, 250]));
fs.writeFileSync(path.join(dir, "adaptive-icon.png"), solidPng(1024, [10, 132, 255]));
fs.writeFileSync(path.join(dir, "notification-icon.png"), solidPng(96, [255, 255, 255]));
console.log("Assets placeholder écrits dans", dir);
