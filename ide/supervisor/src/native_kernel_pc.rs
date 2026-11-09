//! DEV-04.5D3 — native-only Linux kernel image witness for a stopped
//! project DAP frame. This does not grant execution, certify an adapter's
//! program counter, parse DWARF or authorize a debugger command.
//! Only an already admitted cgroup-v2 worker may call this with its private
//! cgroup directory and independently sealed executable.
#![cfg(target_os = "linux")]
use crate::SupervisorError;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, Metadata},
    io::{Read, BufRead, BufReader},
    os::unix::fs::MetadataExt,
    path::Path,
};

const MAX_EXECUTABLE: u64 = 64 * 1024 * 1024;
const MAX_CGROUP_PROCS: usize = 512;
const MAX_MAPPINGS: usize = 16_384;
fn denied() -> SupervisorError { SupervisorError::NoContainment }

#[derive(Debug, Clone)]
pub struct KernelLoadedImage {
    // Kept exclusively in the native host; never serialized into WebView.
    pub pid: u32,
    pub instruction_address: u64,
    pub load_bias: u64,
    pub binary_sha256: String,
    pub kernel_exe_inode_verified: bool,
    pub kernel_rx_vma_verified: bool,
    pub kernel_tracing_stop_verified: bool,
}

/// The mapped executable's current on-disk bytes must still match the
/// protected Cargo build's previously sealed bytes. Not an adapter path.
fn hash_bounded(path: &Path) -> Result<String, SupervisorError> {
    let meta = fs::metadata(path).map_err(|_| denied())?;
    if !meta.is_file() || meta.len() == 0 || meta.len() > MAX_EXECUTABLE {
        return Err(denied());
    }
    let mut input = File::open(path).map_err(|_| denied())?;
    let mut hash = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let size = input.read(&mut buf).map_err(|_| denied())?;
        if size == 0 { break; }
        total = total.checked_add(size as u64).ok_or_else(denied)?;
        if total > MAX_EXECUTABLE { return Err(denied()); }
        hash.update(&buf[..size]);
    }
    if total != meta.len() { return Err(denied()); }
    Ok(format!("{:x}", hash.finalize()))
}
fn major(dev: u64) -> u64 { ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff) }
fn minor(dev: u64) -> u64 { (dev & 0xff) | ((dev >> 12) & !0xff) }
fn ule(data: &[u8], start: usize, count: usize) -> Option<u64> {
    let slice = data.get(start..start.checked_add(count)?)?;
    let mut word = [0u8; 8];
    word.get_mut(..count)?.copy_from_slice(slice);
    Some(u64::from_le_bytes(word))
}
fn aligned(address: u64) -> u64 { address & !0xfff }
/// Independently derive actual ELF load bias from executable program headers,
/// *not* an adapter-provided module base or a guessed ASLR offset.
fn executable_slide(binary: &[u8], map_start: u64, map_offset: u64) -> Option<u64> {
    if binary.get(0..4)? != b"\x7fELF" || *binary.get(4)? != 2
        || *binary.get(5)? != 1 { return None; }
    let elf_type = ule(binary, 16, 2)?;
    if !matches!(elf_type, 2 | 3) { return None; }
    let phoff = usize::try_from(ule(binary, 32, 8)?).ok()?;
    let entsize = usize::try_from(ule(binary, 54, 2)?).ok()?;
    let phnum = usize::try_from(ule(binary, 56, 2)?).ok()?;
    if entsize < 56 || phnum == 0 || phnum > 64 { return None; }
    let mut bias = None;
    for i in 0..phnum {
        let at = phoff.checked_add(i.checked_mul(entsize)?)?;
        let ph = binary.get(at..at.checked_add(56)?)?;
        let ptype = ule(ph, 0, 4)?;
        let flags = ule(ph, 4, 4)?;
        let offset = ule(ph, 8, 8)?;
        let vaddr = ule(ph, 16, 8)?;
        if ptype != 1 || flags & 1 == 0 || aligned(offset) != map_offset { continue; }
        let observed = map_start.checked_sub(aligned(vaddr))?;
        if elf_type == 2 && observed != 0 { return None; }
        if let Some(existing) = bias {
            if existing != observed { return None; }
        }
        bias = Some(observed);
    }
    bias
}
fn start_ticks(pid: u32) -> Result<u64, SupervisorError> {
    // /proc/PID/stat has a parenthesized comm, which may contain spaces.
    let data = fs::read_to_string(format!("/proc/{pid}/stat")).map_err(|_| denied())?;
    let end = data.rfind(") ").ok_or_else(denied)?;
    let fields: Vec<_> = data[end + 2..].split_whitespace().collect();
    fields.get(19).ok_or_else(denied)?.parse::<u64>().map_err(|_| denied())
}
fn tracing_stop(pid: u32) -> Result<bool, SupervisorError> {
    let data = fs::read_to_string(format!("/proc/{pid}/status")).map_err(|_| denied())?;
    let state = data.lines().find_map(|line| line.strip_prefix("State:"))
        .ok_or_else(denied)?.trim_start();
    Ok(state.starts_with("t ") || state.starts_with("T "))
}
fn member(dir: &Path, pid: u32) -> Result<bool, SupervisorError> {
    let data = fs::read_to_string(dir.join("cgroup.procs")).map_err(|_| denied())?;
    if data.len() > MAX_CGROUP_PROCS { return Err(denied()); }
    Ok(data.lines().any(|line| line.parse::<u32>().ok() == Some(pid)))
}
fn pid_mapping(
    pid: u32, meta: &Metadata, pc: u64, image: &[u8],
) -> Result<Option<u64>, SupervisorError> {
    let maps = File::open(format!("/proc/{pid}/maps")).map_err(|_| denied())?;
    let mut count = 0;
    let mut found = None;
    for row in BufReader::new(maps).lines() {
        count += 1;
        if count > MAX_MAPPINGS { return Err(denied()); }
        let row = row.map_err(|_| denied())?;
        let parts: Vec<_> = row.split_whitespace().take(6).collect();
        if parts.len() < 5 { return Err(denied()); }
        let (a, b) = parts[0].split_once('-').ok_or_else(denied)?;
        let low = u64::from_str_radix(a, 16).map_err(|_| denied())?;
        let high = u64::from_str_radix(b, 16).map_err(|_| denied())?;
        if pc < low || pc >= high || !parts[1].contains('x') { continue; }
        let offset = u64::from_str_radix(parts[2], 16).map_err(|_| denied())?;
        let (dev_major, dev_minor) = parts[3].split_once(':').ok_or_else(denied)?;
        let mapped_major = u64::from_str_radix(dev_major, 16).map_err(|_| denied())?;
        let mapped_minor = u64::from_str_radix(dev_minor, 16).map_err(|_| denied())?;
        let inode = parts[4].parse::<u64>().map_err(|_| denied())?;
        if inode != meta.ino() || mapped_major != major(meta.dev())
            || mapped_minor != minor(meta.dev()) {
            continue;
        }
        let slide = executable_slide(image, low, offset).ok_or_else(denied)?;
        if found.replace(slide).is_some() { return Err(denied()); }
    }
    Ok(found)
}
fn verify_pid(pid: u32, expected_binary: &Path, sha: &str, pc: u64)
    -> Result<Option<KernelLoadedImage>, SupervisorError>
{
    let first = start_ticks(pid)?;
    if !tracing_stop(pid)? { return Ok(None); }
    let original = fs::metadata(expected_binary).map_err(|_| denied())?;
    let exe_path = std::path::PathBuf::from(format!("/proc/{pid}/exe"));
    let current = fs::metadata(&exe_path).map_err(|_| denied())?;
    if original.dev() != current.dev() || original.ino() != current.ino()
        || original.len() != current.len() {
        return Ok(None);
    }
    if hash_bounded(&exe_path)? != sha { return Err(denied()); }
    let image = fs::read(expected_binary).map_err(|_| denied())?;
    if image.len() as u64 != original.len() { return Err(denied()); }
    let slide = pid_mapping(pid, &original, pc, &image)?;
    let Some(load_bias) = slide else { return Ok(None); };
    // Protect against PID reuse / ELF remapping / scope change during read.
    if first != start_ticks(pid)? || !tracing_stop(pid)? {
        return Err(denied());
    }
    Ok(Some(KernelLoadedImage {
        pid, instruction_address: pc, load_bias,
        binary_sha256: sha.to_owned(),
        kernel_exe_inode_verified: true, kernel_rx_vma_verified: true,
        kernel_tracing_stop_verified: true,
    }))
}
/// Correlates three independent authorities, but does not authorize a
/// new DAP command: host-only source snapshot, kernel cgroup mapping and
/// separately fsynced original build/one-use debug job receipts.
#[derive(Debug, Clone)]
pub struct ProtectedStopLineage {
    pub kernel: KernelLoadedImage,
    pub debugger_job_id: String,
    pub debugger_plan_sha256: String,
    pub debug_running_receipt_sha256: String,
    pub build_job_id: String,
    pub build_terminal_receipt_sha256: String,
    pub source_snapshot_sha256: String,
    pub target_binding_sha256: String,
    pub source_file_sha256: String,
    pub source_relative_path: String,
    pub source_line: u32,
    pub independent_line_table_verified: bool,
    pub installed_app_accepted: bool,
}

/// A true native witness must remain scoped to the real delegated cgroup,
/// whose directory was created by the protected OS worker from its job ID.
/// Arbitrary caller-supplied PIDs or directories are never trusted.
pub(crate) fn observe_linux_project_pc(
    group: &Path, job_id: &str, expected_binary: &Path,
    expected_sha256: &str, pc: u64,
) -> Result<KernelLoadedImage, SupervisorError> {
    if !job_id.starts_with("job-") || job_id.len() != 36
        || !job_id[4..].bytes().all(|b| b.is_ascii_hexdigit())
        || group.file_name().and_then(|v| v.to_str()) !=
            Some(format!("lu-ide-{job_id}").as_str())
        || !group.starts_with("/sys/fs/cgroup/")
        || !expected_binary.is_absolute()
        || pc == 0
        || expected_sha256.len() != 64
        || !expected_sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    { return Err(denied()); }
    // The worker's sealed target exists in a different read-only mount
    // namespace but must be the same backing inode and bytes as /proc/PID/exe.
    if hash_bounded(expected_binary)? != expected_sha256 { return Err(denied()); }
    let processes = fs::read_to_string(group.join("cgroup.procs")).map_err(|_| denied())?;
    if processes.len() > MAX_CGROUP_PROCS { return Err(denied()); }
    let mut result: Option<KernelLoadedImage> = None;
    for line in processes.lines() {
        let pid = line.parse::<u32>().map_err(|_| denied())?;
        if let Some(witness) = verify_pid(pid, expected_binary, expected_sha256, pc)? {
            if result.is_some() { return Err(denied()); }
            result = Some(witness);
        }
    }
    let witness = result.ok_or_else(denied)?;
    if !member(group, witness.pid)? { return Err(denied()); }
    let from_proc = fs::read_to_string(format!("/proc/{}/cgroup", witness.pid))
        .map_err(|_| denied())?;
    let marker = format!("/lu-ide-{job_id}");
    if !from_proc.lines().any(|line| line.starts_with("0::") && line.ends_with(&marker)) {
        return Err(denied());
    }
    Ok(witness)
}
/// A separately authenticated, fixed-purpose LLVM reader. Strictly a
/// debug/CI-only native oracle, never launched from an AI/WebView command.
/// A production signed app must provide a reviewed offline resource and
/// a separate OS policy admission before enabling a comparable reader.
#[cfg(debug_assertions)]
pub(crate) fn verify_independent_dwarf_line_for_ci(
    witness: &mut ProtectedStopLineage,
    target: &crate::project_debug_target::SealedProjectDebugTarget,
) -> Result<(), SupervisorError> {
    use std::{os::unix::fs::{MetadataExt, PermissionsExt}, process::Command};
    const TOOL: &str = "/usr/lib/llvm-18/bin/llvm-symbolizer";
    const PIN: Option<&str> = option_env!("LU_IDE_D3_LINUX_SYMBOLIZER_SHA256");
    let pin = PIN.ok_or_else(denied)?;
    let tool = Path::new(TOOL);
    let meta = fs::symlink_metadata(tool).map_err(|_| denied())?;
    if !meta.is_file() || meta.file_type().is_symlink()
        || meta.uid() != 0 || meta.permissions().mode() & 0o022 != 0
        || hash_bounded(tool)? != pin {
        return Err(denied());
    }
    target.verify()?;
    if witness.kernel.binary_sha256 != target.binary.binary_sha256
        || witness.target_binding_sha256 != target.binding_sha256
        || !witness.kernel.kernel_exe_inode_verified
        || !witness.kernel.kernel_rx_vma_verified
        || !witness.kernel.kernel_tracing_stop_verified
        || witness.source_line == 0
    {
        return Err(denied());
    }
    let binary = target.root.join(target.confined_relative_binary());
    let pc = format!("0x{:x}", witness.kernel.instruction_address);
    let slide = format!("--adjust-vma=0x{:x}", witness.kernel.load_bias);
    let response = Command::new(tool)
        .arg(format!("--obj={}", binary.display()))
        .arg("--output-style=JSON")
        .arg("--functions=none")
        .arg("--no-inlines")
        .arg(slide)
        .arg(&pc)
        .env_clear()
        .output().map_err(|_| denied())?;
    if !response.status.success() || response.stdout.len() > 8192
        || response.stderr.len() > 8192 {
        return Err(denied());
    }
    let parsed: serde_json::Value = serde_json::from_slice(&response.stdout)
        .map_err(|_| denied())?;
    let entries = parsed.as_array().ok_or_else(denied)?;
    if entries.len() != 1 || entries[0].get("Address").and_then(|v| v.as_str()) != Some(pc.as_str())
        || entries[0].get("ModuleName").and_then(|v| v.as_str()) != binary.to_str()
    {
        return Err(denied());
    }
    let lines = entries[0].get("Symbol").and_then(|v| v.as_array()).ok_or_else(denied)?;
    if lines.len() != 1 { return Err(denied()); }
    let expected_source = format!("/workspace/{}", witness.source_relative_path);
    if lines[0].get("FileName").and_then(|v| v.as_str()) != Some(expected_source.as_str())
        || lines[0].get("Line").and_then(|v| v.as_u64())
            != Some(u64::from(witness.source_line))
    {
        return Err(denied());
    }
    witness.independent_line_table_verified = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_elf_program_header_slide_is_bounded_and_not_dap_supplied() {
        let exe = fs::read("/proc/self/exe").unwrap();
        assert!(executable_slide(&exe, 0, 0).is_none());
        assert!(executable_slide(&[0; 64], 0x400000, 0).is_none());
    }
    #[test]
    fn untrusted_group_or_missing_custody_is_denied_before_any_pid() {
        assert!(observe_linux_project_pc(
            Path::new("/tmp/not-a-cgroup"), "job-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            Path::new("/tmp/foreign"), &"0".repeat(64), 0x401000,
        ).is_err());
    }
}
