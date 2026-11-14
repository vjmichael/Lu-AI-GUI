//! DEV-04.3: format and debug-information structural witness for the
//! independently rehashed, OS-contained, completed Cargo build output.
//! Read-only and fail-closed. No target execution, DAP launch or OS grant.
#[cfg(unix)]
use crate::debug_binary_witness::verify_enclosed_private_hardlinks;
use crate::{
    debug_binary_witness::DebugBinaryWitness,
    wo6_output_evidence::{ArtifactEntry, OutputWitness},
    SupervisorError,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Read,
    path::{Component, Path},
};

const MAX_FILE: u64 = 64 * 1024 * 1024;
const ROLE: &str = "structural_binary_and_debug_information_only_no_debugger_authority";
const MANIFEST_ROLE: &str = "advisory_post_termination_output_metadata_not_os_acceptance";
fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSymbolWitness {
    pub binary_relative_path: String,
    pub binary_sha256: String,
    pub output_tree_sha256: String,
    pub executable_format: &'static str,
    pub symbols_kind: &'static str,
    pub symbols_relative_path: Option<String>,
    pub symbols_sha256: String,
    pub validation_scope: &'static str,
    pub role: &'static str,
    pub debug_symbols_attested: bool,
    pub source_line_mapping_verified: bool,
    pub debugger_execution_authorized: bool,
    pub installed_app_accepted: bool,
}

fn take(data: &[u8], start: usize, len: usize) -> Option<&[u8]> {
    data.get(start..start.checked_add(len)?)
}
fn u16le(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(take(data, at, 2)?.try_into().ok()?))
}
fn u32le(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(take(data, at, 4)?.try_into().ok()?))
}
fn u64le(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(take(data, at, 8)?.try_into().ok()?))
}
fn checked(data: &[u8], at: u64, size: u64) -> Option<&[u8]> {
    let pos = usize::try_from(at).ok()?;
    let len = usize::try_from(size).ok()?;
    take(data, pos, len)
}
fn is_reparse(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}
fn regular(meta: &fs::Metadata) -> bool {
    meta.is_file() && !meta.file_type().is_symlink() && !is_reparse(meta)
}
fn checked_dir(path: &Path) -> Result<(), SupervisorError> {
    let meta = fs::symlink_metadata(path).map_err(|_| denied())?;
    if !meta.is_dir() || meta.file_type().is_symlink() || is_reparse(&meta) {
        return Err(denied());
    }
    Ok(())
}
/// Read from the fixed native-owned published output, never a caller path.
/// Check *each* parent component, reject symlinks/reparse and Unix hard links,
/// use no-follow flags, compare opened inode/size and independent manifest SHA.
pub(crate) fn read_attested(
    root: &Path,
    entry: &ArtifactEntry,
    witness: &OutputWitness,
) -> Result<Vec<u8>, SupervisorError> {
    if !root.is_absolute() || entry.bytes < 4 || entry.bytes > MAX_FILE {
        return Err(denied());
    }
    checked_dir(root)?;
    let rel = Path::new(&entry.relative_path);
    if !rel
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(denied());
    }
    let mut parent = root.to_path_buf();
    if let Some(parents) = rel.parent() {
        for component in parents.components() {
            let Component::Normal(part) = component else {
                return Err(denied());
            };
            parent.push(part);
            checked_dir(&parent)?;
        }
    }
    let path = root.join(rel);
    let before = fs::symlink_metadata(&path).map_err(|_| denied())?;
    if !regular(&before) || before.len() != entry.bytes {
        return Err(denied());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.nlink() == 0 {
            return Err(denied());
        }
    }
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(0x20000); // Linux O_NOFOLLOW
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let mut opened = opts.open(&path).map_err(|_| denied())?;
    let open_meta = opened.metadata().map_err(|_| denied())?;
    if !regular(&open_meta) || open_meta.len() != entry.bytes {
        return Err(denied());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if open_meta.dev() != before.dev()
            || open_meta.ino() != before.ino()
            || open_meta.nlink() != before.nlink()
        {
            return Err(denied());
        }
        verify_enclosed_private_hardlinks(root, witness, entry, &open_meta)?;
    }
    let mut data = Vec::with_capacity(entry.bytes as usize);
    opened
        .take(MAX_FILE + 1)
        .read_to_end(&mut data)
        .map_err(|_| denied())?;
    if data.len() as u64 != entry.bytes {
        return Err(denied());
    }
    let after = fs::symlink_metadata(&path).map_err(|_| denied())?;
    if !regular(&after)
        || after.len() != open_meta.len()
        || after.modified().ok() != open_meta.modified().ok()
    {
        return Err(denied());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if after.dev() != open_meta.dev() || after.ino() != open_meta.ino() {
            return Err(denied());
        }
    }
    if format!("{:x}", Sha256::digest(&data)) != entry.sha256 {
        return Err(denied());
    }
    Ok(data)
}

#[cfg(target_os = "linux")]
fn elf64_dwarf_sections(data: &[u8]) -> Option<String> {
    // Only current native x86-64 Linux debug builds, little-endian ELF64,
    // ET_EXEC or PIE ET_DYN; foreign architectures and compressed sections
    // are not accepted as a project debug target.
    if take(data, 0, 7)? != b"\x7fELF\x02\x01\x01"
        || u16le(data, 16).is_none_or(|kind| kind != 2 && kind != 3)
        || u16le(data, 18)? != 62
        || u32le(data, 20)? != 1
        || u16le(data, 52)? != 64
        || u16le(data, 54)? != 56
        || u16le(data, 58)? != 64
    {
        return None;
    }
    let phoff = u64le(data, 32)?;
    let phnum = usize::from(u16le(data, 56)?);
    if phnum == 0 || phnum > 256 {
        return None;
    }
    let mut executable_load = false;
    for idx in 0..phnum {
        let header = checked(data, phoff.checked_add((idx as u64) * 56)?, 56)?;
        if u32le(header, 0)? == 1 && u32le(header, 4)? & 1 != 0 {
            executable_load = true;
        }
    }
    if !executable_load || u64le(data, 24)? == 0 {
        return None;
    }
    let shoff = u64le(data, 40)?;
    let num = usize::from(u16le(data, 60)?);
    let names = usize::from(u16le(data, 62)?);
    if num < 4 || num > 2048 || names == 0 || names >= num {
        return None;
    }
    let section =
        |idx: usize| -> Option<&[u8]> { checked(data, shoff.checked_add((idx as u64) * 64)?, 64) };
    let strtab = section(names)?;
    if u32le(strtab, 4)? != 3 {
        return None;
    } // SHT_STRTAB
    let strings = checked(data, u64le(strtab, 24)?, u64le(strtab, 32)?)?;
    let mut debug_info = None;
    let mut abbrev = None;
    let mut line = None;
    for idx in 1..num {
        let header = section(idx)?;
        let start = usize::try_from(u32le(header, 0)?).ok()?;
        let name_tail = strings.get(start..)?;
        let name_end = name_tail.iter().position(|c| *c == 0)?;
        let name = take(name_tail, 0, name_end)?;
        if ![b".debug_info".as_slice(), b".debug_abbrev", b".debug_line"].contains(&name) {
            continue;
        }
        if u32le(header, 4)? != 1 || u64le(header, 8)? & 0x800 != 0 {
            return None; // no NOBITS, compressed or external split DWARF
        }
        let body = checked(data, u64le(header, 24)?, u64le(header, 32)?)?;
        if body.len() < 12 {
            return None;
        }
        if name == b".debug_info" {
            if debug_info.replace(body).is_some() {
                return None;
            }
        } else if name == b".debug_abbrev" {
            if abbrev.replace(body).is_some() {
                return None;
            }
        } else if line.replace(body).is_some() {
            return None;
        }
    }
    let info = debug_info?;
    let ab = abbrev?;
    let ln = line?;
    if !unit(info, true, ab.len()) || !unit(ln, false, ab.len()) {
        return None;
    }
    let mut hash = Sha256::new();
    for (tag, body) in [(1u8, info), (2, ab), (3, ln)] {
        hash.update([tag]);
        hash.update((body.len() as u64).to_le_bytes());
        hash.update(body);
    }
    Some(format!("{:x}", hash.finalize()))
}
#[cfg(target_os = "linux")]
fn unit(bytes: &[u8], compile: bool, abbrev_size: usize) -> bool {
    let Some(initial) = u32le(bytes, 0) else {
        return false;
    };
    let (header, total, offset_size) = if initial == 0xffff_ffff {
        let Some(len) = u64le(bytes, 4) else {
            return false;
        };
        let Some(total) = len.checked_add(12) else {
            return false;
        };
        (12usize, total, 8usize)
    } else if initial > 0 && initial < 0xffff_fff0 {
        (4usize, initial as u64 + 4, 4usize)
    } else {
        return false;
    };
    if total > bytes.len() as u64 || total < (header + 4) as u64 {
        return false;
    }
    let Some(version) = u16le(bytes, header) else {
        return false;
    };
    if !(2..=5).contains(&version) {
        return false;
    }
    if compile {
        let abbrev_at = if version == 5 {
            let Some(kind) = bytes.get(header + 2) else {
                return false;
            };
            let Some(address) = bytes.get(header + 3) else {
                return false;
            };
            if !(1..=6).contains(kind) || ![4, 8].contains(address) {
                return false;
            }
            header + 4
        } else {
            let Some(address) = bytes.get(header + 2 + offset_size) else {
                return false;
            };
            if ![4, 8].contains(address) {
                return false;
            }
            header + 2
        };
        let aboff = if offset_size == 8 {
            u64le(bytes, abbrev_at)
        } else {
            u32le(bytes, abbrev_at).map(u64::from)
        };
        aboff.is_some_and(|n| n < abbrev_size as u64)
    } else {
        true
    }
}

/// Strict re-opened immutable PE/PDB pair identity. This validates the
/// compiler RSDS ↔ PDB MSF7 GUID/age/DBI bytes, not a source-line mapping,
/// debugger stop event, or execution authorization. Caller must separately
/// verify OS Job/ACL/source-line and the effective WFP socket matrix.
#[cfg(windows)]
pub(crate) fn verify_exact_sealed_windows_pe_pdb(
    pe:&Path,pdb:&Path,pe_sha:&str,pdb_sha:&str,
)->Result<(),SupervisorError>{
    use std::os::windows::fs::MetadataExt;
    if !pe.is_absolute()||!pdb.is_absolute()||pe==pdb{return Err(denied());}
    for (path,limit,expected) in [(pe,64*1024*1024_u64,pe_sha),
        (pdb,512*1024*1024_u64,pdb_sha)]{
        let m=fs::symlink_metadata(path).map_err(|_|denied())?;
        if !m.is_file()||m.file_attributes()&0x400!=0
            ||m.len()==0||m.len()>limit||expected.len()!=64
            ||!expected.bytes().all(|b|b.is_ascii_hexdigit()){
            return Err(denied());
        }
    }
    let executable=fs::read(pe).map_err(|_|denied())?;
    let symbols=fs::read(pdb).map_err(|_|denied())?;
    if format!("{:x}",Sha256::digest(&executable))!=pe_sha.to_ascii_lowercase()
        ||format!("{:x}",Sha256::digest(&symbols))!=pdb_sha.to_ascii_lowercase(){
        return Err(denied());
    }
    let rsds=pe32plus_rsds(&executable).ok_or_else(denied)?;
    let filename=pdb.file_name().and_then(|v|v.to_str()).ok_or_else(denied)?;
    let stem=pe.file_stem().and_then(|v|v.to_str()).ok_or_else(denied)?;
    if !filename.eq_ignore_ascii_case(&format!("{stem}.pdb"))
        ||!rsds.name.eq_ignore_ascii_case(filename)
        ||!pdb_info_dbi(&symbols,&rsds){return Err(denied());}
    Ok(())
}
#[cfg(windows)]
struct Rsds {
    guid: [u8; 16],
    age: u32,
    name: String,
}
#[cfg(windows)]
fn pe32plus_rsds(data: &[u8]) -> Option<Rsds> {
    if take(data, 0, 2)? != b"MZ" {
        return None;
    }
    let pe = usize::try_from(u32le(data, 0x3c)?).ok()?;
    if take(data, pe, 4)? != b"PE\0\0" {
        return None;
    }
    let coff = pe.checked_add(4)?;
    if u16le(data, coff)? != 0x8664 || u16le(data, coff + 18)? & 0x0002 == 0 {
        return None; // AMD64 executable, not DLL-only/image without entry
    }
    let count = usize::from(u16le(data, coff + 2)?);
    let opt_size = usize::from(u16le(data, coff + 16)?);
    let opt = coff.checked_add(20)?;
    if count == 0
        || count > 96
        || opt_size < 168
        || u16le(data, opt)? != 0x20b
        || u32le(data, opt + 16)? == 0
        || u32le(data, opt + 108)? < 7
    {
        return None;
    }
    take(data, opt, opt_size)?;
    let debug_rva = u32le(data, opt + 160)?;
    let debug_size = u32le(data, opt + 164)?;
    if debug_rva == 0 || debug_size < 28 || debug_size > 28 * 32 || debug_size % 28 != 0 {
        return None;
    }
    let sections = opt.checked_add(opt_size)?;
    let mut x_section = false;
    let mut spans = Vec::new();
    for i in 0..count {
        let sh = take(data, sections.checked_add(i * 40)?, 40)?;
        let raw = u32le(sh, 20)?;
        let raw_size = u32le(sh, 16)?;
        let virtual_address = u32le(sh, 12)?;
        let virtual_size = u32le(sh, 8)?;
        if u32le(sh, 36)? & 0x2000_0000 != 0 && raw_size != 0 {
            x_section = true;
        }
        if raw_size > 0 {
            checked(data, raw.into(), raw_size.into())?;
            spans.push((virtual_address, virtual_size.max(raw_size), raw, raw_size));
        }
    }
    if !x_section {
        return None;
    }
    let rva_to_raw = |rva: u32, length: u32| -> Option<usize> {
        for &(start, virtual_length, raw, raw_length) in &spans {
            let Some(offset) = rva.checked_sub(start) else {
                continue;
            };
            if offset < virtual_length && offset.checked_add(length)? <= raw_length {
                return usize::try_from(raw.checked_add(offset)?).ok();
            }
        }
        None
    };
    let dir = rva_to_raw(debug_rva, debug_size)?;
    let mut match_record = None;
    for i in 0..(debug_size / 28) as usize {
        let record = take(data, dir.checked_add(i * 28)?, 28)?;
        if u32le(record, 12)? != 2 {
            continue;
        }
        let size = usize::try_from(u32le(record, 16)?).ok()?;
        let file_ptr = usize::try_from(u32le(record, 24)?).ok()?;
        if size < 25 || size > 4096 {
            return None;
        }
        let cv = take(data, file_ptr, size)?;
        if take(cv, 0, 4)? != b"RSDS" {
            return None;
        }
        let guid: [u8; 16] = take(cv, 4, 16)?.try_into().ok()?;
        let age = u32le(cv, 20)?;
        if guid == [0u8; 16] || age == 0 {
            return None;
        }
        let rest = cv.get(24..)?;
        let end = rest.iter().position(|b| *b == 0)?;
        let p = std::str::from_utf8(take(rest, 0, end)?).ok()?;
        let leaf = p.rsplit(['\\', '/']).next()?.to_owned();
        if leaf.is_empty()
            || leaf.len() > 200
            || !leaf.ends_with(".pdb")
            || !leaf
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'))
        {
            return None;
        }
        if match_record.is_some() {
            return None;
        } // unambiguous CodeView
        match_record = Some(Rsds {
            guid,
            age,
            name: leaf,
        });
    }
    match_record
}
#[cfg(windows)]
fn pdb_info_dbi(data: &[u8], rsds: &Rsds) -> bool {
    const MAGIC: &[u8] = b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\0\0\0";
    if take(data, 0, 32) != Some(MAGIC) {
        return false;
    }
    let Some(block_size) = u32le(data, 32).map(|v| v as usize) else {
        return false;
    };
    if ![512, 1024, 2048, 4096].contains(&block_size) {
        return false;
    }
    let Some(num_blocks) = u32le(data, 40).map(|v| v as usize) else {
        return false;
    };
    let Some(dir_bytes) = u32le(data, 44).map(|v| v as usize) else {
        return false;
    };
    let Some(map_addr) = u32le(data, 52).map(|v| v as usize) else {
        return false;
    };
    if dir_bytes < 20
        || dir_bytes > 2 * 1024 * 1024
        || num_blocks < 4
        || num_blocks
            .checked_mul(block_size)
            .is_none_or(|size| size > data.len())
    {
        return false;
    }
    let dir_blocks = dir_bytes.div_ceil(block_size);
    if dir_blocks > block_size / 4 || map_addr >= num_blocks {
        return false;
    }
    let Some(map) = take(data, map_addr * block_size, dir_blocks * 4) else {
        return false;
    };
    let mut directory = Vec::with_capacity(dir_bytes);
    for n in 0..dir_blocks {
        let Some(idx) = u32le(map, n * 4).map(|v| v as usize) else {
            return false;
        };
        if idx >= num_blocks {
            return false;
        }
        let Some(bytes) = take(data, idx * block_size, block_size) else {
            return false;
        };
        directory.extend_from_slice(bytes);
    }
    directory.truncate(dir_bytes);
    let Some(num) = u32le(&directory, 0).map(|v| v as usize) else {
        return false;
    };
    if num < 4 || num > 4096 {
        return false;
    }
    let mut sizes = Vec::new();
    for i in 0..num {
        let Some(size) = u32le(&directory, 4 + i * 4) else {
            return false;
        };
        sizes.push(size);
    }
    let mut cursor = 4 + num * 4;
    let mut info = None;
    let mut dbi = None;
    for (i, size) in sizes.iter().enumerate() {
        if *size == 0xffff_ffff {
            continue;
        }
        let byte_size = *size as usize;
        let count = byte_size.div_ceil(block_size);
        if count > num_blocks
            || cursor
                .checked_add(count * 4)
                .is_none_or(|n| n > directory.len())
        {
            return false;
        }
        if i == 1 || i == 3 {
            let wanted = if i == 1 { 28 } else { 64 };
            if byte_size < wanted {
                return false;
            }
            let mut head = Vec::with_capacity(wanted);
            for b in 0..count {
                if head.len() >= wanted {
                    break;
                }
                let Some(idx) = u32le(&directory, cursor + b * 4).map(|v| v as usize) else {
                    return false;
                };
                if idx >= num_blocks {
                    return false;
                }
                let Some(chunk) = take(data, idx * block_size, block_size) else {
                    return false;
                };
                let need = (wanted - head.len()).min(block_size);
                head.extend_from_slice(&chunk[..need]);
            }
            if i == 1 {
                info = Some(head);
            } else {
                dbi = Some(head);
            }
        }
        cursor += count * 4;
    }
    let Some(info) = info else {
        return false;
    };
    let Some(dbi) = dbi else {
        return false;
    };
    // PDB stream 1: Version, Signature, Age, GUID.
    // DBI stream 3: Signature=-1, Version, Age, ... Machine at offset 58.
    u32le(&info, 0).is_some_and(|v| v >= 19941610)
        && u32le(&info, 8) == Some(rsds.age)
        && take(&info, 12, 16) == Some(rsds.guid.as_slice())
        && u32le(&dbi, 0) == Some(0xffff_ffff)
        && u32le(&dbi, 8) == Some(rsds.age)
        && u16le(&dbi, 58) == Some(0x8664)
}

/// Result scope: structural executable and selected debug-info identifiers
/// measured from current exact native output bytes. NOT a signer, full
/// DWARF/CodeView validator, proof of source-line mapping or DAP launch grant.
pub fn verify_build_debug_symbols(
    root: &Path,
    output: &OutputWitness,
    binary: &DebugBinaryWitness,
) -> Result<Option<DebugSymbolWitness>, SupervisorError> {
    if output.truncated
        || output.file_count != output.files.len()
        || output.role != MANIFEST_ROLE
        || binary.output_tree_sha256 != output.tree_sha256
        || binary.debugger_execution_authorized
        || binary.debug_symbols_attested
    {
        return Ok(None);
    }
    let entries: Vec<_> = output
        .files
        .iter()
        .filter(|e| {
            e.relative_path == binary.relative_path
                && e.sha256 == binary.binary_sha256
                && e.bytes == binary.bytes
        })
        .collect();
    if entries.len() != 1 {
        return Ok(None);
    }
    let executable = read_attested(root, entries[0], output)?;
    #[cfg(target_os = "linux")]
    let observation = {
        let Some(sym_hash) = elf64_dwarf_sections(&executable) else {
            return Ok(None);
        };
        ( "elf64_x86_64_executable_dwarf_sections",
          "embedded_dwarf_info_abbrev_line",
          None, sym_hash,
          "elf64_headers_load_segments_and_dwarf_unit_headers_structurally_checked_not_full_die_validation"
        )
    };
    #[cfg(windows)]
    let observation = {
        let Some(rsds) = pe32plus_rsds(&executable) else {
            return Ok(None);
        };
        // Bind the exact PDB sibling to the independently selected native
        // executable. Sealed Windows CARGO_TARGET_DIR uses debug/; retained
        // historical snapshots use target/debug/. Never mix the two roots.
        let prefix = if binary.relative_path.starts_with("debug/") {
            "debug/"
        } else if binary.relative_path.starts_with("target/debug/") {
            "target/debug/"
        } else {
            return Ok(None);
        };
        let leaf = binary.relative_path.strip_prefix(prefix)
            .and_then(|n| n.strip_suffix(".exe"));
        let Some(stem) = leaf else {
            return Ok(None);
        };
        if stem.is_empty() || stem.contains('/') || rsds.name != format!("{stem}.pdb") {
            return Ok(None);
        }
        let pdb_relative = format!("{prefix}{}", rsds.name);
        let pdbs: Vec<_> = output
            .files
            .iter()
            .filter(|file| file.relative_path == pdb_relative)
            .collect();
        if pdbs.len() != 1 {
            return Ok(None);
        }
        let pdb = read_attested(root, pdbs[0], output)?;
        if !pdb_info_dbi(&pdb, &rsds) {
            return Ok(None);
        }
        (
            "pe32plus_x86_64_executable_codeview_rsds",
            "external_pdb_guid_age_dbi_match",
            Some(pdb_relative),
            pdbs[0].sha256.clone(),
            "pe_debug_directory_rsds_guid_age_matches_pdb_info_and_dbi_not_full_symbol_parse",
        )
    };
    #[cfg(not(any(windows, target_os = "linux")))]
    let observation = { return Ok(None) };
    Ok(Some(DebugSymbolWitness {
        binary_relative_path: binary.relative_path.clone(),
        binary_sha256: binary.binary_sha256.clone(),
        output_tree_sha256: binary.output_tree_sha256.clone(),
        executable_format: observation.0,
        symbols_kind: observation.1,
        symbols_relative_path: observation.2,
        symbols_sha256: observation.3,
        validation_scope: observation.4,
        role: ROLE,
        debug_symbols_attested: true,
        source_line_mapping_verified: false,
        debugger_execution_authorized: false,
        installed_app_accepted: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put16(b: &mut [u8], p: usize, v: u16) {
        b[p..p + 2].copy_from_slice(&v.to_le_bytes());
    }
    fn put32(b: &mut [u8], p: usize, v: u32) {
        b[p..p + 4].copy_from_slice(&v.to_le_bytes());
    }
    fn put64(b: &mut [u8], p: usize, v: u64) {
        b[p..p + 8].copy_from_slice(&v.to_le_bytes());
    }
    #[cfg(target_os = "linux")]
    fn minimal_dwarf_elf() -> Vec<u8> {
        let mut b = vec![0; 0x400];
        b[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        put16(&mut b, 16, 2);
        put16(&mut b, 18, 62);
        put32(&mut b, 20, 1);
        put64(&mut b, 24, 0x400100);
        put64(&mut b, 32, 0x40);
        put64(&mut b, 40, 0x200);
        put16(&mut b, 52, 64);
        put16(&mut b, 54, 56);
        put16(&mut b, 56, 1);
        put16(&mut b, 58, 64);
        put16(&mut b, 60, 5);
        put16(&mut b, 62, 1);
        put32(&mut b, 0x40, 1);
        put32(&mut b, 0x44, 5);
        let names = b"\0.shstrtab\0.debug_info\0.debug_abbrev\0.debug_line\0";
        b[0x100..0x100 + names.len()].copy_from_slice(names);
        let mut add_section = |index: usize, name: &[u8], kind: u32, offset: u64, size: u64| {
            let name_offset = names.windows(name.len()).position(|v| v == name).unwrap() as u32;
            let at = 0x200 + index * 64;
            put32(&mut b, at, name_offset);
            put32(&mut b, at + 4, kind);
            put64(&mut b, at + 24, offset);
            put64(&mut b, at + 32, size);
        };
        add_section(1, b".shstrtab", 3, 0x100, names.len() as u64);
        add_section(2, b".debug_info", 1, 0x140, 12);
        add_section(3, b".debug_abbrev", 1, 0x160, 12);
        add_section(4, b".debug_line", 1, 0x180, 12);
        put32(&mut b, 0x140, 8);
        put16(&mut b, 0x144, 4);
        put32(&mut b, 0x146, 0);
        b[0x14a] = 8;
        b[0x160] = 1;
        put32(&mut b, 0x180, 8);
        put16(&mut b, 0x184, 4);
        b
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn completed_native_build_seals_private_one_use_project_debug_target() {
        use crate::{
            debug_binary_witness::audit_debug_binary_candidate,
            project_debug_target::{seal_project_debug_target, CompletedBuildLineage},
            wo6_output_evidence::audit_published_output,
        };
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("lu-dev044-{:032x}", rand::random::<u128>()));
        let output = root.join("build");
        let private = root.join("private");
        fs::create_dir_all(output.join("target/debug")).unwrap();
        fs::create_dir_all(&private).unwrap();
        let file = output.join("target/debug/fixture");
        fs::write(&file, minimal_dwarf_elf()).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o700)).unwrap();
        let output_manifest = audit_published_output(&output).unwrap();
        let binary = audit_debug_binary_candidate(&output, &output_manifest)
            .unwrap()
            .unwrap();
        let symbols = verify_build_debug_symbols(&output, &output_manifest, &binary)
            .unwrap()
            .unwrap();
        let lineage = CompletedBuildLineage {
            project_id: format!("ide-{}", "a".repeat(32)),
            build_job_id: format!("job-{}", "b".repeat(32)),
            build_plan_sha256: "c".repeat(64),
            terminal_receipt_sha256: "d".repeat(64),
            source_head: "e".repeat(40),
            source_snapshot_sha256: "f".repeat(64),
            manifest_sha256: "1".repeat(64),
            build_toolchain_sha256: "2".repeat(64),
            output_tree_sha256: output_manifest.tree_sha256.clone(),
            build_successfully_completed: true,
            requires_reconciliation: false,
        };
        let target =
            seal_project_debug_target(&output, &private, lineage, &binary, &symbols).unwrap();
        target.verify().unwrap();
        assert_eq!(target.binary.binary_sha256, binary.binary_sha256);
        assert_eq!(target.binding_sha256.len(), 64);
        // Even a host-side replacement after private copy must prevent
        // approval or DAP launch. No stale source/build identity is replayed.
        let sealed = target.root.join("target/debug/fixture");
        fs::set_permissions(&sealed, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(&sealed, b"\\x7fELFtampered").unwrap();
        assert!(target.verify().is_err());
        drop(target);
        assert!(fs::read_dir(&private).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn structural_elf_dwarf_requires_real_section_tables_units_and_exec_segment() {
        let source = minimal_dwarf_elf();
        assert!(elf64_dwarf_sections(&source).is_some());
        let mut missing = source.clone();
        put16(&mut missing, 60, 0);
        assert!(elf64_dwarf_sections(&missing).is_none());
        let mut no_compile = source.clone();
        put16(&mut no_compile, 0x144, 0);
        assert!(elf64_dwarf_sections(&no_compile).is_none());
        let mut wrong_arch = source.clone();
        put16(&mut wrong_arch, 18, 3);
        assert!(elf64_dwarf_sections(&wrong_arch).is_none());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn actual_output_hash_recheck_and_missing_symbols_denied() {
        use crate::{
            debug_binary_witness::audit_debug_binary_candidate,
            wo6_output_evidence::audit_published_output,
        };
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("lu-dev043-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(root.join("target/debug")).unwrap();
        let file = root.join("target/debug/fixture");
        fs::write(&file, minimal_dwarf_elf()).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o700)).unwrap();
        let output = audit_published_output(&root).unwrap();
        let binary = audit_debug_binary_candidate(&root, &output)
            .unwrap()
            .unwrap();
        let attested = verify_build_debug_symbols(&root, &output, &binary)
            .unwrap()
            .unwrap();
        assert_eq!(attested.binary_sha256, binary.binary_sha256);
        assert!(attested.debug_symbols_attested);
        assert!(!attested.source_line_mapping_verified);
        assert!(!attested.debugger_execution_authorized);
        let mut stripped = minimal_dwarf_elf();
        put16(&mut stripped, 60, 0);
        fs::write(&file, stripped).unwrap();
        assert!(verify_build_debug_symbols(&root, &output, &binary).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    fn minimal_pe() -> Vec<u8> {
        let mut data = vec![0; 0x800];
        data[..2].copy_from_slice(b"MZ");
        put32(&mut data, 0x3c, 0x80);
        data[0x80..0x84].copy_from_slice(b"PE\0\0");
        put16(&mut data, 0x84, 0x8664);
        put16(&mut data, 0x86, 2);
        put16(&mut data, 0x94, 0xf0);
        put16(&mut data, 0x96, 0x0002);
        let opt = 0x98;
        put16(&mut data, opt, 0x20b);
        put32(&mut data, opt + 16, 0x1000);
        put32(&mut data, opt + 108, 16);
        put32(&mut data, opt + 160, 0x2000);
        put32(&mut data, opt + 164, 28);
        let sec = opt + 0xf0;
        data[sec..sec + 5].copy_from_slice(b".text");
        put32(&mut data, sec + 8, 0x100);
        put32(&mut data, sec + 12, 0x1000);
        put32(&mut data, sec + 16, 0x100);
        put32(&mut data, sec + 20, 0x300);
        put32(&mut data, sec + 36, 0x6000_0020);
        let sec2 = sec + 40;
        data[sec2..sec2 + 6].copy_from_slice(b".rdata");
        put32(&mut data, sec2 + 8, 0x200);
        put32(&mut data, sec2 + 12, 0x2000);
        put32(&mut data, sec2 + 16, 0x200);
        put32(&mut data, sec2 + 20, 0x400);
        put32(&mut data, sec2 + 36, 0x4000_0040);
        put32(&mut data, 0x400 + 12, 2);
        put32(&mut data, 0x400 + 16, 36);
        put32(&mut data, 0x400 + 24, 0x450);
        data[0x450..0x454].copy_from_slice(b"RSDS");
        data[0x454..0x464].copy_from_slice(&[1u8; 16]);
        put32(&mut data, 0x464, 1);
        data[0x468..0x474].copy_from_slice(b"fixture.pdb\0");
        data
    }
    #[cfg(windows)]
    fn minimal_matching_pdb() -> Vec<u8> {
        let mut pdb = vec![0; 512 * 8];
        pdb[..32].copy_from_slice(b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\0\0\0");
        put32(&mut pdb, 32, 512);
        put32(&mut pdb, 40, 8);
        put32(&mut pdb, 44, 4 + 4 * 4 + 2 * 4);
        put32(&mut pdb, 52, 3);
        put32(&mut pdb, 3 * 512, 4);
        let dir = 4 * 512;
        put32(&mut pdb, dir, 4);
        put32(&mut pdb, dir + 4, 0);
        put32(&mut pdb, dir + 8, 28);
        put32(&mut pdb, dir + 12, 0);
        put32(&mut pdb, dir + 16, 64);
        put32(&mut pdb, dir + 20, 5);
        put32(&mut pdb, dir + 24, 6);
        let info = 5 * 512;
        put32(&mut pdb, info, 20000404);
        put32(&mut pdb, info + 8, 1);
        pdb[info + 12..info + 28].copy_from_slice(&[1u8; 16]);
        let dbi = 6 * 512;
        put32(&mut pdb, dbi, 0xffff_ffff);
        put32(&mut pdb, dbi + 8, 1);
        put16(&mut pdb, dbi + 58, 0x8664);
        pdb
    }
    #[cfg(windows)]
    #[test]
    fn sealed_cargo_output_debug_root_requires_matching_sibling_pdb() {
        use crate::{
            debug_binary_witness::audit_debug_binary_candidate,
            wo6_output_evidence::audit_published_output,
        };
        let root = std::env::temp_dir().join(format!(
            "fin01d-native-cargo-debug-{:032x}", rand::random::<u128>()
        ));
        fs::create_dir_all(root.join("debug")).unwrap();
        fs::write(root.join("debug/fixture.exe"), minimal_pe()).unwrap();
        fs::write(root.join("debug/fixture.pdb"), minimal_matching_pdb()).unwrap();
        let manifest = audit_published_output(&root).unwrap();
        let binary = audit_debug_binary_candidate(&root, &manifest).unwrap().unwrap();
        assert_eq!(binary.relative_path, "debug/fixture.exe");
        let symbols = verify_build_debug_symbols(&root, &manifest, &binary)
            .unwrap().unwrap();
        assert_eq!(symbols.symbols_kind, "external_pdb_guid_age_dbi_match");
        assert_eq!(symbols.symbols_relative_path.as_deref(), Some("debug/fixture.pdb"));
        fs::write(root.join("debug/fixture.pdb"), b"forged PDB").unwrap();
        assert!(verify_build_debug_symbols(&root, &manifest, &binary).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn private_appcontainer_target_seal_rejects_changed_pdb_and_preserves_lineage() {
        use crate::{
            debug_binary_witness::audit_debug_binary_candidate,
            project_debug_target::{seal_project_debug_target, CompletedBuildLineage},
            wo6_output_evidence::audit_published_output,
        };
        let root =
            std::env::temp_dir().join(format!("lu-dev044-win-{:032x}", rand::random::<u128>()));
        let output = root.join("build");
        let private = root.join("sealed");
        fs::create_dir_all(output.join("target/debug")).unwrap();
        fs::create_dir_all(&private).unwrap();
        fs::write(output.join("target/debug/fixture.exe"), minimal_pe()).unwrap();
        fs::write(
            output.join("target/debug/fixture.pdb"),
            minimal_matching_pdb(),
        )
        .unwrap();
        let manifest = audit_published_output(&output).unwrap();
        let binary = audit_debug_binary_candidate(&output, &manifest)
            .unwrap()
            .unwrap();
        let symbols = verify_build_debug_symbols(&output, &manifest, &binary)
            .unwrap()
            .unwrap();
        let lineage = CompletedBuildLineage {
            project_id: format!("ide-{}", "a".repeat(32)),
            build_job_id: format!("job-{}", "b".repeat(32)),
            build_plan_sha256: "c".repeat(64),
            terminal_receipt_sha256: "d".repeat(64),
            source_head: "e".repeat(40),
            source_snapshot_sha256: "f".repeat(64),
            manifest_sha256: "1".repeat(64),
            build_toolchain_sha256: "2".repeat(64),
            output_tree_sha256: manifest.tree_sha256.clone(),
            build_successfully_completed: true,
            requires_reconciliation: false,
        };
        let target =
            seal_project_debug_target(&output, &private, lineage, &binary, &symbols).unwrap();
        target.verify().unwrap();
        let staged = target.root.join("target/debug/fixture.pdb");
        let mut forged = minimal_matching_pdb();
        put32(&mut forged, 5 * 512 + 8, 2);
        fs::write(&staged, forged).unwrap();
        assert!(target.verify().is_err());
        drop(target);
        assert!(fs::read_dir(&private).unwrap().next().is_none());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn g04_enclosed_cargo_elf_dwarf_alias_is_valid_but_external_alias_denied() {
        use crate::{
            debug_binary_witness::audit_debug_binary_candidate,
            wo6_output_evidence::audit_published_output,
        };
        use std::os::unix::fs::PermissionsExt;
        let root =
            std::env::temp_dir().join(format!("lu-g04-dwarf-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(root.join("target/debug/deps")).unwrap();
        let image = root.join("target/debug/fixture");
        fs::write(&image, minimal_dwarf_elf()).unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o700)).unwrap();
        fs::hard_link(&image, root.join("target/debug/deps/fixture-fixed")).unwrap();
        let witness = audit_published_output(&root).unwrap();
        let candidate = audit_debug_binary_candidate(&root, &witness)
            .unwrap()
            .unwrap();
        assert!(verify_build_debug_symbols(&root, &witness, &candidate)
            .unwrap()
            .is_some());
        let outside = std::env::temp_dir().join(format!(
            "lu-g04-dwarf-escape-{:032x}",
            rand::random::<u128>()
        ));
        fs::hard_link(&image, &outside).unwrap();
        // Neither the executable nor the DWARF reader can accept missing
        // link provenance outside the original protected output inventory.
        assert!(verify_build_debug_symbols(&root, &witness, &candidate).is_err());
        fs::remove_file(outside).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn full_native_output_manifest_pe_pdb_witness_rehashes_both_files() {
        use crate::{
            debug_binary_witness::audit_debug_binary_candidate,
            wo6_output_evidence::audit_published_output,
        };
        let root =
            std::env::temp_dir().join(format!("lu-dev043-pe-{:032x}", rand::random::<u128>()));
        fs::create_dir_all(root.join("target/debug")).unwrap();
        let exe = root.join("target/debug/fixture.exe");
        let pdb = root.join("target/debug/fixture.pdb");
        fs::write(&exe, minimal_pe()).unwrap();
        fs::write(&pdb, minimal_matching_pdb()).unwrap();
        let output = audit_published_output(&root).unwrap();
        let binary = audit_debug_binary_candidate(&root, &output)
            .unwrap()
            .unwrap();
        let result = verify_build_debug_symbols(&root, &output, &binary)
            .unwrap()
            .unwrap();
        assert!(result.debug_symbols_attested);
        assert_eq!(result.symbols_kind, "external_pdb_guid_age_dbi_match");
        assert_eq!(
            result.symbols_relative_path.as_deref(),
            Some("target/debug/fixture.pdb")
        );
        assert!(!result.debugger_execution_authorized);
        let mut wrong = minimal_matching_pdb();
        put32(&mut wrong, 5 * 512 + 8, 2);
        fs::write(&pdb, wrong).unwrap();
        assert!(verify_build_debug_symbols(&root, &output, &binary).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn pe_rsds_and_pdb_msf_guid_age_dbi_machine_are_required() {
        let pe = minimal_pe();
        let rsds = pe32plus_rsds(&pe).unwrap();
        assert_eq!(rsds.name, "fixture.pdb");
        let pdb = minimal_matching_pdb();
        assert!(pdb_info_dbi(&pdb, &rsds));
        let mut wrong = pdb.clone();
        put32(&mut wrong, 5 * 512 + 8, 2);
        assert!(!pdb_info_dbi(&wrong, &rsds));
        wrong = pdb.clone();
        put16(&mut wrong, 6 * 512 + 58, 0x014c);
        assert!(!pdb_info_dbi(&wrong, &rsds));
        let mut bogus = pe.clone();
        bogus[0x450..0x454].copy_from_slice(b"NB10");
        assert!(pe32plus_rsds(&bogus).is_none());
    }
}
