//! WAVE4-IMAGE-WS-02: host-owned immutable deterministic PNG revision export.
//! The renderer may suggest geometry, but only already-admitted bytes and a
//! native user-selected destination can be used. No image model, script,
//! arbitrary edit tool or model-selected filesystem authority.
use lu_core::{
    multimodal_saved_turn::PinnedImage,
    native_image_revision::{ImageRevisionOperation,transform_admitted_image},
};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{
    fs::{self,OpenOptions},
    io::Write,
    path::Path,
};

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
/// Caller MUST have shown the precise digest, dimensions and the OS Save
/// dialog. Direct tool/model access to this method is forbidden.
pub(crate) fn write_new_revision(
    source:&PinnedImage,operation:ImageRevisionOperation,destination:&Path
)->Result<ImageExportReceipt,String>{
    let result=transform_admitted_image(source,operation)
        .map_err(|e|format!("Image transform refused: {e:?}"))?;
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
    let written=fs::read(destination).map_err(|_|"Cannot recheck saved image")?;
    let digest=format!("{:x}",Sha256::digest(&written));
    if digest!=result.revision_sha256{
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
