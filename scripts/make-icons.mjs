// Builds the application icons (PNG + multi-image ICO) with no external image library.
// Design: deep-green rounded tile, a white document, and a seal — reads clearly at 32 px and does
// not depend on a font being installed on the build machine.
import { deflateSync } from 'node:zlib';
import { writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

const OUT = join(process.cwd(), 'desktop', 'src-tauri', 'icons');
mkdirSync(OUT, { recursive: true });

const ACCENT = [15, 107, 82, 255];
const ACCENT_DARK = [10, 78, 60, 255];
const PAPER = [255, 255, 255, 255];
const FOLD = [214, 228, 222, 255];

const SS = 4; // supersampling factor

function newCanvas(size) {
  return { size, px: new Uint8ClampedArray(size * size * 4) };
}

function blend(canvas, x, y, color, coverage = 1) {
  if (x < 0 || y < 0 || x >= canvas.size || y >= canvas.size) return;
  const i = (y * canvas.size + x) * 4;
  const a = (color[3] / 255) * coverage;
  const dstA = canvas.px[i + 3] / 255;
  const outA = a + dstA * (1 - a);
  if (outA === 0) return;
  for (let c = 0; c < 3; c += 1) {
    canvas.px[i + c] = (color[c] * a + canvas.px[i + c] * dstA * (1 - a)) / outA;
  }
  canvas.px[i + 3] = outA * 255;
}

function roundRect(canvas, x0, y0, x1, y1, r, color) {
  for (let y = Math.floor(y0); y < Math.ceil(y1); y += 1) {
    for (let x = Math.floor(x0); x < Math.ceil(x1); x += 1) {
      const cx = Math.min(Math.max(x + 0.5, x0 + r), x1 - r);
      const cy = Math.min(Math.max(y + 0.5, y0 + r), y1 - r);
      const dx = x + 0.5 - cx;
      const dy = y + 0.5 - cy;
      if (dx * dx + dy * dy <= r * r) blend(canvas, x, y, color);
    }
  }
}

function circle(canvas, cx, cy, r, color) {
  for (let y = Math.floor(cy - r); y < Math.ceil(cy + r); y += 1) {
    for (let x = Math.floor(cx - r); x < Math.ceil(cx + r); x += 1) {
      const dx = x + 0.5 - cx;
      const dy = y + 0.5 - cy;
      if (dx * dx + dy * dy <= r * r) blend(canvas, x, y, color);
    }
  }
}

function drawIcon(size) {
  const canvas = newCanvas(size);
  const u = size / 256; // design units are 256×256

  roundRect(canvas, 0, 0, size, size, 56 * u, ACCENT);
  // Subtle depth on the lower edge.
  roundRect(canvas, 0, size - 26 * u, size, size, 26 * u, ACCENT_DARK);

  // Document.
  roundRect(canvas, 66 * u, 48 * u, 190 * u, 196 * u, 10 * u, PAPER);
  // Folded corner.
  roundRect(canvas, 150 * u, 48 * u, 190 * u, 88 * u, 8 * u, FOLD);
  // Text lines.
  for (let i = 0; i < 4; i += 1) {
    const y = (104 + i * 20) * u;
    roundRect(canvas, 82 * u, y, (i === 3 ? 140 : 174) * u, y + 8 * u, 4 * u, ACCENT);
  }
  // Seal.
  circle(canvas, 176 * u, 176 * u, 38 * u, PAPER);
  circle(canvas, 176 * u, 176 * u, 30 * u, ACCENT);
  circle(canvas, 176 * u, 176 * u, 12 * u, PAPER);
  return canvas;
}

function downsample(canvas, target) {
  const out = newCanvas(target);
  const ratio = canvas.size / target;
  for (let y = 0; y < target; y += 1) {
    for (let x = 0; x < target; x += 1) {
      let r = 0;
      let g = 0;
      let b = 0;
      let a = 0;
      for (let sy = 0; sy < ratio; sy += 1) {
        for (let sx = 0; sx < ratio; sx += 1) {
          const i = ((y * ratio + sy) * canvas.size + (x * ratio + sx)) * 4;
          r += canvas.px[i];
          g += canvas.px[i + 1];
          b += canvas.px[i + 2];
          a += canvas.px[i + 3];
        }
      }
      const n = ratio * ratio;
      const o = (y * target + x) * 4;
      out.px[o] = r / n;
      out.px[o + 1] = g / n;
      out.px[o + 2] = b / n;
      out.px[o + 3] = a / n;
    }
  }
  return out;
}

const crcTable = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(buffer) {
  let c = 0xffffffff;
  for (const byte of buffer) c = crcTable[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([length, body, crc]);
}

function png(canvas) {
  const { size, px } = canvas;
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  const raw = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y += 1) {
    const row = y * (size * 4 + 1);
    raw[row] = 0;
    for (let x = 0; x < size; x += 1) {
      const src = (y * size + x) * 4;
      const dst = row + 1 + x * 4;
      raw[dst] = px[src];
      raw[dst + 1] = px[src + 1];
      raw[dst + 2] = px[src + 2];
      raw[dst + 3] = px[src + 3];
    }
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const master = drawIcon(256 * SS);
const images = new Map();
for (const size of [16, 24, 32, 48, 64, 128, 256]) {
  images.set(size, png(downsample(master, size)));
}

for (const [size, buf] of images) {
  const file = size === 256 ? 'icon.png' : `${size}x${size}.png`;
  writeFileSync(join(OUT, file), buf);
}
console.log(`wrote ${images.size} PNG sizes`);

// Multi-image ICO: every entry is a PNG payload (Vista and later).
function ico(sizes) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(sizes.length, 4);

  const entries = [];
  let offset = 6 + sizes.length * 16;
  const payloads = [];
  for (const size of sizes) {
    const data = images.get(size);
    const entry = Buffer.alloc(16);
    entry[0] = size >= 256 ? 0 : size;
    entry[1] = size >= 256 ? 0 : size;
    entry[2] = 0;
    entry[3] = 0;
    entry.writeUInt16LE(1, 4);
    entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(data.length, 8);
    entry.writeUInt32LE(offset, 12);
    offset += data.length;
    entries.push(entry);
    payloads.push(data);
  }
  return Buffer.concat([header, ...entries, ...payloads]);
}

writeFileSync(join(OUT, 'icon.ico'), ico([16, 32, 48, 256]));
console.log('wrote icon.ico (16/32/48/256)');
