// Pure, bounded geometry for the existing deterministic native image assembly.
// This predicts coordinates only; native Lu Core revalidates every operation
// and owns the pixels, authority and final file/export receipt.
export const REGION_IMAGE_OPERATIONS = Object.freeze([
  'crop','redact','pixelate','rectangle','line','ellipse'
]);
const regions=new Set(REGION_IMAGE_OPERATIONS);
const bounded=(w,h)=>Number.isSafeInteger(w)&&Number.isSafeInteger(h)
  &&w>0&&h>0&&w<=8192&&h<=8192&&w*h<=32_000_000;
export function stagedImageDimensions(width,height,steps=[]){
  if(!bounded(width,height)||!Array.isArray(steps)||steps.length>12)return null;
  let w=width,h=height;
  for(const step of steps){
    if(!step||typeof step.operation!=='string')return null;
    const kind=step.operation;
    if(kind==='resize'){
      if(!bounded(step.width,step.height))return null;
      w=step.width;h=step.height;
    }else if(kind==='rotate90'){
      [w,h]=[h,w];
    }else if(kind==='flip_h'){
      // No change to coordinate space.
    }else if(regions.has(kind)){
      const {x,y,width:rw,height:rh}=step;
      if(!Number.isSafeInteger(x)||!Number.isSafeInteger(y)||
        x<0||y<0||!bounded(rw,rh)||
        x>=w||y>=h||rw>w-x||rh>h-y)return null;
      if(kind==='crop'){w=rw;h=rh;}
      if(kind==='pixelate' &&
        (!Number.isSafeInteger(step.blockSize)||step.blockSize<2||step.blockSize>128))return null;
      if(['rectangle','line','ellipse'].includes(kind)){
        const stroke=step.stroke;
        if(!Number.isSafeInteger(stroke)||stroke<1||
          stroke>(kind==='line'?32:64)||
          !Number.isSafeInteger(step.color)||step.color<0||step.color>0xffffff)return null;
      }
    }else{return null}
  }
  return {width:w,height:h};
}
