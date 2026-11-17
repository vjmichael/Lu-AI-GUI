//! Exact original-path attachment read. No profile object copy, pathname
//! search, replacement fallback, or renderer/model-supplied filesystem grant.
//! The native caller must first select the path through an OS-mediated action.
use std::{fs::{self,Metadata,OpenOptions},io::Read,path::{Component,Path,PathBuf}};

pub(crate) const MAX_LINKED_FILE_BYTES:u64=8*1024*1024;
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub(crate) enum LinkFailure{Unavailable,Unsafe,Changed,InvalidSize}
impl LinkFailure{
    pub(crate) fn message(self)->&'static str{
        match self{
            Self::Unavailable=>"File not available. It may have been moved or deleted.",
            Self::Unsafe=>"File not available. Its location is no longer safe to reopen.",
            Self::Changed=>"File not available. It has changed; attach it again to use the new version.",
            Self::InvalidSize=>"Choose a nonempty file no larger than 8 MB.",
        }
    }
}
#[derive(Debug)]
pub(crate) struct OpenedLink{
    pub(crate) path:PathBuf,
    pub(crate) bytes:Vec<u8>,
}
fn safe_meta(meta:&Metadata)->bool{
    if meta.file_type().is_symlink(){return false}
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return false}
    }
    true
}
fn same_file_state(a:&Metadata,b:&Metadata)->bool{
    if !a.is_file()||!b.is_file()||!safe_meta(a)||!safe_meta(b)||a.len()!=b.len()||
        a.modified().ok()!=b.modified().ok(){return false}
    #[cfg(unix)]{
        use std::os::unix::fs::MetadataExt;
        if a.dev()!=b.dev()||a.ino()!=b.ino(){return false}
    }
    true
}
fn safe_absolute_path(path:&Path)->bool{
    if !path.is_absolute()||path.to_str().is_none_or(|s|s.len()>16*1024||s.chars().any(char::is_control))||
       path.components().any(|c|matches!(c,Component::ParentDir|Component::CurDir)){
        return false
    }
    #[cfg(windows)]{
        use std::path::Prefix;
        // Reject UNC, mapped network drives, device namespaces and ADS.
        let drive=match path.components().next(){
            Some(Component::Prefix(p))=>match p.kind(){Prefix::Disk(d)|Prefix::VerbatimDisk(d)=>d,_=>return false},
            _=>return false,
        };
        if path.components().any(|c|matches!(c,Component::Normal(v) if v.to_string_lossy().contains(':'))){return false}
        #[link(name="kernel32")]
        extern "system"{fn GetDriveTypeW(root:*const u16)->u32;}
        let root=[u16::from(drive),b':' as u16,b'\\' as u16,0];
        // DRIVE_REMOVABLE=2, DRIVE_FIXED=3, DRIVE_CDROM=5, DRIVE_RAMDISK=6.
        // DRIVE_REMOTE=4, unknown and no-root are deliberately refused.
        if !matches!(unsafe{GetDriveTypeW(root.as_ptr())},2|3|5|6){return false}
    }
    path.file_name().is_some()
}
fn check_path(path:&Path)->Result<Metadata,LinkFailure>{
    if !safe_absolute_path(path){return Err(LinkFailure::Unsafe)}
    let meta=fs::symlink_metadata(path).map_err(|_|LinkFailure::Unavailable)?;
    if !safe_meta(&meta)||!meta.is_file(){return Err(LinkFailure::Unsafe)}
    for parent in path.ancestors().skip(1){
        if parent.as_os_str().is_empty(){break}
        let m=fs::symlink_metadata(parent).map_err(|_|LinkFailure::Unavailable)?;
        if !safe_meta(&m)||!m.is_dir(){return Err(LinkFailure::Unsafe)}
    }
    Ok(meta)
}
/// Verify the exact selected spelling (not a moved-file search). Only bounded
/// bytes are retained in RAM for the current authorized operation. The
/// filesystem is never mutated. Caller separately verifies SHA-256 against
/// stored metadata before using any reopened bytes.
pub(crate) fn open_original(path:&Path,expected_bytes:Option<u64>)->Result<OpenedLink,LinkFailure>{
    let before=check_path(path)?;
    if before.len()==0||before.len()>MAX_LINKED_FILE_BYTES{return Err(LinkFailure::InvalidSize)}
    if expected_bytes.is_some_and(|n|n!=before.len()){return Err(LinkFailure::Changed)}
    let canonical=path.canonicalize().map_err(|_|LinkFailure::Unavailable)?;
    if !safe_absolute_path(&canonical){return Err(LinkFailure::Unsafe)}
    let mut options=OpenOptions::new();options.read(true);
    #[cfg(target_os="linux")]{
        use std::os::unix::fs::OpenOptionsExt;
        // O_NOFOLLOW | O_NONBLOCK: do not follow a replaced final symlink
        // or block forever opening a replaced FIFO.
        options.custom_flags(0x20000|0x800);
    }
    #[cfg(windows)]{
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let mut file=options.open(path).map_err(|_|LinkFailure::Unavailable)?;
    let opened=file.metadata().map_err(|_|LinkFailure::Unavailable)?;
    if !same_file_state(&before,&opened){return Err(LinkFailure::Changed)}
    let mut bytes=Vec::with_capacity(before.len() as usize);
    (&mut file).take(before.len()+1).read_to_end(&mut bytes).map_err(|_|LinkFailure::Unavailable)?;
    let after=file.metadata().map_err(|_|LinkFailure::Unavailable)?;
    let named=check_path(path)?;
    if bytes.len() as u64!=before.len()||!same_file_state(&opened,&after)||!same_file_state(&after,&named)||
        path.canonicalize().map_err(|_|LinkFailure::Unavailable)?!=canonical{
        return Err(LinkFailure::Changed)
    }
    // Persist original path, not the canonical alias of a moved target.
    Ok(OpenedLink{path:path.to_path_buf(),bytes})
}

#[cfg(test)]
mod tests{
    use super::*;
    fn root()->PathBuf{
        let p=std::env::temp_dir().join(format!("lu-linked-source-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&p).unwrap();p.canonicalize().unwrap()
    }
    #[test]
    fn selected_original_is_read_without_creating_a_copy(){
        let dir=root();let path=dir.join("report 日本語.txt");fs::write(&path,b"original").unwrap();
        let opened=open_original(&path,None).unwrap();assert_eq!(opened.path,path);assert_eq!(opened.bytes,b"original");
        assert_eq!(fs::read_dir(&dir).unwrap().count(),1);
        assert_eq!(open_original(&path,Some(8)).unwrap().bytes,b"original");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn moved_deleted_and_changed_size_are_explicit_failures(){
        let dir=root();let path=dir.join("first.txt");let moved=dir.join("moved.txt");
        fs::write(&path,b"original").unwrap();fs::rename(&path,&moved).unwrap();
        assert_eq!(open_original(&path,Some(8)).unwrap_err(),LinkFailure::Unavailable);
        fs::rename(&moved,&path).unwrap();assert_eq!(open_original(&path,Some(8)).unwrap().bytes,b"original");
        fs::write(&path,b"edited").unwrap();assert_eq!(open_original(&path,Some(8)).unwrap_err(),LinkFailure::Changed);
        fs::remove_file(&path).unwrap();assert_eq!(open_original(&path,Some(8)).unwrap_err(),LinkFailure::Unavailable);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rejects_directory_relative_empty_and_oversize(){
        let dir=root();let path=dir.join("file.txt");fs::write(&path,b"").unwrap();
        assert_eq!(open_original(&path,None).unwrap_err(),LinkFailure::InvalidSize);
        fs::File::create(&path).unwrap().set_len(MAX_LINKED_FILE_BYTES+1).unwrap();
        assert_eq!(open_original(&path,None).unwrap_err(),LinkFailure::InvalidSize);
        assert_eq!(open_original(&dir,None).unwrap_err(),LinkFailure::Unsafe);
        assert_eq!(open_original(Path::new("file.txt"),None).unwrap_err(),LinkFailure::Unsafe);
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn refuses_replaced_leaf_and_ancestor_symlinks(){
        use std::os::unix::fs::symlink;
        let dir=root();let other=dir.join("other");fs::create_dir(&other).unwrap();
        fs::write(other.join("file.txt"),b"original").unwrap();
        let leaf=dir.join("leaf.txt");symlink(other.join("file.txt"),&leaf).unwrap();
        let parent=dir.join("alias");symlink(&other,&parent).unwrap();
        assert_eq!(open_original(&leaf,None).unwrap_err(),LinkFailure::Unsafe);
        assert_eq!(open_original(&parent.join("file.txt"),None).unwrap_err(),LinkFailure::Unsafe);
        fs::remove_dir_all(dir).unwrap();
    }
}
