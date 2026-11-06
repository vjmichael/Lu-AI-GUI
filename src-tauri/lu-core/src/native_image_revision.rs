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
