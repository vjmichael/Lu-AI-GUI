//! WAVE4-IMAGE-WS-02: bounded, deterministic PNG crop/resize from an
//! already host-admitted PinnedImage. No model edit prompt, mask, background
//! removal, source overwrite, arbitrary file path or silent export.
//!
//! An OS picker + explicit user approval in Tauri controls effect destination.
//! This core never performs filesystem operations or acquires authority.
use crate::multimodal_saved_turn::{
    PinnedImage,MAX_IMAGE_BYTES,MAX_IMAGE_EDGE,MAX_IMAGE_PIXELS,
};
use image::{DynamicImage,GenericImageView,ImageFormat,ImageReader,Limits};
use sha2::{Digest,Sha256};
use std::io::Cursor;
const MAX_RESULT_BYTES:usize=16*1024*1024;

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ImageRevisionOperation{
    Crop{x:u32,y:u32,width:u32,height:u32},
    Resize{width:u32,height:u32},
    Rotate90,
    FlipHorizontal,
    /// Solid rectangular privacy mask; output is always a new image.
    Redact{x:u32,y:u32,width:u32,height:u32},
    /// Deterministic block-average mosaic, not a security redaction.
    Pixelate{x:u32,y:u32,width:u32,height:u32,block_size:u32},
    /// Draw a visible annotation border over an immutable source snapshot.
    Rectangle{x:u32,y:u32,width:u32,height:u32,rgb:[u8;3],stroke:u32},
    /// A visible diagonal annotation bounded to the selected rectangle.
    Line{x:u32,y:u32,width:u32,height:u32,rgb:[u8;3],stroke:u32},
    /// Ellipse outline. Never advertised as a privacy mask.
    Ellipse{x:u32,y:u32,width:u32,height:u32,rgb:[u8;3],stroke:u32},
    /// Diagonal line with a bounded, visible arrowhead.
    Arrow{x:u32,y:u32,width:u32,height:u32,rgb:[u8;3],stroke:u32},
    /// Bounded pixel filters preserve the input alpha channel.
    Grayscale,
    InvertRgb,
    Blur{sigma:u8},
    Brightness{delta:i16},
    Contrast{percent:i16},
    Sharpen{sigma:u8,threshold:u8},
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct ImageRevision{
    pub base_ref:String,
    pub base_sha256:String,
    pub revision_sha256:String,
    pub width:u32,
    pub height:u32,
    pub mime_type:&'static str,
    pub output_png:Vec<u8>,
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum ImageRevisionError{
    InvalidSource,InvalidOperation,TooLarge,Encode,
}
fn dimensions_allowed(w:u32,h:u32)->bool{
    w>0&&h>0&&w<=MAX_IMAGE_EDGE&&h<=MAX_IMAGE_EDGE
        &&u64::from(w)*u64::from(h)<=MAX_IMAGE_PIXELS
}
fn paint_brush(
    image:&mut image::RgbaImage,x:u32,y:u32,rgb:[u8;3],stroke:u32,
){
    let before=stroke.saturating_sub(1)/2;
    let after=stroke/2;
    let max_x=image.width().saturating_sub(1);
    let max_y=image.height().saturating_sub(1);
    for yy in y.saturating_sub(before)..=y.saturating_add(after).min(max_y){
        for xx in x.saturating_sub(before)..=x.saturating_add(after).min(max_x){
            image.put_pixel(xx,yy,image::Rgba([rgb[0],rgb[1],rgb[2],255]));
        }
    }
}
/// Keep the original alpha channel for all color/spatial filters. Blur is a
/// visual treatment, NEVER a privacy redaction and never silently removes
/// transparency from source pixels.
fn original_alpha(source:&DynamicImage,filtered:DynamicImage)->DynamicImage{
    let raw=source.to_rgba8();
    let mut edited=filtered.to_rgba8();
    for (pixel,from) in edited.pixels_mut().zip(raw.pixels()){
        pixel[3]=from[3];
    }
    DynamicImage::ImageRgba8(edited)
}
fn paint_segment(
    canvas:&mut image::RgbaImage,
    a:(u32,u32),b:(u32,u32),rgb:[u8;3],stroke:u32
){
    let dx=i64::from(b.0)-i64::from(a.0);
    let dy=i64::from(b.1)-i64::from(a.1);
    let steps=dx.unsigned_abs().max(dy.unsigned_abs()).max(1);
    for step in 0..=steps{
        let xx=i64::from(a.0)+dx*step as i64/steps as i64;
        let yy=i64::from(a.1)+dy*step as i64/steps as i64;
        paint_brush(canvas,xx.max(0) as u32,yy.max(0) as u32,rgb,stroke);
    }
}
fn validate_region(
    x:u32,y:u32,width:u32,height:u32,base_w:u32,base_h:u32
)->Result<(),ImageRevisionError>{
    if !dimensions_allowed(width,height)||x>=base_w||y>=base_h
       ||width>base_w-x||height>base_h-y{
        return Err(ImageRevisionError::InvalidOperation);
    }
    Ok(())
}
pub fn transform_admitted_image(
    source:&PinnedImage,operation:ImageRevisionOperation
)->Result<ImageRevision,ImageRevisionError>{
    if source.bytes.is_empty()||source.bytes.len()>MAX_IMAGE_BYTES
      ||!source.image_ref.starts_with("img-")
      ||source.image_ref.len()!=36
      ||!source.image_ref[4..].bytes().all(|b|b.is_ascii_hexdigit())
      ||Sha256::digest(&source.bytes).as_slice()!=source.sha256.as_slice(){
        return Err(ImageRevisionError::InvalidSource);
    }
    let fmt=match source.mime_type{
        "image/png" if source.bytes.starts_with(b"\x89PNG\r\n\x1a\n")=>ImageFormat::Png,
        "image/jpeg" if source.bytes.starts_with(&[0xff,0xd8,0xff])=>ImageFormat::Jpeg,
        _=>return Err(ImageRevisionError::InvalidSource),
    };
    let mut reader=ImageReader::with_format(Cursor::new(&source.bytes),fmt);
    let mut limits=Limits::default();
    limits.max_image_width=Some(MAX_IMAGE_EDGE);
    limits.max_image_height=Some(MAX_IMAGE_EDGE);
    limits.max_alloc=Some(128*1024*1024);
    reader.limits(limits);
    let decoded=reader.decode().map_err(|_|ImageRevisionError::InvalidSource)?;
    let (base_w,base_h)=decoded.dimensions();
    if !dimensions_allowed(base_w,base_h){
        return Err(ImageRevisionError::InvalidSource);
    }
    let transformed=match operation{
        ImageRevisionOperation::Crop{x,y,width,height}=>{
            if !dimensions_allowed(width,height)||x>=base_w||y>=base_h
               ||width>base_w-x||height>base_h-y{
                return Err(ImageRevisionError::InvalidOperation);
            }
            decoded.crop_imm(x,y,width,height)
        }
        ImageRevisionOperation::Resize{width,height}=>{
            if !dimensions_allowed(width,height){
                return Err(ImageRevisionError::InvalidOperation);
            }
            decoded.resize_exact(width,height,image::imageops::FilterType::Lanczos3)
        }
        ImageRevisionOperation::Rotate90=>decoded.rotate90(),
        ImageRevisionOperation::FlipHorizontal=>decoded.fliph(),
        ImageRevisionOperation::Redact{x,y,width,height}=>{
            validate_region(x,y,width,height,base_w,base_h)?;
            let mut image=decoded.to_rgba8();
            for py in y..y+height {
                for px in x..x+width {
                    image.put_pixel(px,py,image::Rgba([0,0,0,255]));
                }
            }
            DynamicImage::ImageRgba8(image)
        }
        ImageRevisionOperation::Pixelate{x,y,width,height,block_size}=>{
            validate_region(x,y,width,height,base_w,base_h)?;
            if !(2..=128).contains(&block_size){
                return Err(ImageRevisionError::InvalidOperation);
            }
            let mut image=decoded.to_rgba8();
            let x_end=x+width;
            let y_end=y+height;
            let mut py=y;
            while py<y_end{
                let by=(y_end-py).min(block_size);
                let mut px=x;
                while px<x_end{
                    let bx=(x_end-px).min(block_size);
                    let mut channels=[0u64;4];
                    for ry in py..py+by{
                        for rx in px..px+bx{
                            let p=image.get_pixel(rx,ry);
                            for c in 0..4{channels[c]+=u64::from(p[c]);}
                        }
                    }
                    let count=u64::from(bx)*u64::from(by);
                    let pixel=image::Rgba([
                        (channels[0]/count) as u8,(channels[1]/count) as u8,
                        (channels[2]/count) as u8,(channels[3]/count) as u8
                    ]);
                    for ry in py..py+by{
                        for rx in px..px+bx{image.put_pixel(rx,ry,pixel);}
                    }
                    px+=bx;
                }
                py+=by;
            }
            DynamicImage::ImageRgba8(image)
        }
        ImageRevisionOperation::Rectangle{x,y,width,height,rgb,stroke}=>{
            validate_region(x,y,width,height,base_w,base_h)?;
            if !(1..=64).contains(&stroke){return Err(ImageRevisionError::InvalidOperation);}
            let mut image=decoded.to_rgba8();
            let border=stroke.min(width).min(height);
            for py in y..y+height{
                for px in x..x+width{
                    if px-x<border||py-y<border||
                        x+width-1-px<border||y+height-1-py<border{
                        image.put_pixel(px,py,image::Rgba([rgb[0],rgb[1],rgb[2],255]));
                    }
                }
            }
            DynamicImage::ImageRgba8(image)
        },
        ImageRevisionOperation::Line{x,y,width,height,rgb,stroke}=>{
            validate_region(x,y,width,height,base_w,base_h)?;
            if !(1..=32).contains(&stroke){return Err(ImageRevisionError::InvalidOperation)}
            let mut canvas=decoded.to_rgba8();
            paint_segment(&mut canvas,(x,y),(x+width-1,y+height-1),rgb,stroke);
            DynamicImage::ImageRgba8(canvas)
        },
        ImageRevisionOperation::Arrow{x,y,width,height,rgb,stroke}=>{
            validate_region(x,y,width,height,base_w,base_h)?;
            if !(1..=32).contains(&stroke){return Err(ImageRevisionError::InvalidOperation)}
            if width<3||height<3{return Err(ImageRevisionError::InvalidOperation)}
            let mut canvas=decoded.to_rgba8();
            let start=(x,y);let end=(x+width-1,y+height-1);
            paint_segment(&mut canvas,start,end,rgb,stroke);
            let dx=f64::from(width-1);
            let dy=f64::from(height-1);
            let length=(dx*dx+dy*dy).sqrt();
            let ux=dx/length;let uy=dy/length;
            let head=f64::from((stroke*4+6).min(48)).min(length*0.5);
            for side in [-1.0f64,1.0]{
                let wing_x=f64::from(end.0)-ux*head-side*uy*head*0.55;
                let wing_y=f64::from(end.1)-uy*head+side*ux*head*0.55;
                let wx=wing_x.round().clamp(0.0,f64::from(base_w-1)) as u32;
                let wy=wing_y.round().clamp(0.0,f64::from(base_h-1)) as u32;
                paint_segment(&mut canvas,(wx,wy),end,rgb,stroke);
            }
            DynamicImage::ImageRgba8(canvas)
        },
        ImageRevisionOperation::Ellipse{x,y,width,height,rgb,stroke}=>{
            validate_region(x,y,width,height,base_w,base_h)?;
            if !(1..=64).contains(&stroke){return Err(ImageRevisionError::InvalidOperation)}
            let mut canvas=decoded.to_rgba8();
            let w=f64::from(width);let h=f64::from(height);
            let inner_x=(w/2.0-f64::from(stroke)).max(0.0);
            let inner_y=(h/2.0-f64::from(stroke)).max(0.0);
            for cy in 0..height{
                for cx in 0..width{
                    let dx=f64::from(cx)+0.5-w/2.0;
                    let dy=f64::from(cy)+0.5-h/2.0;
                    let outer=(dx/(w/2.0)).powi(2)+(dy/(h/2.0)).powi(2);
                    if outer>1.0{continue}
                    let inside=inner_x>0.0&&inner_y>0.0
                        &&(dx/inner_x).powi(2)+(dy/inner_y).powi(2)<1.0;
                    if !inside{
                        canvas.put_pixel(x+cx,y+cy,
                            image::Rgba([rgb[0],rgb[1],rgb[2],255]));
                    }
                }
            }
            DynamicImage::ImageRgba8(canvas)
        },
        ImageRevisionOperation::Grayscale=>{
            let mut pixels=decoded.to_rgba8();
            for pixel in pixels.pixels_mut(){
                let gray=(77*u32::from(pixel[0])+150*u32::from(pixel[1])
                    +29*u32::from(pixel[2])+128)/256;
                pixel[0]=gray as u8;pixel[1]=gray as u8;pixel[2]=gray as u8;
            }
            DynamicImage::ImageRgba8(pixels)
        },
        ImageRevisionOperation::InvertRgb=>{
            let mut pixels=decoded.to_rgba8();
            for pixel in pixels.pixels_mut(){
                pixel[0]=255-pixel[0];
                pixel[1]=255-pixel[1];
                pixel[2]=255-pixel[2];
            }
            DynamicImage::ImageRgba8(pixels)
        },
        ImageRevisionOperation::Blur{sigma}=>{
            if !(1..=8).contains(&sigma){return Err(ImageRevisionError::InvalidOperation)}
            original_alpha(&decoded,decoded.blur(f32::from(sigma)))
        },
        ImageRevisionOperation::Brightness{delta}=>{
            if !(-128..=128).contains(&delta){
                return Err(ImageRevisionError::InvalidOperation)
            }
            original_alpha(&decoded,decoded.brighten(i32::from(delta)))
        },
        ImageRevisionOperation::Contrast{percent}=>{
            if !(-100..=100).contains(&percent){
                return Err(ImageRevisionError::InvalidOperation)
            }
            original_alpha(&decoded,decoded.adjust_contrast(f32::from(percent)))
        },
        ImageRevisionOperation::Sharpen{sigma,threshold}=>{
            if !(1..=8).contains(&sigma)||threshold>64{
                return Err(ImageRevisionError::InvalidOperation)
            }
            original_alpha(&decoded,
                decoded.unsharpen(f32::from(sigma),i32::from(threshold)))
        }
    };
    let (width,height)=transformed.dimensions();
    let mut png=Vec::new();
    transformed.write_to(&mut Cursor::new(&mut png),ImageFormat::Png)
        .map_err(|_|ImageRevisionError::Encode)?;
    if png.is_empty()||png.len()>MAX_RESULT_BYTES{
        return Err(ImageRevisionError::TooLarge);
    }
    Ok(ImageRevision{
        base_ref:source.image_ref.clone(),
        base_sha256:source.sha256.iter().map(|b|format!("{b:02x}")).collect::<String>(),
        revision_sha256:format!("{:x}",Sha256::digest(&png)),
        width,height,mime_type:"image/png",output_png:png,
    })
}
/// An in-memory, nondestructive image edit assembly. The undo/redo UI
/// changes only this list; no intermediate PNG is ever written to disk.
/// The final output retains its ORIGINAL admitted source identity even
/// though each operation is applied to the preceding staged pixels.
pub fn transform_admitted_image_sequence(
    source:&PinnedImage,operations:&[ImageRevisionOperation]
)->Result<ImageRevision,ImageRevisionError>{
    if operations.is_empty()||operations.len()>12{
        return Err(ImageRevisionError::InvalidOperation);
    }
    let mut current=source.clone();
    let origin_ref=source.image_ref.clone();
    let origin_sha=source.sha256.iter().map(|b|format!("{b:02x}")).collect::<String>();
    let mut final_result=None;
    for (index,operation) in operations.iter().enumerate(){
        let mut result=transform_admitted_image(&current,*operation)?;
        result.base_ref=origin_ref.clone();
        result.base_sha256=origin_sha.clone();
        if index+1<operations.len(){
            if result.output_png.len()>MAX_IMAGE_BYTES{
                return Err(ImageRevisionError::TooLarge);
            }
            let digest=Sha256::digest(&result.output_png);
            let mut sha256=[0u8;32];
            sha256.copy_from_slice(&digest);
            current=PinnedImage{
                image_ref:format!("img-{}",digest[..16]
                    .iter().map(|b|format!("{b:02x}")).collect::<String>()),
                mime_type:"image/png",sha256,bytes:result.output_png.clone(),
            };
        }
        final_result=Some(result);
    }
    final_result.ok_or(ImageRevisionError::InvalidOperation)
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::multimodal_saved_turn::admit_data_url;
    use base64::{engine::general_purpose::STANDARD,Engine as _};
    fn original()->PinnedImage{
        let mut data=Vec::new();
        DynamicImage::new_rgba8(16,12).write_to(
            &mut Cursor::new(&mut data),ImageFormat::Png).unwrap();
        admit_data_url(&format!("data:image/png;base64,{}",STANDARD.encode(data))).unwrap()
    }
    #[test]
    fn crop_and_resize_do_not_mutate_admitted_source(){
        let source=original();let before=source.bytes.clone();
        let crop=transform_admitted_image(&source,
            ImageRevisionOperation::Crop{x:2,y:1,width:5,height:4}).unwrap();
        assert_eq!((crop.width,crop.height),(5,4));
        assert!(crop.output_png.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(source.bytes,before);
        let scaled=transform_admitted_image(&source,
            ImageRevisionOperation::Resize{width:10,height:8}).unwrap();
        assert_eq!((scaled.width,scaled.height),(10,8));
        assert_ne!(crop.revision_sha256,scaled.revision_sha256);
    }
    #[test]
    fn immutable_filters_are_deterministic_and_bounds_checked(){
        let source=original();
        let original_bytes=source.bytes.clone();
        for op in [
            ImageRevisionOperation::Grayscale,
            ImageRevisionOperation::InvertRgb,
            ImageRevisionOperation::Blur{sigma:2},
            ImageRevisionOperation::Brightness{delta:30},
            ImageRevisionOperation::Contrast{percent:20},
            ImageRevisionOperation::Sharpen{sigma:2,threshold:3}
        ]{
            let first=transform_admitted_image(&source,op).unwrap();
            let second=transform_admitted_image(&source,op).unwrap();
            assert_eq!(first.revision_sha256,second.revision_sha256);
            assert_eq!(source.bytes,original_bytes);
        }
        assert_eq!(
            transform_admitted_image(&source,ImageRevisionOperation::Blur{sigma:0}),
            Err(ImageRevisionError::InvalidOperation)
        );
        assert_eq!(
            transform_admitted_image(&source,ImageRevisionOperation::Contrast{percent:101}),
            Err(ImageRevisionError::InvalidOperation)
        );
    }
    #[test]
    fn arrow_is_deterministic_bounded_and_immutable(){
        let source=original();let original=source.bytes.clone();
        let operation=ImageRevisionOperation::Arrow{
            x:1,y:1,width:12,height:8,rgb:[255,80,0],stroke:2
        };
        let a=transform_admitted_image(&source,operation).unwrap();
        let b=transform_admitted_image(&source,operation).unwrap();
        assert_eq!(a.revision_sha256,b.revision_sha256);
        assert_eq!(source.bytes,original);
        assert_eq!(transform_admitted_image(&source,
            ImageRevisionOperation::Arrow{
                x:1,y:1,width:2,height:8,rgb:[255,80,0],stroke:2
            }),Err(ImageRevisionError::InvalidOperation));
    }
    #[test]
    fn elliptical_and_line_annotations_are_deterministic_and_keep_original(){
        let source=original();let original=source.bytes.clone();
        for operation in [
            ImageRevisionOperation::Ellipse{
                x:1,y:1,width:12,height:9,rgb:[200,30,90],stroke:2,
            },
            ImageRevisionOperation::Line{
                x:1,y:1,width:12,height:9,rgb:[20,230,90],stroke:2,
            },
        ]{
            let a=transform_admitted_image(&source,operation).unwrap();
            let b=transform_admitted_image(&source,operation).unwrap();
            assert_eq!(a.revision_sha256,b.revision_sha256);
            assert_ne!(a.revision_sha256,format!("{:x}",Sha256::digest(&source.bytes)));
            assert_eq!(source.bytes,original);
        }
        assert_eq!(
            transform_admitted_image(&source,ImageRevisionOperation::Ellipse{
                x:15,y:0,width:2,height:4,rgb:[255,0,0],stroke:1
            }),
            Err(ImageRevisionError::InvalidOperation)
        );
    }
    #[test]
    fn solid_redaction_is_opaque_and_does_not_reveal_original_pixels(){
        let mut data=Vec::new();
        let mut pixels=image::RgbaImage::new(8,8);
        for y in 0..8{
            for x in 0..8{
                pixels.put_pixel(x,y,image::Rgba([200,50,100,0]));
            }
        }
        DynamicImage::ImageRgba8(pixels).write_to(
            &mut Cursor::new(&mut data),ImageFormat::Png).unwrap();
        let source=admit_data_url(&format!("data:image/png;base64,{}",
            STANDARD.encode(data))).unwrap();
        let original=source.bytes.clone();
        let out=transform_admitted_image(&source,
            ImageRevisionOperation::Redact{x:2,y:3,width:3,height:2}).unwrap();
        let decoded=image::load_from_memory(&out.output_png).unwrap().to_rgba8();
        for y in 0..8{
            for x in 0..8{
                let pixel=decoded.get_pixel(x,y);
                if (2..5).contains(&x)&&(3..5).contains(&y){
                    assert_eq!(pixel.0,[0,0,0,255]);
                }else{
                    assert_eq!(pixel.0,[200,50,100,0]);
                }
            }
        }
        assert_eq!(source.bytes,original);
        assert_eq!(transform_admitted_image(&source,
            ImageRevisionOperation::Redact{x:7,y:7,width:2,height:1}),
            Err(ImageRevisionError::InvalidOperation));
    }
    #[test]
    fn image_plan_rejects_empty_or_excessive_stages_without_mutation(){
        let source=original();
        let baseline=source.bytes.clone();
        assert_eq!(transform_admitted_image_sequence(&source,&[]),
            Err(ImageRevisionError::InvalidOperation));
        let operations=vec![ImageRevisionOperation::FlipHorizontal;13];
        assert_eq!(transform_admitted_image_sequence(&source,&operations),
            Err(ImageRevisionError::InvalidOperation));
        let two=[ImageRevisionOperation::Crop{x:1,y:1,width:10,height:8},
            ImageRevisionOperation::Rotate90];
        let result=transform_admitted_image_sequence(&source,&two).unwrap();
        assert_eq!((result.width,result.height),(8,10));
        assert_eq!(result.base_ref,source.image_ref);
        assert_eq!(result.base_sha256,format!("{:x}",Sha256::digest(&source.bytes)));
        assert_eq!(source.bytes,baseline);
    }
    #[test]
    fn invalid_crop_oversized_output_and_changed_source_fail_closed(){
        let source=original();
        assert_eq!(transform_admitted_image(&source,
            ImageRevisionOperation::Crop{x:15,y:11,width:8,height:8}),
            Err(ImageRevisionError::InvalidOperation));
        assert_eq!(transform_admitted_image(&source,
            ImageRevisionOperation::Resize{width:8192,height:8192}),
            Err(ImageRevisionError::InvalidOperation));
        let mut changed=source.clone();changed.bytes.push(0);
        assert_eq!(transform_admitted_image(&changed,
            ImageRevisionOperation::Resize{width:4,height:4}),
            Err(ImageRevisionError::InvalidSource));
    }
}
