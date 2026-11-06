//! TOOLS-LINUX-ARCHIVE: bounded TAR and conventional Linux compression
//! primitives. Source and destination are host-opened Dir capabilities, not
//! model supplied root paths. No shell, mount, symlink, device or ownership
//! restoration. Linux xz/bzip2/zstd use fixed absolute trusted system binaries;
//! gzip uses in-process flate2. These are not independently authorized tools.
use std::{
    collections::{BTreeMap,BTreeSet},
    io::{Read,Write},
    process::{Command,Stdio},
    sync::{Arc,mpsc},
    thread,
    time::Duration,
};
use cap_std::fs::{Dir,OpenOptions};
use flate2::{read::MultiGzDecoder,write::GzEncoder,Compression};
use sha2::{Digest,Sha256};

const MAX_ARCHIVE:u64=128*1024*1024;
const MAX_UNPACKED:usize=256*1024*1024;
const MAX_ENTRY:usize=128*1024*1024;
const MAX_ENTRIES:usize=4096;
const MAX_PATH:usize=1024;
const MAX_PAX:usize=64*1024;
const MAX_SOURCE_FILES:usize=1024;
const SECTOR:usize=512;
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum LinuxArchiveError {
    UnsupportedFormat,UnsupportedPlatform,BackendUnavailable,BackendFailed,
    InvalidName,InvalidHeader,InvalidChecksum,InvalidEncoding,UnsafeEntry,
    DuplicateEntry,UnsupportedEntry,TooManyEntries,TooLarge,NotFile,
    NotFound,TargetExists,StageCollision,SourceChanged,
    PublishedNeedsReconciliation,Io(String),
}
#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub enum LinuxArchiveFormat {
    Tar,TarGz,TarXz,TarBz2,TarZst,Gzip,Xz,Bzip2,Zstd,
}
impl LinuxArchiveFormat{
    fn is_tar(self)->bool{matches!(self,Self::Tar|Self::TarGz|Self::TarXz|Self::TarBz2|Self::TarZst)}
    fn compressor(self)->Option<Self>{match self{
        Self::Tar|Self::TarGz|Self::Gzip=>if self==Self::Tar{None}else{Some(Self::Gzip)},
        Self::TarXz|Self::Xz=>Some(Self::Xz),
        Self::TarBz2|Self::Bzip2=>Some(Self::Bzip2),
        Self::TarZst|Self::Zstd=>Some(Self::Zstd),
    }}
}
pub fn detect_linux_archive(name:&str)->Option<LinuxArchiveFormat>{
    let n=name.to_ascii_lowercase();
    for (suffix,format) in [
        (".tar.gz",LinuxArchiveFormat::TarGz),(".tgz",LinuxArchiveFormat::TarGz),
        (".tar.bz2",LinuxArchiveFormat::TarBz2),(".tbz2",LinuxArchiveFormat::TarBz2),
        (".tbz",LinuxArchiveFormat::TarBz2),
        (".tar.xz",LinuxArchiveFormat::TarXz),(".txz",LinuxArchiveFormat::TarXz),
        (".tar.zst",LinuxArchiveFormat::TarZst),(".tzst",LinuxArchiveFormat::TarZst),
        (".tar",LinuxArchiveFormat::Tar),
        (".gz",LinuxArchiveFormat::Gzip),(".bz2",LinuxArchiveFormat::Bzip2),
        (".xz",LinuxArchiveFormat::Xz),(".zst",LinuxArchiveFormat::Zstd),
    ]{
        if n.ends_with(suffix){return Some(format)}
    }
    None
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct LinuxArchiveEntry{
    pub path:String,
    pub is_directory:bool,
    pub uncompressed_bytes:u64,
    pub sha256:String,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct LinuxArchiveManifest{
    pub format:LinuxArchiveFormat,
    pub total_entries:usize,
    pub entries:Vec<LinuxArchiveEntry>,
    pub total_uncompressed_bytes:u64,
    pub input_bytes:u64,
    pub truncated:bool,
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct LinuxArchiveReceipt{
    pub target_name:String,
    pub bytes_written:u64,
    pub sha256:String,
    pub extracted_files:usize,
}
fn io(e:std::io::Error)->LinuxArchiveError{LinuxArchiveError::Io(e.to_string())}
fn flat_name(s:&str)->bool{
    !s.is_empty() && s.len()<=240 && s!="." && s!=".."
       && !s.starts_with(".lu-stage-")
       && !s.ends_with('.') && !s.ends_with(' ')
       && !s.chars().any(|c|c.is_control()||matches!(c,'/'|'\\'|':'))
}
fn member_name(s:&str)->bool{
    !s.is_empty() && s.len()<=MAX_PATH && !s.starts_with('/')
    && !s.chars().any(|c|c.is_control()||matches!(c,'\\'|':'))
    && s.split('/').all(|part|
        !part.is_empty()&&part!="."&&part!=".."&&!part.starts_with('-')
        && !part.eq_ignore_ascii_case(".git")
        && !part.ends_with('.')&&!part.ends_with(' ')
        && part.len()<=240)
}
fn sha(bytes:&[u8])->String{format!("{:x}",Sha256::digest(bytes))}
fn read_bounded(root:&Dir,name:&str)->Result<Vec<u8>,LinuxArchiveError>{
    if !flat_name(name){return Err(LinuxArchiveError::InvalidName)}
    let meta=root.symlink_metadata(name).map_err(io)?;
    if !meta.is_file()||meta.file_type().is_symlink(){return Err(LinuxArchiveError::NotFile)}
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return Err(LinuxArchiveError::NotFile)}
    }
    if meta.len()>MAX_ARCHIVE{return Err(LinuxArchiveError::TooLarge)}
    let mut file=root.open(name).map_err(io)?;
    if !file.metadata().map_err(io)?.is_file(){return Err(LinuxArchiveError::NotFile)}
    let mut bytes=Vec::new();
    file.take(MAX_ARCHIVE+1).read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() as u64>MAX_ARCHIVE{return Err(LinuxArchiveError::TooLarge)}
    Ok(bytes)
}
/// No shell/PATH resolution, working-directory-injected tools, environment,
/// user arguments or stdout-to-file redirection. Fixed system-owned binaries
/// are an explicit Linux packaging dependency and checked at runtime.
#[cfg(unix)]
fn fixed_backend(format:LinuxArchiveFormat)->Result<(&'static str,&'static [&'static str]),LinuxArchiveError>{
    let (name,args)=match format{
        LinuxArchiveFormat::Xz=>
            ("xz",&["-dc"] as &'static [&'static str]),
        LinuxArchiveFormat::Bzip2=>("bzip2",&["-dc"]),
        LinuxArchiveFormat::Zstd=>("zstd",&["-dcq"]),
        _=>return Err(LinuxArchiveError::UnsupportedFormat),
    };
    for prefix in ["/usr/bin/","/bin/"]{
        let path=match (prefix,name){
            ("/usr/bin/","xz")=>"/usr/bin/xz",("/bin/","xz")=>"/bin/xz",
            ("/usr/bin/","bzip2")=>"/usr/bin/bzip2",("/bin/","bzip2")=>"/bin/bzip2",
            ("/usr/bin/","zstd")=>"/usr/bin/zstd",("/bin/","zstd")=>"/bin/zstd",
            _=>continue,
        };
        if std::fs::symlink_metadata(path).is_ok(){
            return Ok((path,args))
        }
    }
    Err(LinuxArchiveError::BackendUnavailable)
}
#[cfg(unix)]
fn run_backend(
    format:LinuxArchiveFormat,compress:bool,input:&[u8],
)->Result<Vec<u8>,LinuxArchiveError>{
    let (program,_) = fixed_backend(format)?;
    let args:&[&str]=if compress {
        match format {
            LinuxArchiveFormat::Xz=>&["-zc","-T1"],
            LinuxArchiveFormat::Bzip2=>&["-zc"],
            LinuxArchiveFormat::Zstd=>&["-zcq","-T1"],
            _=>return Err(LinuxArchiveError::UnsupportedFormat)
        }
    }else{
        match format{
            LinuxArchiveFormat::Xz=>&["-dc"],
            LinuxArchiveFormat::Bzip2=>&["-dc"],
            LinuxArchiveFormat::Zstd=>&["-dcq"],
            _=>return Err(LinuxArchiveError::UnsupportedFormat)
        }
    };
    let mut child=Command::new(program)
        .args(args).env_clear()
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|_|LinuxArchiveError::BackendUnavailable)?;
    let mut stdin=child.stdin.take().ok_or(LinuxArchiveError::BackendFailed)?;
    let mut stdout=child.stdout.take().ok_or(LinuxArchiveError::BackendFailed)?;
    let payload=input.to_vec();
    let writer=thread::spawn(move||stdin.write_all(&payload).is_ok());
    // A watchdog owns the child through a mutex while the main thread reads.
    // On any over-limit/deadline the process is killed and reaped.
    let child=Arc::new(std::sync::Mutex::new(child));
    let (cancel_tx,cancel_rx)=mpsc::channel::<()>();
    let watchdog_child=Arc::clone(&child);
    let watchdog=thread::spawn(move||{
        if cancel_rx.recv_timeout(Duration::from_secs(30)).is_err(){
            if let Ok(mut guard)=watchdog_child.lock(){let _=guard.kill();}
        }
    });
    let limit=if compress{MAX_ARCHIVE as usize}else{MAX_UNPACKED};
    let mut output=Vec::new();
    let result=stdout.take(limit as u64+1).read_to_end(&mut output);
    if output.len()>limit || result.is_err(){
        if let Ok(mut guard)=child.lock(){let _=guard.kill();}
    }
    let _=writer.join();
    let status=child.lock().map_err(|_|LinuxArchiveError::BackendFailed)?
        .wait().map_err(io)?;
    let _=cancel_tx.send(());
    let _=watchdog.join();
    if output.len()>limit{return Err(LinuxArchiveError::TooLarge)}
    if !status.success()||result.is_err(){return Err(LinuxArchiveError::BackendFailed)}
    Ok(output)
}
#[cfg(not(unix))]
fn run_backend(_:LinuxArchiveFormat,_:bool,_:&[u8])
    ->Result<Vec<u8>,LinuxArchiveError>{
    Err(LinuxArchiveError::UnsupportedPlatform)
}
fn expand(format:LinuxArchiveFormat,data:&[u8])->Result<Vec<u8>,LinuxArchiveError>{
    match format.compressor(){
        None=>{if data.len()>MAX_UNPACKED{return Err(LinuxArchiveError::TooLarge)}
            Ok(data.to_vec())},
        Some(LinuxArchiveFormat::Gzip)=>{
            let mut decoder=MultiGzDecoder::new(data);
            let mut output=Vec::new();
            decoder.take(MAX_UNPACKED as u64+1).read_to_end(&mut output)
                .map_err(|_|LinuxArchiveError::InvalidEncoding)?;
            if output.len()>MAX_UNPACKED{return Err(LinuxArchiveError::TooLarge)}
            Ok(output)
        },
        Some(other)=>run_backend(other,false,data)
    }
}
fn compress(format:LinuxArchiveFormat,data:&[u8])->Result<Vec<u8>,LinuxArchiveError>{
    match format.compressor(){
        None=>Ok(data.to_vec()),
        Some(LinuxArchiveFormat::Gzip)=>{
            let mut enc=GzEncoder::new(Vec::new(),Compression::default());
            enc.write_all(data).map_err(io)?;
            enc.finish().map_err(io)
        },
        Some(other)=>run_backend(other,true,data)
    }
}
fn number(field:&[u8])->Result<u64,LinuxArchiveError>{
    // GNU base-256 and negative TAR numbers are intentionally refused.
    if field.is_empty() || field[0]&0x80!=0{
        return Err(LinuxArchiveError::UnsupportedEntry)
    }
    let digits=field.iter().copied().take_while(|b|*b!=0)
        .collect::<Vec<_>>();
    let trimmed=String::from_utf8(digits).map_err(|_|LinuxArchiveError::InvalidHeader)?;
    let n=trimmed.trim();
    if n.is_empty(){return Ok(0)}
    if !n.bytes().all(|b|(b'0'..=b'7').contains(&b)){
        return Err(LinuxArchiveError::InvalidHeader)
    }
    u64::from_str_radix(n,8).map_err(|_|LinuxArchiveError::InvalidHeader)
}
fn header_name(field:&[u8])->Result<String,LinuxArchiveError>{
    let n=field.iter().position(|b|*b==0).unwrap_or(field.len());
    std::str::from_utf8(&field[..n]).map(str::to_owned)
        .map_err(|_|LinuxArchiveError::InvalidEncoding)
}
fn checksum(h:&[u8])->bool{
    let expected=number(&h[148..156]);
    let actual:u64=h.iter().enumerate().map(|(i,b)|
        if (148..156).contains(&i){b' ' as u64}else{*b as u64}).sum();
    expected.is_ok_and(|n|n==actual)
}
fn block_end(size:usize)->Result<usize,LinuxArchiveError>{
    size.checked_add(SECTOR-1).map(|n|n/SECTOR*SECTOR)
        .ok_or(LinuxArchiveError::TooLarge)
}
#[derive(Clone)]
struct ParsedEntry{path:String,is_directory:bool,data_start:usize,data_end:usize}
fn pax_path(bytes:&[u8])->Result<Option<String>,LinuxArchiveError>{
    if bytes.len()>MAX_PAX{return Err(LinuxArchiveError::TooLarge)}
    let mut cursor=0;
    let mut path=None;
    while cursor<bytes.len(){
        let sep=bytes[cursor..].iter().position(|b|*b==b' ')
            .ok_or(LinuxArchiveError::InvalidHeader)?+cursor;
        let len=std::str::from_utf8(&bytes[cursor..sep])
            .map_err(|_|LinuxArchiveError::InvalidEncoding)?
            .parse::<usize>().map_err(|_|LinuxArchiveError::InvalidHeader)?;
        if len<4||cursor.checked_add(len).is_none_or(|e|e>bytes.len()){
            return Err(LinuxArchiveError::InvalidHeader)
        }
        let line=&bytes[sep+1..cursor+len];
        if line.last()!=Some(&b'\n'){return Err(LinuxArchiveError::InvalidHeader)}
        let record=std::str::from_utf8(&line[..line.len()-1])
            .map_err(|_|LinuxArchiveError::InvalidEncoding)?;
        let (key,value)=record.split_once('=').ok_or(LinuxArchiveError::InvalidHeader)?;
        match key {
            "path"=>path=Some(value.to_owned()),
            // Do not honor metadata that could alter link targets, sparse
            // extent interpretation, device nodes or published file metadata.
            "linkpath"|"GNU.sparse.map"|"GNU.sparse.name"|"GNU.sparse.realsize"|
            "GNU.sparse.size"|"GNU.sparse.major"|"GNU.sparse.minor"=>
                return Err(LinuxArchiveError::UnsupportedEntry),
            _=>{},
        }
        cursor+=len;
    }
    Ok(path)
}
fn tar_entries(data:&[u8])->Result<Vec<ParsedEntry>,LinuxArchiveError>{
    let mut offset=0;
    let mut result=Vec::new();
    let mut seen=BTreeSet::new();
    let mut pending_path:Option<String>=None;
    let mut global_pax=false;
    while offset+SECTOR<=data.len(){
        let header=&data[offset..offset+SECTOR];
        if header.iter().all(|b|*b==0){
            if !data[offset..].iter().all(|b|*b==0){
                return Err(LinuxArchiveError::InvalidHeader)
            }
            return Ok(result)
        }
        if !checksum(header){return Err(LinuxArchiveError::InvalidChecksum)}
        let size=number(&header[124..136])?;
        let n=usize::try_from(size).map_err(|_|LinuxArchiveError::TooLarge)?;
        if n>MAX_ENTRY{return Err(LinuxArchiveError::TooLarge)}
        let start=offset+SECTOR;
        let end=start.checked_add(n).ok_or(LinuxArchiveError::TooLarge)?;
        let padded=start.checked_add(block_end(n)?).ok_or(LinuxArchiveError::TooLarge)?;
        if padded>data.len(){return Err(LinuxArchiveError::InvalidHeader)}
        let kind=header[156];
        if kind==b'x' || kind==b'g' {
            let parsed=pax_path(&data[start..end])?;
            if kind==b'g' {
                // Global PAX headers are accepted only for harmless metadata,
                // not per-file path rewrite.
                if parsed.is_some(){return Err(LinuxArchiveError::UnsafeEntry)}
                global_pax=true;
            }else{pending_path=parsed}
            offset=padded;continue
        }
        if kind==b'L'{
            if n>MAX_PAX{return Err(LinuxArchiveError::TooLarge)}
            let bytes=&data[start..end];
            let n=bytes.iter().position(|b|*b==0).unwrap_or(bytes.len());
            let s=std::str::from_utf8(&bytes[..n]).map_err(|_|LinuxArchiveError::InvalidEncoding)?;
            pending_path=Some(s.trim_end_matches('\n').to_owned());
            offset=padded;continue
        }
        if !matches!(kind,0|b'0'|b'5'){
            return Err(LinuxArchiveError::UnsupportedEntry)
        }
        let mut name=header_name(&header[..100])?;
        let prefix=header_name(&header[345..500])?;
        if !prefix.is_empty(){name=format!("{prefix}/{name}")}
        if let Some(long)=pending_path.take(){name=long}
        let is_directory=kind==b'5';
        if is_directory{name=name.trim_end_matches('/').into();}
        if !member_name(&name){return Err(LinuxArchiveError::UnsafeEntry)}
        let key=name.to_lowercase();
        if !seen.insert(key){return Err(LinuxArchiveError::DuplicateEntry)}
        if is_directory && size!=0{return Err(LinuxArchiveError::UnsupportedEntry)}
        if result.len()>=MAX_ENTRIES{return Err(LinuxArchiveError::TooManyEntries)}
        result.push(ParsedEntry{path:name,is_directory,data_start:start,data_end:end});
        offset=padded;
    }
    let _=global_pax;
    Err(LinuxArchiveError::InvalidHeader)
}
fn decoded(root:&Dir,name:&str)->Result<(LinuxArchiveFormat,Vec<u8>,u64),LinuxArchiveError>{
    let format=detect_linux_archive(name).ok_or(LinuxArchiveError::UnsupportedFormat)?;
    let encoded=read_bounded(root,name)?;
    let bytes=encoded.len() as u64;
    let clear=expand(format,&encoded)?;
    Ok((format,clear,bytes))
}
pub fn inspect_linux_archive(
    root:&Dir,name:&str,max_entries:usize,
)->Result<LinuxArchiveManifest,LinuxArchiveError>{
    if max_entries==0||max_entries>256{return Err(LinuxArchiveError::TooLarge)}
    let (format,raw,input_bytes)=decoded(root,name)?;
    let mut total=0u64;
    let mut entries=Vec::new();
    if format.is_tar(){
        for e in tar_entries(&raw)?{
            let content=&raw[e.data_start..e.data_end];
            total=total.checked_add(content.len() as u64)
                .ok_or(LinuxArchiveError::TooLarge)?;
            entries.push(LinuxArchiveEntry{
                path:e.path,is_directory:e.is_directory,
                uncompressed_bytes:content.len() as u64,
                sha256:if e.is_directory{String::new()}else{sha(content)},
            });
        }
    }else{
        let ending=match format{
            LinuxArchiveFormat::Gzip=>".gz",
            LinuxArchiveFormat::Xz=>".xz",
            LinuxArchiveFormat::Bzip2=>".bz2",
            LinuxArchiveFormat::Zstd=>".zst",
            _=>return Err(LinuxArchiveError::UnsupportedFormat)
        };
        let target=&name[..name.len()-ending.len()];
        if !flat_name(target){return Err(LinuxArchiveError::InvalidName)}
        total=raw.len() as u64;
        entries.push(LinuxArchiveEntry{
            path:target.into(),is_directory:false,
            uncompressed_bytes:total,sha256:sha(&raw),
        });
    }
    let count=entries.len();
    let truncated=count>max_entries;
    entries.truncate(max_entries);
    Ok(LinuxArchiveManifest{
        format,total_entries:count,entries,total_uncompressed_bytes:total,
        input_bytes,truncated,
    })
}
/// Return bounded validated directory/file members for a NEW tree extraction.
/// A tar member is never used as a host pathname without the separate
/// destination-root checks in native_archive_tree. No symlinks or special files.
pub fn linux_archive_tree_members(
    root:&Dir,name:&str
)->Result<Vec<(String,bool,Vec<u8>)>,LinuxArchiveError>{
    let (format,raw,_)=decoded(root,name)?;
    if !format.is_tar(){
        let suffix=match format{
            LinuxArchiveFormat::Gzip=>".gz",LinuxArchiveFormat::Xz=>".xz",
            LinuxArchiveFormat::Bzip2=>".bz2",LinuxArchiveFormat::Zstd=>".zst",
            _=>return Err(LinuxArchiveError::UnsupportedFormat),
        };
        let output=&name[..name.len()-suffix.len()];
        if !flat_name(output){return Err(LinuxArchiveError::InvalidName)}
        return Ok(vec![(output.into(),false,raw)])
    }
    let entries=tar_entries(&raw)?;
    let mut total=0usize;
    let mut output=Vec::new();
    for entry in entries{
        let bytes=raw[entry.data_start..entry.data_end].to_vec();
        total=total.checked_add(bytes.len()).ok_or(LinuxArchiveError::TooLarge)?;
        if total>MAX_UNPACKED{return Err(LinuxArchiveError::TooLarge)}
        output.push((entry.path,entry.is_directory,bytes));
    }
    Ok(output)
}
/// Hash the exact selected decoded member without publishing. The returned
/// digest is an integrity hint for trusted user preview, never a permission.
pub fn linux_archive_member_digest(
    source_root:&Dir,archive_name:&str,member:&str,
)->Result<String,LinuxArchiveError>{
    let (format,raw,_)=decoded(source_root,archive_name)?;
    if format.is_tar(){
        let selected=tar_entries(&raw)?.into_iter().find(|e|e.path==member)
            .ok_or(LinuxArchiveError::NotFound)?;
        if selected.is_directory{return Err(LinuxArchiveError::UnsupportedEntry)}
        Ok(sha(&raw[selected.data_start..selected.data_end]))
    }else{
        let suffix=match format{
            LinuxArchiveFormat::Gzip=>".gz",LinuxArchiveFormat::Xz=>".xz",
            LinuxArchiveFormat::Bzip2=>".bz2",LinuxArchiveFormat::Zstd=>".zst",
            _=>return Err(LinuxArchiveError::UnsupportedFormat),
        };
        if member!=&archive_name[..archive_name.len()-suffix.len()]{
            return Err(LinuxArchiveError::NotFound)
        }
        Ok(sha(&raw))
    }
}
/// Extract a selected member; the host must pin the exact source/member hash,
/// destination and persisted human turn before dispatch via EffectJournal.
pub fn extract_linux_archive_member(
    source_root:&Dir,archive_name:&str,member:&str,
    destination:&Dir,target_name:&str,stage_id:&str,approved_digest:&str,
)->Result<LinuxArchiveReceipt,LinuxArchiveError>{
    if !flat_name(target_name) || !valid_stage(stage_id) || !digest_valid(approved_digest){
        return Err(LinuxArchiveError::InvalidName)
    }
    let (format,raw,_)=decoded(source_root,archive_name)?;
    let content=if format.is_tar(){
        let items=tar_entries(&raw)?;
        let e=items.into_iter().find(|e|e.path==member).ok_or(LinuxArchiveError::NotFound)?;
        if e.is_directory{return Err(LinuxArchiveError::UnsupportedEntry)}
        &raw[e.data_start..e.data_end]
    }else{
        let suffix=match format{
            LinuxArchiveFormat::Gzip=>".gz",LinuxArchiveFormat::Xz=>".xz",
            LinuxArchiveFormat::Bzip2=>".bz2",LinuxArchiveFormat::Zstd=>".zst",
            _=>return Err(LinuxArchiveError::UnsupportedFormat),
        };
        let expected=&archive_name[..archive_name.len()-suffix.len()];
        if member!=expected{return Err(LinuxArchiveError::NotFound)}
        &raw
    };
    if sha(content)!=approved_digest{return Err(LinuxArchiveError::SourceChanged)}
    publish_new(destination,target_name,stage_id,content,approved_digest,1)
}
pub fn extract_all_linux_archive_flat(
    source_root:&Dir,archive_name:&str,destination:&Dir,
    approved_manifest:&LinuxArchiveManifest,stage_nonce:&str,
)->Result<Vec<LinuxArchiveReceipt>,LinuxArchiveError>{
    // This conservative first bulk adapter requires flat regular files.
    // It refuses folders rather than silently creating partial directory trees.
    // Publication of multiple members is NOT atomic as one logical action,
    // therefore the host must journal each member separately and reconcile.
    if !valid_stage(stage_nonce){return Err(LinuxArchiveError::InvalidName)}
    let current=inspect_linux_archive(source_root,archive_name,256)?;
    if current.truncated || current!=*approved_manifest{return Err(LinuxArchiveError::SourceChanged)}
    if current.entries.iter().any(|e|e.is_directory||!flat_name(&e.path)){
        return Err(LinuxArchiveError::UnsupportedEntry)
    }
    let mut result=Vec::new();
    for (index,e) in current.entries.iter().enumerate(){
        let stage=format!("{stage_nonce}-{index}");
        result.push(extract_linux_archive_member(
            source_root,archive_name,&e.path,destination,&e.path,&stage,&e.sha256
        )?);
    }
    Ok(result)
}
fn valid_stage(s:&str)->bool{
    (16..=120).contains(&s.len()) &&
        s.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'||b==b'-')
}
fn digest_valid(s:&str)->bool{
    s.len()==64&&s.bytes().all(|b|b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
fn clean_stage(root:&Dir,stage:&str,error:LinuxArchiveError)->LinuxArchiveError{
    if root.remove_file(stage).is_err(){
        LinuxArchiveError::PublishedNeedsReconciliation
    }else{error}
}
fn publish_new(
    root:&Dir,name:&str,stage_id:&str,data:&[u8],expected:&str,files:usize,
)->Result<LinuxArchiveReceipt,LinuxArchiveError>{
    if !flat_name(name)||!valid_stage(stage_id)||!digest_valid(expected)||
        data.len()>MAX_UNPACKED{
        return Err(LinuxArchiveError::InvalidName)
    }
    if sha(data)!=expected{return Err(LinuxArchiveError::SourceChanged)}
    let stage=format!(".lu-stage-{stage_id}.tar.tmp");
    let mut options=OpenOptions::new();
    options.create_new(true).write(true);
    let mut handle=root.open_with(&stage,&options).map_err(|e|
        if e.kind()==std::io::ErrorKind::AlreadyExists{
            LinuxArchiveError::StageCollision
        }else{io(e)}
    )?;
    let write=handle.write_all(data).and_then(|_|handle.sync_all());
    drop(handle);
    if let Err(e)=write{return Err(clean_stage(root,&stage,io(e)))}
    if let Err(e)=root.hard_link(&stage,root,name){
        return Err(clean_stage(root,&stage,if e.kind()==std::io::ErrorKind::AlreadyExists{
            LinuxArchiveError::TargetExists
        }else{io(e)}))
    }
    root.remove_file(&stage).map_err(|_|LinuxArchiveError::PublishedNeedsReconciliation)?;
    Ok(LinuxArchiveReceipt{
        target_name:name.into(),bytes_written:data.len() as u64,
        sha256:expected.into(),extracted_files:files,
    })
}
fn octal(field:&mut[u8],n:u64)->Result<(),LinuxArchiveError>{
    let digits=format!("{:o}",n);
    if digits.len()+1>field.len(){return Err(LinuxArchiveError::TooLarge)}
    field.fill(b'0');
    let start=field.len()-1-digits.len();
    field[start..start+digits.len()].copy_from_slice(digits.as_bytes());
    field[field.len()-1]=0;
    Ok(())
}
fn append_header(
    out:&mut Vec<u8>,name:&str,length:usize,folder:bool,
)->Result<(),LinuxArchiveError>{
    let mut header=[0u8;SECTOR];
    if name.len()>100{return Err(LinuxArchiveError::TooLarge)}
    header[..name.len()].copy_from_slice(name.as_bytes());
    octal(&mut header[100..108],if folder{0o755}else{0o644})?;
    octal(&mut header[108..116],0)?;
    octal(&mut header[116..124],0)?;
    octal(&mut header[124..136],length as u64)?;
    octal(&mut header[136..148],0)?;
    header[148..156].fill(b' ');
    header[156]=if folder{b'5'}else{b'0'};
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let checksum:u64=header.iter().map(|b|*b as u64).sum();
    octal(&mut header[148..156],checksum)?;
    out.extend_from_slice(&header);
    Ok(())
}
/// One archive assembled from host-selected existing flat files. The result
/// is a *new* target, never an overwrite. All inputs are checked and bounded
/// before staging, no filesystem metadata is imported into archive headers.
fn render_linux_archive(
    root:&Dir,target_name:&str,inputs:&[String]
)->Result<Vec<u8>,LinuxArchiveError>{
    if !flat_name(target_name){return Err(LinuxArchiveError::InvalidName)}
    let format=detect_linux_archive(target_name)
        .ok_or(LinuxArchiveError::UnsupportedFormat)?;
    if inputs.is_empty()||inputs.len()>MAX_SOURCE_FILES{
        return Err(LinuxArchiveError::TooManyEntries)
    }
    if !format.is_tar() && inputs.len()!=1{
        return Err(LinuxArchiveError::UnsupportedFormat)
    }
    let mut seen=BTreeSet::new();
    let mut files=BTreeMap::new();
    let mut total=0usize;
    for name in inputs{
        if !flat_name(name)||!seen.insert(name.to_lowercase())||
            name.eq_ignore_ascii_case(target_name){
            return Err(LinuxArchiveError::InvalidName)
        }
        let bytes=read_bounded(root,name)?;
        if bytes.len()>MAX_ENTRY{return Err(LinuxArchiveError::TooLarge)}
        total=total.checked_add(bytes.len()).ok_or(LinuxArchiveError::TooLarge)?;
        if total>MAX_UNPACKED{return Err(LinuxArchiveError::TooLarge)}
        files.insert(name.as_str(),bytes);
    }
    let mut raw=Vec::new();
    if format.is_tar(){
        for (name,bytes) in files{
            append_header(&mut raw,name,bytes.len(),false)?;
            raw.extend_from_slice(&bytes);
            raw.resize(raw.len()+((SECTOR-bytes.len()%SECTOR)%SECTOR),0);
            if raw.len()>MAX_UNPACKED{return Err(LinuxArchiveError::TooLarge)}
        }
        raw.resize(raw.len()+2*SECTOR,0);
    }else{
        raw=files.into_values().next().ok_or(LinuxArchiveError::NotFound)?;
    }
    let encoded=compress(format,&raw)?;
    if encoded.len()>MAX_ARCHIVE as usize{return Err(LinuxArchiveError::TooLarge)}
    Ok(encoded)
}
/// Compute the exact proposed archive content receipt. This verifies bytes,
/// but is NOT authority: the trusted host separately binds user/target/grant.
pub fn preview_linux_archive(
    root:&Dir,target_name:&str,inputs:&[String],
)->Result<(u64,String),LinuxArchiveError>{
    let encoded=render_linux_archive(root,target_name,inputs)?;
    Ok((encoded.len() as u64,sha(&encoded)))
}
pub fn create_linux_archive_new_checked(
    root:&Dir,target_name:&str,inputs:&[String],stage_id:&str,approved_digest:&str,
)->Result<LinuxArchiveReceipt,LinuxArchiveError>{
    if !valid_stage(stage_id)||!digest_valid(approved_digest){
        return Err(LinuxArchiveError::InvalidName)
    }
    let encoded=render_linux_archive(root,target_name,inputs)?;
    let digest=sha(&encoded);
    if digest!=approved_digest{return Err(LinuxArchiveError::SourceChanged)}
    publish_new(root,target_name,stage_id,&encoded,approved_digest,inputs.len())
}
pub fn create_linux_archive_new(
    root:&Dir,target_name:&str,inputs:&[String],stage_id:&str
)->Result<LinuxArchiveReceipt,LinuxArchiveError>{
    let (_bytes,digest)=preview_linux_archive(root,target_name,inputs)?;
    create_linux_archive_new_checked(root,target_name,inputs,stage_id,&digest)
}
