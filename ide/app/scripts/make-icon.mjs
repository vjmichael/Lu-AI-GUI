/** Deterministic first-party Lu IDE icon, generated without external packages.
 * This asset belongs only to the standalone IDE application, not Lu Desktop.
 * Produces a 64x64 32-bit BGRA ICO with a transparent boundary.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { deflateSync } from 'node:zlib';
const root=resolve(import.meta.dirname,'..');
const w=64,h=64,row=w*4,maskStride=Math.ceil(w/32)*4;
const icoHeader=Buffer.alloc(6+16+40);
icoHeader.writeUInt16LE(1,2);
icoHeader.writeUInt16LE(1,4);
icoHeader.writeUInt8(w,6); icoHeader.writeUInt8(h,7);
icoHeader.writeUInt16LE(1,10); icoHeader.writeUInt16LE(32,12);
icoHeader.writeUInt32LE(40+row*h+maskStride*h,14);
icoHeader.writeUInt32LE(22,18);
icoHeader.writeUInt32LE(40,22);
icoHeader.writeInt32LE(w,26); icoHeader.writeInt32LE(h*2,30);
icoHeader.writeUInt16LE(1,34); icoHeader.writeUInt16LE(32,36);
icoHeader.writeUInt32LE(row*h+maskStride*h,42);
const pixels=Buffer.alloc(row*h),mask=Buffer.alloc(maskStride*h);
for(let y=0;y<h;y++) for(let x=0;x<w;x++){
  const center=x>=6&&x<=57&&y>=6&&y<=57;
  const stroke=(x>=21&&x<=29&&y>=16&&y<=47) ||
    (x>=21&&x<=46&&y>=40&&y<=47) ||
    (x>=36&&x<=46&&y>=16&&y<=23) ||
    (x>=39&&x<=46&&y>=24&&y<=33);
  const i=((h-1-y)*w+x)*4;
  // Teal IDE background + white "L" and small debugging brackets.
  const [r,g,b]=stroke?[239,248,255]:[21,105,128];
  pixels[i]=b;pixels[i+1]=g;pixels[i+2]=r;pixels[i+3]=center?255:0;
  if(!center)mask[(h-1-y)*maskStride+(x>>3)]|=0x80>>(x&7);
}
const dir=resolve(root,'src-tauri/icons');
mkdirSync(dir,{recursive:true});
const file=resolve(dir,'icon.ico');
writeFileSync(file,Buffer.concat([icoHeader,pixels,mask]));

// Tauri's Linux icon resource needs an actual PNG, not a Windows ICO.
// Convert this SAME first-party pixel data to a deterministic 64x64 RGBA PNG
// with the correct top-down scanline order and standard zlib compression.
// No downloaded graphics, image libraries, font files or environment tools.
function crc32(bytes) {
  let crc=0xffffffff;
  for(const byte of bytes){
    crc^=byte;
    for(let i=0;i<8;i++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);
  }
  return (crc^0xffffffff)>>>0;
}
function pngChunk(kind,data){
  const tag=Buffer.from(kind,'ascii');
  const size=Buffer.alloc(4),checksum=Buffer.alloc(4);
  size.writeUInt32BE(data.length);
  checksum.writeUInt32BE(crc32(Buffer.concat([tag,data])));
  return Buffer.concat([size,tag,data,checksum]);
}
const pngRows=Buffer.alloc(h*(1+w*4));
for(let y=0;y<h;y++){
  const rowStart=y*(1+w*4);
  pngRows[rowStart]=0; // no PNG row filter
  for(let x=0;x<w;x++){
    const src=((h-1-y)*w+x)*4;
    const dest=rowStart+1+x*4;
    pngRows[dest]=pixels[src+2];
    pngRows[dest+1]=pixels[src+1];
    pngRows[dest+2]=pixels[src];
    pngRows[dest+3]=pixels[src+3];
  }
}
const ihdr=Buffer.alloc(13);
ihdr.writeUInt32BE(w,0);ihdr.writeUInt32BE(h,4);
ihdr[8]=8;ihdr[9]=6; // 8-bit RGBA
const png=Buffer.concat([
  Buffer.from([137,80,78,71,13,10,26,10]),
  pngChunk('IHDR',ihdr),
  pngChunk('IDAT',deflateSync(pngRows,{level:9})),
  pngChunk('IEND',Buffer.alloc(0)),
]);
const pngFile=resolve(dir,'icon.png');
writeFileSync(pngFile,png);
console.log('Generated standalone Lu IDE icons: '+file+' and '+pngFile);
