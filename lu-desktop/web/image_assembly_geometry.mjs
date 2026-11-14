// Pure, bounded geometry for the existing deterministic native image assembly.
// This predicts coordinates only; native Lu Core revalidates every operation
// and owns the pixels, authority and final file/export receipt.
export const REGION_IMAGE_OPERATIONS = Object.freeze([
  'crop','redact','pixelate','rectangle','line','ellipse','arrow'
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
    }else if(kind==='flip_h'||kind==='grayscale'||kind==='invert'){
      // Pixel filters keep the same dimensions.
    }else if(kind==='blur'||kind==='sharpen'){
      if(!Number.isSafeInteger(step.sigma)||step.sigma<1||step.sigma>8)return null;
      if(kind==='sharpen'&&(!Number.isSafeInteger(step.threshold)||
          step.threshold<0||step.threshold>64))return null;
    }else if(kind==='brightness'||kind==='contrast'){
      const allowed=kind==='brightness'?128:100;
      if(!Number.isSafeInteger(step.amount)||
         step.amount< -allowed||step.amount>allowed)return null;
    }else if(regions.has(kind)){
      const {x,y,width:rw,height:rh}=step;
      if(!Number.isSafeInteger(x)||!Number.isSafeInteger(y)||
        x<0||y<0||!bounded(rw,rh)||
        x>=w||y>=h||rw>w-x||rh>h-y)return null;
      if(kind==='crop'){w=rw;h=rh;}
      if(kind==='pixelate' &&
        (!Number.isSafeInteger(step.blockSize)||step.blockSize<2||step.blockSize>128))return null;
      if(['rectangle','line','ellipse','arrow'].includes(kind)){
        const stroke=step.stroke;
        if(!Number.isSafeInteger(stroke)||stroke<1||
          stroke>(['line','arrow'].includes(kind)?32:64)||
          (kind==='arrow'&&(rw<3||rh<3))||
          !Number.isSafeInteger(step.color)||step.color<0||step.color>0xffffff)return null;
      }
    }else{return null}
  }
  return {width:w,height:h};
}

/// Convert a user pointer gesture into pixels of the ACTUAL displayed image.
/// The browser is not allowed to supply pixel dimensions or any file path.
export function imagePointerPixel(clientX,clientY,rect,width,height){
  if(!bounded(width,height)||!rect||
    ![clientX,clientY,rect.left,rect.top,rect.width,rect.height]
      .every(Number.isFinite)||rect.width<=0||rect.height<=0)return null;
  return {
    x:Math.min(width-1,Math.max(0,
      Math.floor((clientX-rect.left)/rect.width*width))),
    y:Math.min(height-1,Math.max(0,
      Math.floor((clientY-rect.top)/rect.height*height)))
  };
}
export function imageDragRectangle(a,b,width,height){
  if(!a||!b||!bounded(width,height)||
    ![a.x,a.y,b.x,b.y].every(Number.isSafeInteger)||
    [a.x,b.x].some(x=>x<0||x>=width)||
    [a.y,b.y].some(y=>y<0||y>=height))return null;
  const x=Math.min(a.x,b.x),y=Math.min(a.y,b.y);
  return {x,y,width:Math.abs(a.x-b.x)+1,height:Math.abs(a.y-b.y)+1};
}
