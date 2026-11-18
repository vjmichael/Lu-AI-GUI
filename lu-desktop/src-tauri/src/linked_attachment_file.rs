//! Exact original-path attachment read. No profile object copy, pathname
//! search, replacement fallback or renderer/model-supplied filesystem grant.
//! The native caller must first select the path through an OS-mediated action.
use std::{fs::{self,File,Metadata},io::Read,path::{Component,Path,PathBuf}};

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
    pub(crate) source_epoch:u64,
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
        if a.dev()!=b.dev()||a.ino()!=b.ino()||a.ctime()!=b.ctime()||a.ctime_nsec()!=b.ctime_nsec(){return false}
    }
    true
}
pub(crate) fn safe_absolute_path(path:&Path)->bool{
    if !path.is_absolute()||path.to_str().is_none_or(|s|s.len()>16*1024||s.chars().any(|c|c.is_control()||matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')))||
       path.components().any(|c|matches!(c,Component::ParentDir|Component::CurDir)){
        return false
    }
    #[cfg(windows)]{
        use std::path::Prefix;
        let drive=match path.components().next(){
            Some(Component::Prefix(p))=>match p.kind(){Prefix::Disk(d)|Prefix::VerbatimDisk(d)=>d,_=>return false},
            _=>return false,
        };
        if path.components().any(|c|matches!(c,Component::Normal(v) if v.to_string_lossy().contains(':')||
            v.to_string_lossy().ends_with(['.',' ']))){return false}
        #[link(name="kernel32")]
        extern "system"{fn GetDriveTypeW(root:*const u16)->u32;}
        let root=[u16::from(drive),b':' as u16,b'\\' as u16,0];
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
struct GuardedOpen{
    file:File,
    #[cfg(windows)]
    _ancestors:Vec<File>,
}
/// The kernel resolves the entire Linux path without following any symlink,
/// not merely the final component. Unsupported kernels fail closed; there
/// is no weaker ordinary open fallback. O_NONBLOCK rejects FIFO substitution.
#[cfg(all(target_os="linux",any(target_arch="x86_64",target_arch="aarch64")))]
fn open_guarded(path:&Path)->Result<GuardedOpen,LinkFailure>{
    use std::{ffi::{CString,c_long},os::{fd::{AsRawFd,FromRawFd},unix::ffi::OsStrExt}};
    #[repr(C)]
    struct OpenHow{flags:u64,mode:u64,resolve:u64}
    extern "C"{fn syscall(number:c_long,...)->c_long;}
    let root=File::open("/").map_err(|_|LinkFailure::Unsafe)?;
    let relative=path.strip_prefix("/").map_err(|_|LinkFailure::Unsafe)?;
    let relative=CString::new(relative.as_os_str().as_bytes()).map_err(|_|LinkFailure::Unsafe)?;
    // openat2(437) on supported Linux x86_64/aarch64. O_RDONLY is zero;
    // O_CLOEXEC|O_NOFOLLOW|O_NONBLOCK; RESOLVE_BENEATH|NO_SYMLINKS|NO_MAGICLINKS.
    let how=OpenHow{flags:0x80000|0x20000|0x800,mode:0,resolve:0x8|0x4|0x2};
    let fd=unsafe{syscall(437,root.as_raw_fd(),relative.as_ptr(),&how as *const OpenHow,std::mem::size_of::<OpenHow>())};
    if fd<0{return Err(LinkFailure::Unsafe)}
    Ok(GuardedOpen{file:unsafe{File::from_raw_fd(fd as i32)}})
}
/// Windows opens each ancestor from the root downward without following a
/// reparse point and holds read-attribute handles denying write/delete shares.
/// The file handle likewise denies write/delete until both reads are verified.
#[cfg(windows)]
fn open_guarded(path:&Path)->Result<GuardedOpen,LinkFailure>{
    use std::{fs::OpenOptions,os::windows::fs::OpenOptionsExt};
    let mut ancestors=Vec::new();
    let parents:Vec<&Path>=path.ancestors().skip(1).filter(|p|!p.as_os_str().is_empty()).collect();
    for parent in parents.into_iter().rev(){
        let dir=OpenOptions::new().read(true).access_mode(0x80).share_mode(1)
            .custom_flags(0x02000000|0x00200000).open(parent).map_err(|_|LinkFailure::Unsafe)?;
        let meta=dir.metadata().map_err(|_|LinkFailure::Unsafe)?;
        if !meta.is_dir()||!safe_meta(&meta){return Err(LinkFailure::Unsafe)}
        ancestors.push(dir);
    }
    let file=OpenOptions::new().read(true).share_mode(1).custom_flags(0x00200000)
        .open(path).map_err(|_|LinkFailure::Unavailable)?;
    Ok(GuardedOpen{file,_ancestors:ancestors})
}
#[cfg(not(any(windows,all(target_os="linux",any(target_arch="x86_64",target_arch="aarch64")))))]
fn open_guarded(_path:&Path)->Result<GuardedOpen,LinkFailure>{Err(LinkFailure::Unsafe)}
#[cfg(windows)]
pub(crate) fn handle_identity(file:&File)->Option<(u64,[u8;16])>{
    use std::{ffi::c_void,os::windows::io::AsRawHandle};
    #[repr(C)]
    struct FileId{volume:u64,id:[u8;16]}
    #[link(name="kernel32")]
    extern "system"{fn GetFileInformationByHandleEx(handle:*mut c_void,class:i32,info:*mut c_void,len:u32)->i32;}
    let mut result=FileId{volume:0,id:[0;16]};
    if unsafe{GetFileInformationByHandleEx(file.as_raw_handle(),18,(&mut result as *mut FileId).cast(),
        std::mem::size_of::<FileId>() as u32)}==0{return None}
    Some((result.volume,result.id))
}
#[cfg(windows)]
pub(crate) fn same_handle(a:&File,b:&File)->bool{
    matches!((handle_identity(a),handle_identity(b)),(Some(left),Some(right)) if left==right)
}
#[cfg(not(windows))]
fn same_handle(a:&File,b:&File)->bool{
    a.metadata().ok().zip(b.metadata().ok()).is_some_and(|(a,b)|same_file_state(&a,&b))
}
/// Only bounded bytes remain in RAM for the authorized operation. The caller
/// independently verifies the saved SHA-256; the file is never mutated.
pub(crate) struct HeldOriginal{
    path:PathBuf,before:Metadata,guarded:GuardedOpen,
}
impl HeldOriginal{
    pub(crate) fn hold(path:&Path,expected_bytes:Option<u64>)->Result<Self,LinkFailure>{
        let before=check_path(path)?;
        if before.len()==0||before.len()>MAX_LINKED_FILE_BYTES{return Err(LinkFailure::InvalidSize)}
        if expected_bytes.is_some_and(|n|n!=before.len()){return Err(LinkFailure::Changed)}
        let guarded=open_guarded(path)?;
        let opened=guarded.file.metadata().map_err(|_|LinkFailure::Unavailable)?;
        if !same_file_state(&before,&opened){return Err(LinkFailure::Changed)}
        let held=Self{path:path.to_path_buf(),before:opened,guarded};
        held.verify()?;Ok(held)
    }
    fn verify(&self)->Result<(),LinkFailure>{
        let current=self.guarded.file.metadata().map_err(|_|LinkFailure::Unavailable)?;
        let named=check_path(&self.path)?;
        let reopened=open_guarded(&self.path)?;
        if !same_file_state(&self.before,&current)||!same_file_state(&current,&named)||
           !same_handle(&self.guarded.file,&reopened.file){return Err(LinkFailure::Changed)}
        Ok(())
    }
    pub(crate) fn read(mut self)->Result<OpenedLink,LinkFailure>{
        self.verify()?;
        let mut bytes=Vec::with_capacity(self.before.len() as usize);
        (&mut self.guarded.file).take(self.before.len()+1).read_to_end(&mut bytes)
            .map_err(|_|LinkFailure::Unavailable)?;
        self.verify()?;
        if bytes.len() as u64!=self.before.len(){return Err(LinkFailure::Changed)}
        let source_epoch=self.before.modified().ok()
            .and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok())
            .and_then(|t|u64::try_from(t.as_nanos()).ok()).unwrap_or(0);
        Ok(OpenedLink{path:self.path,bytes,source_epoch})
    }
}
pub(crate) fn open_original(path:&Path,expected_bytes:Option<u64>)->Result<OpenedLink,LinkFailure>{
    HeldOriginal::hold(path,expected_bytes)?.read()
}

#[cfg(test)]
mod tests{
    use super::*;
    fn root()->PathBuf{
        let p=std::env::temp_dir().join(format!("lu-linked-source-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&p).unwrap();p.canonicalize().unwrap()
    }
    #[cfg(target_os="linux")]
    #[test]
    fn cp03c_held_exact_file_refuses_replacement_or_mutation_before_read(){
        let dir=root();let path=dir.join("report.txt");fs::write(&path,b"original").unwrap();
        let held=HeldOriginal::hold(&path,None).unwrap();
        fs::rename(&path,dir.join("parked.txt")).unwrap();fs::write(&path,b"hostile!").unwrap();
        assert!(held.read().is_err());
        let held=HeldOriginal::hold(&path,None).unwrap();fs::write(&path,b"changed!").unwrap();
        assert!(held.read().is_err());
        assert_eq!(HeldOriginal::hold(&path,None).unwrap().read().unwrap().bytes,b"changed!");
        fs::remove_dir_all(dir).unwrap();
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
    #[cfg(target_os="linux")]
    #[test]
    fn concurrent_ancestor_swaps_never_return_symlink_target_bytes(){
        use std::{os::unix::fs::symlink,sync::{Arc,atomic::{AtomicBool,Ordering}}};
        let dir=root();let parent=dir.join("parent");let parked=dir.join("parked");let other=dir.join("other");
        fs::create_dir(&parent).unwrap();fs::create_dir(&other).unwrap();
        let path=parent.join("report.txt");fs::write(&path,b"original").unwrap();
        fs::write(other.join("report.txt"),b"hostile!").unwrap();
        let stop=Arc::new(AtomicBool::new(false));let done=Arc::clone(&stop);
        let p=parent.clone();let worker=std::thread::spawn(move||{
            while !done.load(Ordering::Acquire){
                fs::rename(&p,&parked).unwrap();symlink(&other,&p).unwrap();
                std::thread::yield_now();fs::remove_file(&p).unwrap();fs::rename(&parked,&p).unwrap();
            }
        });
        let mut hostile=false;
        for _ in 0..2000{if let Ok(link)=open_original(&path,Some(8)){hostile|=link.bytes!=b"original";}}
        stop.store(true,Ordering::Release);worker.join().unwrap();
        assert!(!hostile);assert_eq!(open_original(&path,Some(8)).unwrap().bytes,b"original");
        fs::remove_dir_all(dir).unwrap();
    }
}
