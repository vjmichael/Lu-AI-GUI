//! Read-only, fail-closed 7z and RAR/RAR5 archive inspection through
//! vetted libarchive ABI; never execute tools, mount archives, or restore ACLs.
//! Only extracted regular file bytes / directories can reach the separate
//! exact-manifest, non-overwriting archive-tree publication gate.
//! Encrypted, multi-volume, partial, damaged, link or special entries fail.
use std::io::Read;
use cap_std::fs::Dir;
use crate::native_libarchive::{decode_members,NativeFormat,NativeArchiveError};
const MAX_SOURCE:u64=128*1024*1024;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum SevenRarError {
    UnsupportedFormat,NotRegular,BackendUnavailable,UnsafeArchive,
    EncryptedOrUnsupported,TooLarge,Io(String),
}
fn allowed_name(name:&str)->bool{
    !name.is_empty()&&name.len()<=240&&!name.starts_with(".lu-")
        &&!name.ends_with(' ')&&!name.ends_with('.')
        &&!name.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
pub fn detect_7z_rar(name:&str)->Option<NativeFormat>{
    let n=name.to_ascii_lowercase();
    if n.ends_with(".7z"){Some(NativeFormat::SevenZip)}
    else if n.ends_with(".rar"){Some(NativeFormat::Rar)}
    else{None}
}
pub fn seven_rar_tree_members(
    root:&Dir,name:&str
)->Result<Vec<(String,bool,Vec<u8>)>,SevenRarError>{
    let kind=detect_7z_rar(name).ok_or(SevenRarError::UnsupportedFormat)?;
    if !allowed_name(name){return Err(SevenRarError::UnsafeArchive)}
    let meta=root.symlink_metadata(name).map_err(|e|SevenRarError::Io(e.to_string()))?;
    if !meta.is_file()||meta.file_type().is_symlink()||
        meta.len()>MAX_SOURCE{return Err(SevenRarError::NotRegular)}
    #[cfg(windows)] {
        // cap_std::fs::Metadata does not expose Windows file_attributes;
        // preserve the no-follow symlink check on capability-scoped metadata.
        if meta.file_type().is_symlink() { return Err(SevenRarError::NotRegular) }
    }
    let file=root.open(name).map_err(|e|SevenRarError::Io(e.to_string()))?;
    let opened=file.metadata().map_err(|e|SevenRarError::Io(e.to_string()))?;
    if !opened.is_file()||opened.len()>MAX_SOURCE{
        return Err(SevenRarError::NotRegular)
    }
    let mut data=Vec::new();
    file.take(MAX_SOURCE+1).read_to_end(&mut data)
        .map_err(|e|SevenRarError::Io(e.to_string()))?;
    if data.len() as u64>MAX_SOURCE{return Err(SevenRarError::TooLarge)}
    let files=decode_members(kind,&data).map_err(|e|match e{
        NativeArchiveError::BackendMissing=>SevenRarError::BackendUnavailable,
        NativeArchiveError::TooLarge=>SevenRarError::TooLarge,
        NativeArchiveError::EncryptedOrUnsupported=>
            SevenRarError::EncryptedOrUnsupported,
        _=>SevenRarError::UnsafeArchive,
    })?;
    Ok(files.into_iter().map(|e|(e.path,e.is_directory,e.bytes)).collect())
}
