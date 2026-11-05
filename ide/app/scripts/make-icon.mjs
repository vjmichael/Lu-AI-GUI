/** Deterministic first-party Lu IDE icon, generated without external packages.
 * This asset belongs only to the standalone IDE application, not Lu Desktop.
 * Produces a 64x64 32-bit BGRA ICO with a transparent boundary.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
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
console.log('Generated standalone Lu IDE icon: '+file);
