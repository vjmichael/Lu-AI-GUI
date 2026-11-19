#[cfg(target_os="linux")]
pub fn linux_scan_identity(file:&std::fs::File)->std::io::Result<String>{
    use std::os::unix::fs::MetadataExt;
    use sha2::{Digest,Sha256};
    let m=file.metadata()?;
    if !m.is_file()||m.len()==0||m.len()>4*1024*1024*1024||m.nlink()!=1{
        return Err(std::io::Error::other("unsupported reviewed scan input"))
    }
    let mut h=Sha256::new();
    h.update(m.dev().to_le_bytes());h.update(m.ino().to_le_bytes());
    h.update(m.len().to_le_bytes());h.update(m.ctime().to_le_bytes());
    h.update(m.ctime_nsec().to_le_bytes());h.update(m.mtime().to_le_bytes());
    h.update(m.mtime_nsec().to_le_bytes());
    Ok(format!("{:x}",h.finalize()))
}
