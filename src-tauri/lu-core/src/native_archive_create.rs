//! TOOLS-04: host-selected, create-new ZIP archive primitive.
//! This is not an advertised model tool and does NOT authorize writes.
//! Its caller must hold a bound user-turn grant, use a durable effect journal,
//! and own a trusted directory capability and unique staging identifier.
//! Only flat regular files in the same approved directory are admitted.
use cap_std::fs::{Dir, OpenOptions};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::{Read, Write}, path::Path};
use zip::{ZipWriter, write::SimpleFileOptions, CompressionMethod};

const MAX_INPUT_FILES: usize = 64;
const MAX_INPUT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_SINGLE_FILE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveCreateError {
    InvalidName,
    InvalidStage,
    EmptyInput,
    TooManyEntries,
    DuplicateEntry,
    ArchiveAsInput,
    SourceNotRegular,
    SourceUnavailable,
    TooLarge,
    TargetExists,
    StageExists,
    Io(String),
    PublishedNeedsReconciliation,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveCreateReceipt {
    pub archive_name: String,
    pub entries: usize,
    pub archive_bytes: u64,
    pub sha256: String,
}
fn filename(s: &str) -> bool {
    !s.is_empty() && s.len() <= 240 && s != "." && s != ".."
        && !s.starts_with(".lu-stage-") && !s.ends_with('.') && !s.ends_with(' ')
        && !s.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
        && !Path::new(s).is_absolute()
}
fn stage_id(s: &str) -> bool {
    (16..=128).contains(&s.len()) &&
    s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn hash_file(mut file: cap_std::fs::File) -> Result<(u64, String), ArchiveCreateError> {
    let mut hasher=Sha256::new();
    let mut n=0_u64;
    let mut buf=[0_u8; 64 * 1024];
    loop {
        let count=file.read(&mut buf).map_err(|e| ArchiveCreateError::Io(e.to_string()))?;
        if count==0 {break}
        n+=count as u64;
        if n>MAX_INPUT_BYTES+128*1024 {return Err(ArchiveCreateError::TooLarge)}
        hasher.update(&buf[..count]);
    }
    Ok((n,format!("{:x}",hasher.finalize())))
}
/// Publish a new (never overwritten) ZIP containing exact existing files.
/// Inputs have flat names only. No recursive folders, links or user-supplied
/// extraction paths are supported. Caller owns source/destination grant.
pub fn create_zip_new_in_dir(
    root:&Dir, archive_name:&str, inputs:&[String], host_stage_id:&str
) -> Result<ArchiveCreateReceipt,ArchiveCreateError> {
    if !filename(archive_name) || !archive_name.to_ascii_lowercase().ends_with(".zip") {
        return Err(ArchiveCreateError::InvalidName);
    }
    if !stage_id(host_stage_id){return Err(ArchiveCreateError::InvalidStage)}
    if inputs.is_empty(){return Err(ArchiveCreateError::EmptyInput)}
    if inputs.len()>MAX_INPUT_FILES{return Err(ArchiveCreateError::TooManyEntries)}
    let mut seen=BTreeSet::new();
    let mut total=0_u64;
    for name in inputs {
        if !filename(name){return Err(ArchiveCreateError::InvalidName)}
        if archive_name.eq_ignore_ascii_case(name){return Err(ArchiveCreateError::ArchiveAsInput)}
        if !seen.insert(name.to_lowercase()){return Err(ArchiveCreateError::DuplicateEntry)}
        let meta=root.symlink_metadata(name).map_err(|_| ArchiveCreateError::SourceUnavailable)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(ArchiveCreateError::SourceNotRegular);
        }
        if meta.len()>MAX_SINGLE_FILE_BYTES{return Err(ArchiveCreateError::TooLarge)}
        total=total.checked_add(meta.len()).ok_or(ArchiveCreateError::TooLarge)?;
        if total>MAX_INPUT_BYTES{return Err(ArchiveCreateError::TooLarge)}
    }
    match root.symlink_metadata(archive_name) {
        Ok(_)=>return Err(ArchiveCreateError::TargetExists),
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},
        Err(e)=>return Err(ArchiveCreateError::Io(e.to_string())),
    }
    let stage=format!(".lu-stage-{host_stage_id}.zip");
    let mut options=OpenOptions::new();
    options.create_new(true).read(true).write(true);
    let stage_file=root.open_with(&stage,&options).map_err(|e| {
        if e.kind()==std::io::ErrorKind::AlreadyExists {ArchiveCreateError::StageExists}
        else {ArchiveCreateError::Io(e.to_string())}
    })?;
    let written=(|| -> Result<(),ArchiveCreateError> {
        let mut writer=ZipWriter::new(stage_file);
        let opts=SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let mut buf=[0_u8;64*1024];
        let mut actual_total=0_u64;
        for name in inputs {
            let meta=root.symlink_metadata(name).map_err(|_| ArchiveCreateError::SourceUnavailable)?;
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(ArchiveCreateError::SourceNotRegular);
            }
            let mut source=root.open(name).map_err(|_| ArchiveCreateError::SourceUnavailable)?;
            if !source.metadata().map_err(|e| ArchiveCreateError::Io(e.to_string()))?.is_file() {
                return Err(ArchiveCreateError::SourceNotRegular);
            }
            writer.start_file(name.as_str(),opts)
                .map_err(|e| ArchiveCreateError::Io(e.to_string()))?;
            let mut count=0_u64;
            loop {
                let n=source.read(&mut buf).map_err(|e| ArchiveCreateError::Io(e.to_string()))?;
                if n==0{break}
                count+=n as u64;
                actual_total+=n as u64;
                if count>MAX_SINGLE_FILE_BYTES || actual_total>MAX_INPUT_BYTES {
                    return Err(ArchiveCreateError::TooLarge)
                }
                writer.write_all(&buf[..n]).map_err(|e| ArchiveCreateError::Io(e.to_string()))?;
            }
        }
        let result=writer.finish().map_err(|e| ArchiveCreateError::Io(e.to_string()))?;
        result.sync_all().map_err(|e| ArchiveCreateError::Io(e.to_string()))?;
        Ok(())
    })();
    if let Err(e)=written {
        if root.remove_file(&stage).is_err() {
            return Err(ArchiveCreateError::PublishedNeedsReconciliation);
        }
        return Err(e)
    }
    let stage_file=root.open(&stage).map_err(|e| ArchiveCreateError::Io(e.to_string()))?;
    let (archive_bytes,sha256)=match hash_file(stage_file) {
        Ok(v)=>v,
        Err(e)=>{
            if root.remove_file(&stage).is_err() {
                return Err(ArchiveCreateError::PublishedNeedsReconciliation);
            }
            return Err(e)
        }
    };
    if let Err(e)=root.hard_link(&stage,root,archive_name) {
        if root.remove_file(&stage).is_err() {
            return Err(ArchiveCreateError::PublishedNeedsReconciliation);
        }
        return Err(if e.kind()==std::io::ErrorKind::AlreadyExists {
            ArchiveCreateError::TargetExists
        } else {ArchiveCreateError::Io(e.to_string())})
    }
    root.remove_file(&stage).map_err(|_| ArchiveCreateError::PublishedNeedsReconciliation)?;
    Ok(ArchiveCreateReceipt {
        archive_name:archive_name.into(),entries:inputs.len(),archive_bytes,sha256,
    })
}
