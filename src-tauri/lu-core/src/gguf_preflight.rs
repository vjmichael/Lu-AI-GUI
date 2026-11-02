//! Lu-owned, bounded GGUF header preflight. This does not load weights or
//! claim inference readiness; backend admission requires a separate engine test.
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

pub const GGUF_HEADER_BYTES: usize = 24;
pub const MAX_MODEL_BYTES: u64 = 1 << 44; // 16 TiB sanity ceiling, not a memory budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GgufHeader {
    pub version: u32,
    pub tensor_count: u64,
    pub metadata_count: u64,
    pub file_bytes: u64,
}
#[derive(Debug)]
pub enum GgufPreflightError {
    Io(io::Error),
    InvalidMagic,
    UnsupportedVersion(u32),
    InvalidLength(u64),
    ImplausibleCounts,
}
impl From<io::Error> for GgufPreflightError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Read only 24 bytes. Never parse arbitrary metadata, allocate tensors,
/// follow model-provided paths, or invoke the inference engine at this stage.
pub fn inspect_gguf_header(path: &Path) -> Result<GgufHeader, GgufPreflightError> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if !(GGUF_HEADER_BYTES as u64..=MAX_MODEL_BYTES).contains(&len) {
        return Err(GgufPreflightError::InvalidLength(len));
    }
    let mut header = [0u8; GGUF_HEADER_BYTES];
    file.read_exact(&mut header)?;
    if &header[0..4] != b"GGUF" {
        return Err(GgufPreflightError::InvalidMagic);
    }
    let version = u32::from_le_bytes(header[4..8].try_into().unwrap());
    if version != 2 && version != 3 {
        return Err(GgufPreflightError::UnsupportedVersion(version));
    }
    let tensor_count = u64::from_le_bytes(header[8..16].try_into().unwrap());
    let metadata_count = u64::from_le_bytes(header[16..24].try_into().unwrap());
    // A valid header can still reference invalid tensor offsets or metadata.
    // Bound counts conservatively before passing to any further parser.
    if tensor_count == 0 || tensor_count > 100_000_000 || metadata_count > 100_000_000 {
        return Err(GgufPreflightError::ImplausibleCounts);
    }
    Ok(GgufHeader {
        version,
        tensor_count,
        metadata_count,
        file_bytes: len,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn check(bytes: &[u8]) -> Result<GgufHeader, GgufPreflightError> {
        let path = std::env::temp_dir().join(format!(
            "lu-gguf-header-{}-{:?}.gguf",
            std::process::id(),
            std::thread::current().id()
        ));
        let mut f = File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        drop(f);
        let result = inspect_gguf_header(&path);
        std::fs::remove_file(path).unwrap();
        result
    }
    fn header(v: u32, n: u64, m: u64) -> Vec<u8> {
        let mut b = b"GGUF".to_vec();
        b.extend(v.to_le_bytes());
        b.extend(n.to_le_bytes());
        b.extend(m.to_le_bytes());
        b
    }
    #[test]
    fn valid_v3_header() {
        let h = check(&header(3, 2, 7)).unwrap();
        assert_eq!((h.version, h.tensor_count, h.metadata_count), (3, 2, 7));
    }
    #[test]
    fn rejects_short_file() {
        assert!(matches!(
            check(b"GGUF"),
            Err(GgufPreflightError::InvalidLength(4))
        ));
    }
    #[test]
    fn rejects_wrong_magic() {
        let mut b = header(3, 1, 1);
        b[0] = b'X';
        assert!(matches!(check(&b), Err(GgufPreflightError::InvalidMagic)));
    }
    #[test]
    fn rejects_unsupported_version() {
        assert!(matches!(
            check(&header(5, 1, 1)),
            Err(GgufPreflightError::UnsupportedVersion(5))
        ));
    }
    #[test]
    fn rejects_unreasonable_counts() {
        assert!(matches!(
            check(&header(3, 0, 1)),
            Err(GgufPreflightError::ImplausibleCounts)
        ));
    }
}
