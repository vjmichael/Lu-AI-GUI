//! WAVE4-IMAGE-WS-02: host-owned immutable deterministic PNG revision export.
//! The renderer may suggest geometry, but only already-admitted bytes and a
//! native user-selected destination can be used. No image model, script,
//! arbitrary edit tool or model-selected filesystem authority.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use lu_core::{
    multimodal_saved_turn::PinnedImage,
    native_image_revision::{
        ImageRevisionOperation,ImageRevision,
        transform_admitted_image,transform_admitted_image_sequence,
    },
};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::{
    fs::{self,OpenOptions},
    io::Write,
    path::Path,
};

#[derive(Debug,Clone,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct ImageRevisionStep{
    pub operation:String,
    pub x:Option<u32>,
    pub y:Option<u32>,
    pub width:u32,
    pub height:u32,
    pub block_size:Option<u32>,
    pub stroke:Option<u32>,
    pub color:Option<u32>,
}
impl ImageRevisionStep{
    pub(crate) fn operation(&self)->Result<ImageRevisionOperation,String>{
        use ImageRevisionOperation as Op;
        let x=||self.x.ok_or("Image region X coordinate missing".to_string());
        let y=||self.y.ok_or("Image region Y coordinate missing".to_string());
        Ok(match self.operation.as_str(){
            "crop"=>Op::Crop{x:x()?,y:y()?,width:self.width,height:self.height},
            "resize"=>Op::Resize{width:self.width,height:self.height},
            "rotate90"=>Op::Rotate90,
            "flip_h"=>Op::FlipHorizontal,
            "redact"=>Op::Redact{x:x()?,y:y()?,
                width:self.width,height:self.height},
            "pixelate"=>Op::Pixelate{x:x()?,y:y()?,
                width:self.width,height:self.height,
                block_size:self.block_size.unwrap_or(12)},
            "rectangle"|"line"|"ellipse"=>{
                let rgb=self.color.unwrap_or(0xff5500);
                if rgb>0xffffff{return Err("Invalid image annotation color".into());}
                let color=[(rgb>>16) as u8,(rgb>>8) as u8,rgb as u8];
                let x=x()?;let y=y()?;
                let stroke=self.stroke.unwrap_or(3);
                match self.operation.as_str(){
                    "line"=>Op::Line{
                        x,y,width:self.width,height:self.height,rgb:color,stroke
                    },
                    "ellipse"=>Op::Ellipse{
                        x,y,width:self.width,height:self.height,rgb:color,stroke
                    },
                    _=>Op::Rectangle{
                        x,y,width:self.width,height:self.height,rgb:color,stroke
                    },
                }
            },
            _=>return Err("Image assembly includes an unknown operation".into()),
        })
    }
}
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ImageExportReceipt{
    pub base_ref:String,
    pub base_sha256:String,
    pub revision_sha256:String,
    pub width:u32,
    pub height:u32,
    pub format:&'static str,
    pub file_name:String,
    pub bytes_written:usize,
    pub source_unchanged:bool,
    pub overwritten_existing_file:bool,
}
/// A volatile preview of an explicitly staged operator edit assembly.
/// The source remains pinned, and no file, journal, memory, network or
/// model-facing artifact is written. The result is returned to the existing
/// local renderer only, with a separate 4 MiB transport cap.
#[derive(Debug,Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ImageAssemblyPreview {
    pub base_ref:String,
    pub base_sha256:String,
    pub revision_sha256:String,
    pub width:u32,
    pub height:u32,
    pub png_bytes:usize,
    pub data_url:String,
    pub persisted:bool,
}
pub(crate) fn preview_revision_plan(
    source:&PinnedImage,steps:&[ImageRevisionStep]
)->Result<ImageAssemblyPreview,String>{
    if steps.is_empty()||steps.len()>12{
        return Err("Preview requires 1–12 staged image steps".into());
    }
    let operations=steps.iter().map(ImageRevisionStep::operation)
        .collect::<Result<Vec<_>,_>>()?;
    let revision=transform_admitted_image_sequence(source,&operations)
        .map_err(|e|format!("Native image preview refused: {e:?}"))?;
    if revision.output_png.len()>4*1024*1024{
        return Err("Preview PNG exceeds 4 MiB; export can still use native approval".into());
    }
    let png_bytes=revision.output_png.len();
    Ok(ImageAssemblyPreview{
        base_ref:revision.base_ref,
        base_sha256:revision.base_sha256,
        revision_sha256:revision.revision_sha256,
        width:revision.width,height:revision.height,png_bytes,
        data_url:format!("data:image/png;base64,{}",STANDARD.encode(&revision.output_png)),
        persisted:false,
    })
}

/// Caller MUST have shown the precise digest, dimensions and the OS Save
/// dialog. Direct tool/model access to this method is forbidden.
pub(crate) fn write_new_revision(
    source:&PinnedImage,operation:ImageRevisionOperation,destination:&Path
)->Result<ImageExportReceipt,String>{
    let result=transform_admitted_image(source,operation)
        .map_err(|e|format!("Image transform refused: {e:?}"))?;
    write_precomputed_revision(result,destination)
}
pub(crate) fn write_new_revision_plan(
    source:&PinnedImage,operations:&[ImageRevisionOperation],destination:&Path
)->Result<ImageExportReceipt,String>{
    let result=transform_admitted_image_sequence(source,operations)
        .map_err(|e|format!("Image assembly refused: {e:?}"))?;
    write_precomputed_revision(result,destination)
}
fn write_precomputed_revision(
    result:ImageRevision,destination:&Path
)->Result<ImageExportReceipt,String>{
    let name=destination.file_name().and_then(|s|s.to_str())
        .filter(|s|!s.is_empty()&&s.len()<=200)
        .ok_or("Invalid image output filename")?;
    if destination.extension().and_then(|s|s.to_str())
        .is_none_or(|x|!x.eq_ignore_ascii_case("png")){
        return Err("Image revisions must be new PNG files".into());
    }
    let parent=destination.parent().ok_or("Image destination has no folder")?;
    let folder=fs::symlink_metadata(parent)
        .map_err(|_|"Output folder no longer exists")?;
    if !folder.file_type().is_dir()||folder.file_type().is_symlink(){
        return Err("Image output folder is not an ordinary folder".into());
    }
    // No clobber: an existing file/reparse point/symlink fails because the
    // exclusive create_new option is used, not a pre-existing path check.
    let mut file=OpenOptions::new().write(true).create_new(true)
        .open(destination).map_err(|_|"Image destination exists or is not writable")?;
    let write_result=file.write_all(&result.output_png)
        .and_then(|_|file.sync_all());
    drop(file);
    if write_result.is_err(){
        let _=fs::remove_file(destination);
        return Err("Could not durably create image revision".into());
    }
    // The saved bytes were not decoded from UI base64, but from the original
    // pinned image; the output checksum is checked after the write.
    let written=match fs::read(destination){
        Ok(bytes)=>bytes,
        Err(_)=>{
            // An export cannot be reported as a successful immutable
            // revision unless the exact saved bytes were reopened.
            let _=fs::remove_file(destination);
            return Err("Cannot recheck saved image".into());
        }
    };
    let digest=format!("{:x}",Sha256::digest(&written));
    if digest!=result.revision_sha256{
        // Roll back this newly created output rather than leaving a corrupt
        // revision alongside an unmodified source. create_new above rules
        // out silent overwrite of any pre-existing document.
        let _=fs::remove_file(destination);
        return Err("Saved revision digest differs from intended output".into());
    }
    Ok(ImageExportReceipt{
        base_ref:result.base_ref,base_sha256:result.base_sha256,
        revision_sha256:result.revision_sha256,
        width:result.width,height:result.height,
        format:"image/png",file_name:name.to_owned(),
        bytes_written:written.len(),source_unchanged:true,
        overwritten_existing_file:false,
    })
}
#[cfg(test)]
mod tests{
    use super::*;
    use image::{DynamicImage,ImageFormat};
    use base64::{engine::general_purpose::STANDARD,Engine as _};
    use std::io::Cursor;
    use lu_core::multimodal_saved_turn::admit_data_url;
    fn source()->PinnedImage{
        let mut data=Vec::new();
        DynamicImage::new_rgba8(20,15)
            .write_to(&mut Cursor::new(&mut data),ImageFormat::Png).unwrap();
        admit_data_url(&format!("data:image/png;base64,{}",STANDARD.encode(data))).unwrap()
    }
    #[test]
    fn read_only_assembly_preview_is_deterministic_and_does_not_touch_source(){
        let input=source();
        let original=input.bytes.clone();
        let plan=[ImageRevisionStep{
            operation:"rectangle".into(),x:Some(1),y:Some(1),
            width:8,height:6,block_size:None,
            stroke:Some(2),color:Some(0x2255cc)
        }];
        let preview=preview_revision_plan(&input,&plan).unwrap();
        assert_eq!(preview.width,20);
        assert_eq!(preview.height,15);
        assert!(!preview.persisted);
        assert_eq!(preview.base_ref,input.image_ref);
        assert_eq!(input.bytes,original);
        let encoded=preview.data_url.strip_prefix("data:image/png;base64,").unwrap();
        let decoded=STANDARD.decode(encoded).unwrap();
        assert_eq!(preview.png_bytes,decoded.len());
        assert_eq!(preview.revision_sha256,format!("{:x}",Sha256::digest(decoded)));
        assert!(preview_revision_plan(&input,&[]).is_err());
        let repeated=preview_revision_plan(&input,&plan).unwrap();
        assert_eq!(preview.revision_sha256,repeated.revision_sha256);
    }
    #[test]
    fn immutable_new_file_and_no_clobber(){
        let input=source();let before=input.bytes.clone();
        let dir=std::env::temp_dir().join(format!("lu-image-test-{}",std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let target=dir.join("result.png");
        let _=fs::remove_file(&target);
        let r=write_new_revision(&input,
            ImageRevisionOperation::Crop{x:0,y:0,width:6,height:4},&target).unwrap();
        assert_eq!(r.width,6);assert_eq!(r.height,4);
        assert!(r.source_unchanged&&!r.overwritten_existing_file);
        assert_eq!(input.bytes,before);
        assert!(write_new_revision(&input,
            ImageRevisionOperation::Resize{width:8,height:8},&target).is_err());
        fs::remove_file(target).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }
}
