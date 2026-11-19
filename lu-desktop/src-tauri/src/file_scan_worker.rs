//! Read-only, bounded-memory background file integrity scanner.
//! This deliberately does NOT interpret video frames or audio samples. It
//! proves native task progress and cancellation against actual media-sized
//! files without keeping inference active.
use std::{ffi::OsStr,fs::File,io::{Read,Write},path::Path};
use sha2::{Digest,Sha256};

/// Main executable enters this mode before any Tauri or model initialization.
/// Host-selected file path is the only argument; there is no command shell,
/// network access, filesystem write, or user-provided executable.
pub fn run_file_scan_worker()->i32{
    let mut args=std::env::args_os().skip(1);
    if args.next().as_deref()!=Some(OsStr::new("--lu-file-scan-worker")){
        return 2;
    }
    let Some(path)=args.next() else{return 2;};
    let identity=args.next();
    let bytes=args.next();
    if args.next().is_some(){return 2;}
    let mut output=std::io::stdout().lock();
    let result=match (identity,bytes){
        (None,None)=>scan_file(Path::new(&path),&mut output),
        #[cfg(windows)]
        (Some(identity),Some(bytes))=>{
            let Some(identity)=identity.to_str() else{return 2};
            let Some(bytes)=bytes.to_str().and_then(|s|s.parse::<u64>().ok()) else{return 2};
            scan_reviewed_file(Path::new(&path),identity,bytes,&mut output)
        },
        _=>return 2,
    };
    match result{Ok(())=>0,Err(_)=>3}
}

/// Bounded 1 MiB reads. Data and hash are never sent through the progress
/// stream; only the total byte count, current position and final SHA-256.
pub fn scan_file(path:&Path,output:&mut impl Write)->std::io::Result<()>{
    scan_open_file(File::open(path)?,output)
}
/// The original scheduler holds an opened share-read-only lease until
/// terminal reconciliation. Verify the opened child handle, not the path,
/// before reading any byte or emitting any progress/digest.
#[cfg(windows)]
fn scan_reviewed_file(path:&Path,identity:&str,bytes:u64,output:&mut impl Write)->std::io::Result<()>{
    use std::os::windows::fs::OpenOptionsExt;
    if identity.len()!=48||!identity.bytes().all(|b|b.is_ascii_hexdigit())
        ||bytes==0||bytes>64*1024*1024*1024{
        return Err(std::io::Error::other("invalid reviewed input"))
    }
    let file=std::fs::OpenOptions::new().read(true).share_mode(1)
        .custom_flags(0x0020_0000).open(path)?;
    if lu_core::windows_file_guard::inspect_open_file(&file)?.token()!=identity
        ||file.metadata()?.len()!=bytes{
        return Err(std::io::Error::other("reviewed input changed"))
    }
    scan_open_file(file,output)
}
fn scan_open_file(mut file:File,output:&mut impl Write)->std::io::Result<()>{
    let metadata=file.metadata()?;
    if !metadata.is_file()||metadata.len()==0{
        return Err(std::io::Error::other("not a non-empty regular file"));
    }
    let total=metadata.len();
    let mut hasher=Sha256::new();
    let mut read=0u64;
    let mut last_percent=0u64;
    let mut buffer=vec![0u8;1024*1024];
    writeln!(output,"P\t0\t{total}")?;
    output.flush()?;
    loop{
        let n=file.read(&mut buffer)?;
        if n==0{break;}
        hasher.update(&buffer[..n]);
        read=read.saturating_add(n as u64);
        if read>total{
            return Err(std::io::Error::other("file grew during scan"));
        }
        let percent=(u128::from(read)*100/u128::from(total)) as u64;
        if percent>last_percent{
            // Reserve 100% for the host-confirmed final digest.
            writeln!(output,"P\t{read}\t{total}")?;
            output.flush()?;
            last_percent=percent;
        }
    }
    if read!=total{
        return Err(std::io::Error::other("file changed during scan"));
    }
    writeln!(output,"D\t{:x}",hasher.finalize())?;
    output.flush()
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn actual_file_scan_emits_bounded_progress_and_valid_digest(){
        let file=std::env::temp_dir().join(format!("lu-file-scan-test-{}",std::process::id()));
        std::fs::write(&file,b"real media bytes from disk").unwrap();
        let mut output=Vec::new();
        scan_file(&file,&mut output).unwrap();
        std::fs::remove_file(&file).unwrap();
        let transcript=String::from_utf8(output).unwrap();
        assert!(transcript.starts_with("P\t0\t26\n")||transcript.starts_with("P\t0\t"));
        assert!(transcript.contains("\nD\t"));
        let expected=format!("{:x}",Sha256::digest(b"real media bytes from disk"));
        assert!(transcript.ends_with(&format!("D\t{expected}\n")));
    }
}
#[cfg(all(test,windows))]
#[test]
fn reviewed_scan_rejects_substituted_id_before_progress(){
    let root=std::env::temp_dir().join(format!("lu-reviewed-scan-worker-{}",std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let a=root.join("a.bin");let b=root.join("b.bin");
    std::fs::write(&a,b"original").unwrap();std::fs::write(&b,b"replaced").unwrap();
    let id=lu_core::windows_file_guard::inspect_open_file(&File::open(&a).unwrap()).unwrap().token();
    let mut out=Vec::new();
    assert!(scan_reviewed_file(&b,&id,8,&mut out).is_err());assert!(out.is_empty());
    assert!(scan_reviewed_file(&a,&id,9,&mut out).is_err());assert!(out.is_empty());
    scan_reviewed_file(&a,&id,8,&mut out).unwrap();
    assert!(String::from_utf8(out).unwrap().contains(&format!("D\t{:x}",Sha256::digest(b"original"))));
    std::fs::remove_dir_all(root).unwrap();
}
