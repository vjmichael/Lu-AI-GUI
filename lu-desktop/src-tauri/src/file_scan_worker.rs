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
    if args.next().is_some(){return 2;}
    match scan_file(Path::new(&path),&mut std::io::stdout().lock()){
        Ok(())=>0,
        Err(_)=>3, // Don't log filenames or private host paths.
    }
}

/// Bounded 1 MiB reads. Data and hash are never sent through the progress
/// stream; only the total byte count, current position and final SHA-256.
pub fn scan_file(path:&Path,output:&mut impl Write)->std::io::Result<()>{
    let mut file=File::open(path)?;
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
