/** Generate a valid, single-image Win32 ICO without external image packages.
 * The result is a minimal Lu 'L' application icon, not a borrowed Jan asset. */
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { crc32, deflateSync } from 'node:zlib';
const root = resolve(import.meta.dirname, '..');
const w = 64, h = 64, row = 4 * w;
const header = Buffer.alloc(6 + 16 + 40);
header.writeUInt16LE(1, 2); header.writeUInt16LE(1, 4);
header.writeUInt8(w, 6); header.writeUInt8(h, 7);
header.writeUInt16LE(1, 10); header.writeUInt16LE(32, 12);
header.writeUInt32LE(40 + row*h + (w/8)*h, 14); header.writeUInt32LE(22, 18);
header.writeUInt32LE(40, 22); header.writeInt32LE(w, 26); header.writeInt32LE(2*h, 30);
header.writeUInt16LE(1, 34); header.writeUInt16LE(32, 36);
header.writeUInt32LE(row*h + (w/8)*h, 42);
const rgba = Buffer.alloc(row*h), mask = Buffer.alloc((w/8)*h);
for (let y=0;y<h;y++) for(let x=0;x<w;x++) {
  const i = ((h-1-y)*w+x)*4;
  const inside=x>=5 && x<=58 && y>=5 && y<=58;
  const glyph=(x>=22 && x<=29 && y>=16 && y<=47) || (x>=22 && x<=44 && y>=41 && y<=47);
  const color=glyph?[245,242,231]:[113,78,49];
  rgba[i]=color[2];rgba[i+1]=color[1];rgba[i+2]=color[0];rgba[i+3]=inside?255:0;
  if(!inside) mask[(h-1-y)*(w/8)+(x>>3)] |= (0x80>>(x&7));
}
const ico=Buffer.concat([header,rgba,mask]);
const dir=resolve(root,'src-tauri/icons');mkdirSync(dir,{recursive:true});
writeFileSync(resolve(dir,'icon.ico'),ico);
console.log(`Generated native Lu Windows icon (${ico.length} bytes)`);

// Tauri uses a PNG application icon on Linux. Reuse the ICO pixel data,
// reversing the bottom-up rows and converting BGRA to RGBA.
const pixels = Buffer.alloc(h * (row + 1));
for (let y = 0; y < h; y++) {
  for (let x = 0; x < w; x++) {
    const source = ((h - 1 - y) * w + x) * 4;
    const dest = y * (row + 1) + 1 + x * 4;
    pixels[dest] = rgba[source + 2];
    pixels[dest + 1] = rgba[source + 1];
    pixels[dest + 2] = rgba[source];
    pixels[dest + 3] = rgba[source + 3];
  }
}
function pngChunk(type, data) {
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const length = Buffer.alloc(4);
  const checksum = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  checksum.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, checksum]);
}
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(w, 0);
ihdr.writeUInt32BE(h, 4);
ihdr[8] = 8;
ihdr[9] = 6;
writeFileSync(resolve(dir, 'icon.png'), Buffer.concat([
  Buffer.from('89504e470d0a1a0a', 'hex'),
  pngChunk('IHDR', ihdr),
  pngChunk('IDAT', deflateSync(pixels)),
  pngChunk('IEND', Buffer.alloc(0)),
]));
