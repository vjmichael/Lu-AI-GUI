//! WAVE2 native archive/codec ABI bridge. NO shell, PATH, guessed library,
//! user-selected DLL, external program or implicit model authority.
//! Windows loads a pinned app-owned lu-native-codecs/archive.dll alongside
//! its executable. Linux opens a fixed distro-owned system libarchive.
//! Callers MUST enforce full input/output limits and exact approval separately.
//! libarchive may reject encrypted/unsupported 7z or RAR variants: fail closed.
use std::{ffi::{c_void,c_char,CStr,CString},path::PathBuf,ptr};
const MAX_INPUT:usize=128*1024*1024;
const MAX_OUTPUT:usize=256*1024*1024;
const MAX_MEMBER:usize=64*1024*1024;
const MAX_ENTRIES:usize=4096;
const BLOCK:usize=64*1024;
const FILETYPE_MASK:u32=0o170000;
const REGULAR:u32=0o100000;
const DIRECTORY:u32=0o040000;
const FORMAT_7Z:i32=0xE0000;
const FORMAT_RAR:i32=0xD0000;
const FORMAT_RAR5:i32=0x100000;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum NativeFormat{SevenZip,Rar,RawXz,RawBzip2,RawZstd}
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum NativeArchiveError{
    BackendMissing,UnsupportedFormat,CorruptArchive,EncryptedOrUnsupported,
    MaliciousEntry,TooLarge,DuplicateEntry,InvalidName,NativeFailure,Cancelled,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct NativeMember{pub path:String,pub is_directory:bool,pub bytes:Vec<u8>}
#[cfg(windows)]
mod platform{
    use super::*;
    #[link(name="kernel32")]
    unsafe extern "system"{
        fn LoadLibraryExW(file:*const u16,handle:*mut c_void,flags:u32)->*mut c_void;
        fn GetProcAddress(module:*mut c_void,name:*const u8)->*mut c_void;
        fn FreeLibrary(module:*mut c_void)->i32;
    }
    // LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR + LOAD_LIBRARY_SEARCH_DEFAULT_DIRS.
    // Include DLL dependencies in same trusted subdirectory.
    const LOAD_FLAGS:u32=0x00000100|0x00001000;
    fn manifest_verified(folder:&std::path::Path)->bool{
        use sha2::{Digest,Sha256};
        use std::os::windows::fs::MetadataExt;
        let regular=|path:&std::path::Path,is_dir:bool|{
            let Ok(meta)=std::fs::symlink_metadata(path)else{return false};
            !meta.file_type().is_symlink() &&
                meta.file_attributes()&0x400==0 &&
                if is_dir{meta.is_dir()}else{meta.is_file()}
        };
        if !regular(folder,true)||
            !regular(&folder.join("manifest.json"),false){return false}
        let Ok(contents)=std::fs::read(folder.join("manifest.json"))else{return false};
        if contents.len()>64*1024{return false}
        let Ok(doc)=serde_json::from_slice::<serde_json::Value>(&contents)else{return false};
        if doc["schema"].as_u64()!=Some(1){return false}
        let Some(files)=doc["files"].as_array()else{return false};
        if files.len()<2||files.len()>128{return false}
        let mut names=std::collections::BTreeSet::new();
        for item in files{
            let Some(name)=item["name"].as_str()else{return false};
            let Some(hash)=item["sha256"].as_str()else{return false};
            let Some(expected)=item["bytes"].as_u64()else{return false};
            if !name.to_ascii_lowercase().ends_with(".dll")||
                name.is_empty()||name.len()>128||
                !name.bytes().all(|b|b.is_ascii_alphanumeric()||
                    matches!(b,b'.'|b'-'|b'_'))||
                hash.len()!=64||
                !hash.bytes().all(|b|b.is_ascii_digit()||
                    (b'a'..=b'f').contains(&b))||
                !names.insert(name.to_ascii_lowercase()){
                return false
            }
            if !regular(&folder.join(name),false){return false}
            let Ok(contents)=std::fs::read(folder.join(name))else{return false};
            if contents.len() as u64!=expected||expected==0||
                format!("{:x}",Sha256::digest(&contents))!=hash{
                return false
            }
        }
        if !names.contains("archive.dll")&&!names.contains("libarchive.dll"){
            return false
        }
        // Manifest may not omit any executable DLL present in its folder.
        let Ok(entries)=std::fs::read_dir(folder)else{return false};
        for entry in entries{
            let Ok(entry)=entry else{return false};
            let file_name=entry.file_name();
            let Some(name)=file_name.to_str()else{return false};
            if name.to_ascii_lowercase().ends_with(".dll")&&
                !names.contains(&name.to_ascii_lowercase()){
                return false
            }
        }
        true
    }
    pub unsafe fn open()->Result<*mut c_void,NativeArchiveError>{
        let exe=std::env::current_exe().map_err(|_|NativeArchiveError::BackendMissing)?;
        let parent=exe.parent().ok_or(NativeArchiveError::BackendMissing)?;
        let trusted=parent.join("lu-native-codecs");
        if !manifest_verified(&trusted){
            return Err(NativeArchiveError::BackendMissing)
        }
        for name in ["archive.dll","libarchive.dll"]{
            let file=parent.join("lu-native-codecs").join(name);
            if !file.is_file(){continue}
            use std::os::windows::ffi::OsStrExt;
            let mut name=file.as_os_str().encode_wide().collect::<Vec<u16>>();
            name.push(0);
            let handle=LoadLibraryExW(name.as_ptr(),ptr::null_mut(),LOAD_FLAGS);
            if !handle.is_null(){return Ok(handle)}
        }
        Err(NativeArchiveError::BackendMissing)
    }
    pub unsafe fn symbol(handle:*mut c_void,name:&[u8])->*mut c_void{
        GetProcAddress(handle,name.as_ptr())
    }
    pub unsafe fn close(handle:*mut c_void){let _=FreeLibrary(handle);}
}
#[cfg(all(unix,not(target_os="macos")))]
mod platform{
    use super::*;
    #[link(name="dl")]
    unsafe extern "C"{
        fn dlopen(path:*const c_char,flags:i32)->*mut c_void;
        fn dlsym(handle:*mut c_void,symbol:*const c_char)->*mut c_void;
        fn dlclose(handle:*mut c_void)->i32;
    }
    pub unsafe fn open()->Result<*mut c_void,NativeArchiveError>{
        // Explicit distro-owned library locations. Never consult LD_LIBRARY_PATH.
        for file in [
            "/usr/lib/x86_64-linux-gnu/libarchive.so.13",
            "/usr/lib/aarch64-linux-gnu/libarchive.so.13",
            "/usr/lib64/libarchive.so.13",
            "/lib/x86_64-linux-gnu/libarchive.so.13",
            "/lib/aarch64-linux-gnu/libarchive.so.13",
            "/usr/lib/libarchive.so.13",
        ]{
            let name=CString::new(file).map_err(|_|NativeArchiveError::BackendMissing)?;
            let handle=dlopen(name.as_ptr(),2);
            if !handle.is_null(){return Ok(handle)}
        }
        Err(NativeArchiveError::BackendMissing)
    }
    pub unsafe fn symbol(handle:*mut c_void,name:&[u8])->*mut c_void{
        dlsym(handle,name.as_ptr().cast())
    }
    pub unsafe fn close(handle:*mut c_void){let _=dlclose(handle);}
}
#[cfg(target_os="macos")]
mod platform{
    use super::*;
    #[link(name="System")]
    unsafe extern "C"{
        fn dlopen(path:*const c_char,flags:i32)->*mut c_void;
        fn dlsym(handle:*mut c_void,symbol:*const c_char)->*mut c_void;
        fn dlclose(handle:*mut c_void)->i32;
    }
    pub unsafe fn open()->Result<*mut c_void,NativeArchiveError>{
        let filename=CString::new("/usr/lib/libarchive.2.dylib").unwrap();
        let handle=dlopen(filename.as_ptr(),2);
        if handle.is_null(){Err(NativeArchiveError::BackendMissing)}else{Ok(handle)}
    }
    pub unsafe fn symbol(handle:*mut c_void,name:&[u8])->*mut c_void{dlsym(handle,name.as_ptr().cast())}
    pub unsafe fn close(handle:*mut c_void){let _=dlclose(handle);}
}
type New=unsafe extern "C" fn()->*mut c_void;
type Free=unsafe extern "C" fn(*mut c_void)->i32;
type EntryFree=unsafe extern "C" fn(*mut c_void);
type One=unsafe extern "C" fn(*mut c_void)->i32;
type OpenMemory=unsafe extern "C" fn(*mut c_void,*const c_void,usize)->i32;
type Next=unsafe extern "C" fn(*mut c_void,*mut *mut c_void)->i32;
type Data=unsafe extern "C" fn(*mut c_void,*mut c_void,usize)->isize;
type Format=unsafe extern "C" fn(*mut c_void)->i32;
type FilterCode=unsafe extern "C" fn(*mut c_void,i32)->i32;
type Path=unsafe extern "C" fn(*mut c_void)->*const c_char;
type Mode=unsafe extern "C" fn(*mut c_void)->u32;
type Size=unsafe extern "C" fn(*mut c_void)->i64;
type Encrypted=unsafe extern "C" fn(*mut c_void)->i32;
type SetFiletype=unsafe extern "C" fn(*mut c_void,u32);
type SetSize=unsafe extern "C" fn(*mut c_void,i64);
type SetPath=unsafe extern "C" fn(*mut c_void,*const c_char);
type WriteOpen=unsafe extern "C" fn(
    *mut c_void,*mut c_void,Option<unsafe extern "C" fn(*mut c_void,*mut c_void)->i32>,
    Option<unsafe extern "C" fn(*mut c_void,*mut c_void,*const c_void,usize)->isize>,
    Option<unsafe extern "C" fn(*mut c_void,*mut c_void)->i32>,
)->i32;
type Header=unsafe extern "C" fn(*mut c_void,*mut c_void)->i32;
type WriteData=unsafe extern "C" fn(*mut c_void,*const c_void,usize)->isize;
struct Library{handle:*mut c_void}
impl Drop for Library{fn drop(&mut self){unsafe{platform::close(self.handle)}}}
impl Library{
    fn new()->Result<Self,NativeArchiveError>{Ok(Self{handle:unsafe{platform::open()?}})}
    unsafe fn required<T:Copy>(&self,name:&'static [u8])->Result<T,NativeArchiveError>{
        let ptr=platform::symbol(self.handle,name);
        if ptr.is_null(){return Err(NativeArchiveError::BackendMissing)}
        Ok(std::mem::transmute_copy::<*mut c_void,T>(&ptr))
    }
}
struct Archive<'a>{lib:&'a Library,handle:*mut c_void,free:Free}
impl Drop for Archive<'_>{
    fn drop(&mut self){if !self.handle.is_null(){unsafe{(self.free)(self.handle);}}}
}
fn valid_path(path:&str,dir:bool)->Result<Option<String>,NativeArchiveError>{
    if path=="."&&dir{return Ok(None)}
    let path=path.strip_prefix("./").unwrap_or(path);
    let path=if dir{path.trim_end_matches('/')}else{path};
    if path.is_empty()||path.len()>1024||path.starts_with('/')||
        path.contains('\0')||path.chars().any(|c|c.is_control()||matches!(c,'\\'|':')){
        return Err(NativeArchiveError::InvalidName)
    }
    let parts=path.split('/').collect::<Vec<_>>();
    if parts.len()>16||parts.iter().any(|p|
        p.is_empty()||*p=="."||*p==".."||p.len()>200
        ||p.starts_with(".lu-")||p.starts_with('-')
        ||p.eq_ignore_ascii_case(".git")
        ||p.ends_with(' ')||p.ends_with('.')){
        return Err(NativeArchiveError::MaliciousEntry)
    }
    Ok(Some(path.into()))
}
fn check_source(data:&[u8])->Result<(),NativeArchiveError>{
    if data.is_empty(){return Err(NativeArchiveError::CorruptArchive)}
    if data.len()>MAX_INPUT{return Err(NativeArchiveError::TooLarge)}
    Ok(())
}
fn configured_reader<'a>(lib:&'a Library,format:NativeFormat)
    ->Result<Archive<'a>,NativeArchiveError>{
    unsafe{
        let new:New=lib.required(b"archive_read_new\0")?;
        let free:Free=lib.required(b"archive_read_free\0")?;
        let handle=new();
        if handle.is_null(){return Err(NativeArchiveError::NativeFailure)}
        let a=Archive{lib,handle,free};
        let filter:One=lib.required(b"archive_read_support_filter_all\0")?;
        // Allow non-program libarchive filters; never enable program filters.
        if filter(a.handle)<0{return Err(NativeArchiveError::BackendMissing)}
        let sym=match format{
            NativeFormat::SevenZip=>b"archive_read_support_format_7zip\0".as_slice(),
            NativeFormat::Rar=>b"archive_read_support_format_rar\0".as_slice(),
            NativeFormat::RawXz|NativeFormat::RawBzip2|NativeFormat::RawZstd=>
                b"archive_read_support_format_raw\0".as_slice(),
        };
        let enable:One=lib.required(sym)?;
        if enable(a.handle)<0{return Err(NativeArchiveError::UnsupportedFormat)}
        if format==NativeFormat::Rar{
            let rar5:One=lib.required(b"archive_read_support_format_rar5\0")?;
            if rar5(a.handle)<0{return Err(NativeArchiveError::UnsupportedFormat)}
        }
        Ok(a)
    }
}
pub fn decode_members(
    format:NativeFormat,source:&[u8]
)->Result<Vec<NativeMember>,NativeArchiveError>{
    decode_members_cancellable(format,source,&||false)
}
/// The caller supplies a host-owned cancellation predicate, never a model
/// argument. Checks occur before native open and between each decoded chunk.
pub fn decode_members_cancellable(
    format:NativeFormat,source:&[u8],cancelled:&dyn Fn()->bool
)->Result<Vec<NativeMember>,NativeArchiveError>{
    if cancelled(){return Err(NativeArchiveError::Cancelled)}
    if !matches!(format,NativeFormat::SevenZip|NativeFormat::Rar){
        return Err(NativeArchiveError::UnsupportedFormat)
    }
    check_source(source)?;
    let lib=Library::new()?;
    let read=configured_reader(&lib,format)?;
    unsafe{
        let open:OpenMemory=lib.required(b"archive_read_open_memory\0")?;
        let next:Next=lib.required(b"archive_read_next_header\0")?;
        let data:Data=lib.required(b"archive_read_data\0")?;
        let mode:Mode=lib.required(b"archive_entry_filetype\0")?;
        let size:Size=lib.required(b"archive_entry_size\0")?;
        let path:Path=lib.required(b"archive_entry_pathname\0")?;
        let symlink:Path=lib.required(b"archive_entry_symlink\0")?;
        let hardlink:Path=lib.required(b"archive_entry_hardlink\0")?;
        let format_id:Format=lib.required(b"archive_format\0")?;
        let encrypted:Encrypted=lib.required(b"archive_entry_is_encrypted\0")?;
        if open(read.handle,source.as_ptr().cast(),source.len())<0{
            return Err(NativeArchiveError::CorruptArchive)
        }
        let mut items=Vec::new();let mut seen=std::collections::BTreeSet::new();
        let mut total=0usize;let mut file_count=0usize;
        loop{
            if cancelled(){return Err(NativeArchiveError::Cancelled)}
            let mut entry=ptr::null_mut();
            let status=next(read.handle,&mut entry);
            if status==1{break}
            if status!=0||entry.is_null(){return Err(NativeArchiveError::CorruptArchive)}
            let detected=format_id(read.handle)&0xff0000;
            if format==NativeFormat::SevenZip&&detected!=FORMAT_7Z||
                format==NativeFormat::Rar&&detected!=FORMAT_RAR&&detected!=FORMAT_RAR5 {
                return Err(NativeArchiveError::UnsupportedFormat)
            }
            if encrypted(entry)!=0||!symlink(entry).is_null()||!hardlink(entry).is_null(){
                return Err(NativeArchiveError::EncryptedOrUnsupported)
            }
            let kind=mode(entry)&FILETYPE_MASK;
            let directory=if kind==DIRECTORY{true}else if kind==REGULAR{false}else{
                return Err(NativeArchiveError::MaliciousEntry)
            };
            let declared=size(entry);
            if declared<0||declared as u64>MAX_MEMBER as u64||
                directory&&declared!=0{
                return Err(NativeArchiveError::TooLarge)
            }
            let raw=path(entry);
            if raw.is_null(){return Err(NativeArchiveError::InvalidName)}
            let name=CStr::from_ptr(raw).to_str()
                .map_err(|_|NativeArchiveError::InvalidName)?;
            let Some(name)=valid_path(name,directory)? else{continue};
            if !seen.insert(name.to_lowercase()){
                return Err(NativeArchiveError::DuplicateEntry)
            }
            if items.len()>=MAX_ENTRIES{return Err(NativeArchiveError::TooLarge)}
            let mut bytes=Vec::new();let mut buffer=[0u8;BLOCK];
            if !directory{
                loop{
                    if cancelled(){return Err(NativeArchiveError::Cancelled)}
                    let read_bytes=data(read.handle,buffer.as_mut_ptr().cast(),BLOCK);
                    if read_bytes<0{return Err(NativeArchiveError::CorruptArchive)}
                    if read_bytes==0{break}
                    let n=read_bytes as usize;
                    if bytes.len().checked_add(n).is_none_or(|sum|sum>MAX_MEMBER){
                        return Err(NativeArchiveError::TooLarge)
                    }
                    bytes.extend_from_slice(&buffer[..n]);
                }
                if bytes.len() as i64!=declared{
                    return Err(NativeArchiveError::CorruptArchive)
                }
                file_count+=1;
                total=total.checked_add(bytes.len()).ok_or(NativeArchiveError::TooLarge)?;
                if total>MAX_OUTPUT{return Err(NativeArchiveError::TooLarge)}
            }
            items.push(NativeMember{path:name,is_directory:directory,bytes});
        }
        if cancelled(){return Err(NativeArchiveError::Cancelled)}
        if file_count==0{return Err(NativeArchiveError::CorruptArchive)}
        Ok(items)
    }
}
pub fn expand_raw(
    format:NativeFormat,source:&[u8]
)->Result<Vec<u8>,NativeArchiveError>{
    if !matches!(format,NativeFormat::RawXz|NativeFormat::RawBzip2|NativeFormat::RawZstd){
        return Err(NativeArchiveError::UnsupportedFormat)
    }
    check_source(source)?;
    let lib=Library::new()?;
    let read=configured_reader(&lib,format)?;
    unsafe{
        let open:OpenMemory=lib.required(b"archive_read_open_memory\0")?;
        let next:Next=lib.required(b"archive_read_next_header\0")?;
        let data:Data=lib.required(b"archive_read_data\0")?;
        let filter_code:FilterCode=lib.required(b"archive_filter_code\0")?;
        if open(read.handle,source.as_ptr().cast(),source.len())<0{
            return Err(NativeArchiveError::CorruptArchive)
        }
        let mut entry=ptr::null_mut();
        if next(read.handle,&mut entry)!=0||entry.is_null(){
            return Err(NativeArchiveError::CorruptArchive)
        }
        // libarchive's RAW format also accepts arbitrary uncompressed bytes.
        // Require the actual requested native compression FILTER, not only
        // a successful RAW header, or mislabeled payloads could pass through.
        let expected_filter=match format{
            NativeFormat::RawXz=>6,
            NativeFormat::RawBzip2=>2,
            NativeFormat::RawZstd=>14,
            _=>unreachable!(),
        };
        if filter_code(read.handle,0)!=expected_filter{
            return Err(NativeArchiveError::UnsupportedFormat)
        }
        let mut out=Vec::new();let mut buffer=[0u8;BLOCK];
        loop{
            let n=data(read.handle,buffer.as_mut_ptr().cast(),BLOCK);
            if n<0{return Err(NativeArchiveError::CorruptArchive)}
            if n==0{break}
            if out.len().checked_add(n as usize).is_none_or(|v|v>MAX_OUTPUT){
                return Err(NativeArchiveError::TooLarge)
            }
            out.extend_from_slice(&buffer[..n as usize]);
        }
        let mut another=ptr::null_mut();
        if next(read.handle,&mut another)!=1{
            return Err(NativeArchiveError::CorruptArchive)
        }
        Ok(out)
    }
}
struct OutputBuffer{data:Vec<u8>,exceeded:bool}
unsafe extern "C" fn write_open(_: *mut c_void,_:*mut c_void)->i32{0}
unsafe extern "C" fn write_close(_: *mut c_void,_:*mut c_void)->i32{0}
unsafe extern "C" fn write_bytes(_: *mut c_void,opaque:*mut c_void,
    raw:*const c_void,n:usize)->isize{
    if opaque.is_null()||raw.is_null(){return -1}
    let out=&mut *(opaque as *mut OutputBuffer);
    if out.data.len().checked_add(n).is_none_or(|x|x>MAX_INPUT){
        out.exceeded=true;return -1
    }
    let slice=std::slice::from_raw_parts(raw as *const u8,n);
    out.data.extend_from_slice(slice);
    n as isize
}
pub fn compress_raw(
    format:NativeFormat,source:&[u8]
)->Result<Vec<u8>,NativeArchiveError>{
    if !matches!(format,NativeFormat::RawXz|NativeFormat::RawBzip2|NativeFormat::RawZstd){
        return Err(NativeArchiveError::UnsupportedFormat)
    }
    if source.len()>MAX_OUTPUT{return Err(NativeArchiveError::TooLarge)}
    let lib=Library::new()?;
    unsafe{
        let new:New=lib.required(b"archive_write_new\0")?;
        let free:Free=lib.required(b"archive_write_free\0")?;
        let handle=new();
        if handle.is_null(){return Err(NativeArchiveError::NativeFailure)}
        let write=Archive{lib:&lib,handle,free};
        // RAW compression is an exact byte stream, not a padded archive
        // container. libarchive's default 10-KiB block finalization can
        // append zero padding (Zstandard sees this as a bogus second frame).
        // Set BOTH knobs: disable block aggregation and last-block padding.
        // Keep the reader's exact compression-filter and EOF validation.
        type BlockOption=unsafe extern "C" fn(*mut c_void,i32)->i32;
        let per_block:BlockOption=lib.required(b"archive_write_set_bytes_per_block\0")?;
        let final_block:BlockOption=lib.required(b"archive_write_set_bytes_in_last_block\0")?;
        if per_block(write.handle,0)!=0||final_block(write.handle,1)!=0{
            return Err(NativeArchiveError::BackendMissing)
        }
        let raw:One=lib.required(b"archive_write_set_format_raw\0")?;
        if raw(write.handle)!=0{return Err(NativeArchiveError::UnsupportedFormat)}
        let filter=match format{
            NativeFormat::RawXz=>b"archive_write_add_filter_xz\0".as_slice(),
            NativeFormat::RawBzip2=>b"archive_write_add_filter_bzip2\0".as_slice(),
            NativeFormat::RawZstd=>b"archive_write_add_filter_zstd\0".as_slice(),
            _=>unreachable!(),
        };
        let filter_fn:One=lib.required(filter)?;
        if filter_fn(write.handle)!=0{return Err(NativeArchiveError::BackendMissing)}
        let mut output=OutputBuffer{data:Vec::new(),exceeded:false};
        let open:WriteOpen=lib.required(b"archive_write_open\0")?;
        if open(write.handle,(&mut output as *mut OutputBuffer).cast(),
            Some(write_open),Some(write_bytes),Some(write_close))!=0{
            return Err(NativeArchiveError::NativeFailure)
        }
        let entry_new:New=lib.required(b"archive_entry_new\0")?;
        let entry_free:EntryFree=lib.required(b"archive_entry_free\0")?;
        let entry=entry_new();
        if entry.is_null(){return Err(NativeArchiveError::NativeFailure)}
        struct Entry(*mut c_void,EntryFree);
        impl Drop for Entry{fn drop(&mut self){unsafe{(self.1)(self.0);}}}
        let entry=Entry(entry,entry_free);
        let set_name:SetPath=lib.required(b"archive_entry_set_pathname\0")?;
        let set_mode:SetFiletype=lib.required(b"archive_entry_set_filetype\0")?;
        let set_size:SetSize=lib.required(b"archive_entry_set_size\0")?;
        set_name(entry.0,b"data\0".as_ptr().cast());
        set_mode(entry.0,REGULAR);
        set_size(entry.0,source.len() as i64);
        let header:Header=lib.required(b"archive_write_header\0")?;
        let data:WriteData=lib.required(b"archive_write_data\0")?;
        let close:One=lib.required(b"archive_write_close\0")?;
        if header(write.handle,entry.0)!=0{return Err(NativeArchiveError::NativeFailure)}
        let mut offset=0;
        while offset<source.len(){
            let n=data(write.handle,source[offset..].as_ptr().cast(),
                (source.len()-offset).min(BLOCK));
            if n<=0{return Err(NativeArchiveError::NativeFailure)}
            offset+=n as usize;
        }
        if close(write.handle)!=0||output.exceeded{
            return Err(if output.exceeded{
                NativeArchiveError::TooLarge
            }else{NativeArchiveError::NativeFailure})
        }
        // The native writer has callbacks into the OutputBuffer. Release it
        // BEFORE moving the buffer into the returned Vec.
        drop(entry);
        drop(write);
        Ok(output.data)
    }
}
pub fn available()->bool{Library::new().is_ok()}
