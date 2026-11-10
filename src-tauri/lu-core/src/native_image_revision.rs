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
            // Deterministic, CPU-only. Lanczos3 is reproducible with the
            // pinned image crate and does not invoke GPU inference.
            decoded.resize_exact(width,height,image::imageops::FilterType::Lanczos3)
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
