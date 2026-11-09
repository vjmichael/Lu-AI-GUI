//! DEV-04.5D4C: CI-only independent source-line reader for the *real*
//! protected Lu Windows LLDB-DAP stop.
//!
//! This does not read a fake DAP event, open a user-specified executable,
//! launch a debugger, or add source breakpoints/stepping/variable rights.
//! A separate OS query already established Job membership + PE module base
//! and an opened, owner-checked thread context matching the DAP PC.
//! LLVM decodes the actual compiler PDB independently of LLDB-DAP.
//! The parent WindowsDapSession must verify original durable approvals,
//! source snapshots, receipt hashes and unchanged stopped DAP state again.
#![cfg(all(windows, debug_assertions, feature = "dev045d4c-ci-oracle"))]
use crate::{
    project_debug_target::SealedProjectDebugTarget,
    windows_kernel_pc::{WindowsLoadedModuleWitness, WindowsThreadRipSample},
    SupervisorError,
};
use lu_ide_workspace::dap_source_lines::SourceLineCandidate;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    os::windows::fs::MetadataExt,
    path::{Component, Path},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_SOURCE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ORACLE_OUTPUT: u64 = 8192;
const MAX_ORACLE_RUNTIME: Duration = Duration::from_secs(6);

fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}
fn lower_file_name(path: &Path) -> Option<String> {
    Some(path.file_name()?.to_str()?.to_ascii_lowercase())
}
fn normalized(path: &str) -> Option<String> {
    if path.is_empty() || path.len() > 2048 || path.chars().any(char::is_control) {
        return None;
    }
    let s = path.replace('/', "\\").to_lowercase();
    let s = s.strip_prefix("\\\\?\\").unwrap_or(&s);
    if s.starts_with("\\\\") || s.split('\\').any(|x| x == ".." || x == ".") {
        return None;
    }
    Some(s.to_owned())
}
fn exact_native_source(actual: &Path, adapter: &str) -> Result<(), SupervisorError> {
    let expected = normalized(actual.to_str().ok_or_else(denied)?).ok_or_else(denied)?;
    if normalized(adapter).as_deref() != Some(expected.as_str()) {
        return Err(denied());
    }
    Ok(())
}
fn bounded_sha(path: &Path, ceiling: u64) -> Result<String, SupervisorError> {
    if !path.is_absolute() { return Err(denied()); }
    let meta = fs::symlink_metadata(path).map_err(|_| denied())?;
    if !meta.is_file() || meta.file_type().is_symlink()
        || meta.file_attributes() & 0x400 != 0 || meta.len() < 4
        || meta.len() > ceiling {
        return Err(denied());
    }
    let mut f = File::open(path).map_err(|_| denied())?;
    let open_meta = f.metadata().map_err(|_| denied())?;
    if open_meta.len() != meta.len() { return Err(denied()); }
    let mut hash = Sha256::new();
    let mut b = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut b).map_err(|_| denied())?;
        if n == 0 { break; }
        total = total.checked_add(n as u64).ok_or_else(denied)?;
        if total > ceiling { return Err(denied()); }
        hash.update(&b[..n]);
    }
    let after = fs::symlink_metadata(path).map_err(|_| denied())?;
    if after.len() != meta.len() || after.modified().ok() != meta.modified().ok()
        || total != meta.len() || after.file_attributes() & 0x400 != 0 {
        return Err(denied());
    }
    Ok(format!("{:x}", hash.finalize()))
}
struct EphemeralOutput(std::path::PathBuf);
impl Drop for EphemeralOutput {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}
fn run_measured_reader(
    program: &Path, binary: &Path, load_bias: u64, stopped_rip: u64,
    local: &Path,
) -> Result<Value, SupervisorError> {
    let private = local.parent().ok_or_else(denied)?;
    let metadata = fs::symlink_metadata(private).map_err(|_| denied())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink()
        || metadata.file_attributes() & 0x400 != 0 {
        return Err(denied());
    }
    let output_path = private.join(format!(
        "dev045d4c-symbol-line-{:032x}.json", rand::random::<u128>()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let stdout = options.open(&output_path).map_err(|_| denied())?;
    let _cleanup = EphemeralOutput(output_path.clone());
    let address = format!("0x{stopped_rip:x}");
    let slide = format!("--adjust-vma=0x{load_bias:x}");
    let mut process = Command::new(program)
        .arg(format!("--obj={}", binary.display()))
        .arg("--output-style=JSON")
        .arg("--functions=none")
        .arg("--no-inlines")
        .arg(slide)
        .arg(address)
        .env_clear()
        .current_dir(private)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| denied())?;
    let started = Instant::now();
    let status = loop {
        match process.try_wait() {
            Ok(Some(exit)) => break exit,
            Ok(None) if started.elapsed() < MAX_ORACLE_RUNTIME => {
                thread::sleep(Duration::from_millis(10));
            }
            _ => {
                let _ = process.kill();
                let _ = process.wait();
                return Err(denied());
            }
        }
    };
    if !status.success() { return Err(denied()); }
    let size = fs::metadata(&output_path).map_err(|_| denied())?.len();
    if size == 0 || size > MAX_ORACLE_OUTPUT { return Err(denied()); }
    let data = fs::read(&output_path).map_err(|_| denied())?;
    serde_json::from_slice(&data).map_err(|_| denied())
}

#[derive(Clone, Debug)]
pub(crate) struct IndependentPdbLine {
    pub(crate) source_relative_path: String,
    pub(crate) source_sha256: String,
    pub(crate) line: u32,
    pub(crate) binary_sha256: String,
    pub(crate) pdb_sha256: String,
    pub(crate) oracle_bundle_sha256: String,
    pub(crate) os_thread_rip_matches_lldb_stack: bool,
    pub(crate) os_loaded_pe_mapping_verified: bool,
    pub(crate) independent_pe_pdb_source_line_verified: bool,
    // Caller still must authenticate the debugger owning the debug event.
    pub(crate) lldb_owned_kernel_debug_event_attested: bool,
    pub(crate) debugger_commands_authorized: bool,
    pub(crate) installed_app_accepted: bool,
}

fn equal_mapped_line(
    json: &Value, pc: u64, original_source: &Path,
    exact_binary: &Path, expected_line: u32,
) -> Result<(), SupervisorError> {
    let items = json.as_array().ok_or_else(denied)?;
    if items.len() != 1 { return Err(denied()); }
    let address = items[0].get("Address").and_then(Value::as_str).ok_or_else(denied)?;
    if address.to_ascii_lowercase() != format!("0x{pc:x}") {
        return Err(denied());
    }
    let module_name = items[0].get("ModuleName").and_then(Value::as_str)
        .ok_or_else(denied)?;
    let actual_name = lower_file_name(exact_binary).ok_or_else(denied)?;
    if normalized(module_name)
        .and_then(|n| n.split('\\').next_back().map(str::to_owned))
        != Some(actual_name) {
        return Err(denied());
    }
    let symbols = items[0].get("Symbol").and_then(Value::as_array)
        .ok_or_else(denied)?;
    if symbols.len() != 1 { return Err(denied()); }
    let name = symbols[0].get("FileName").and_then(Value::as_str).ok_or_else(denied)?;
    exact_native_source(original_source, name)?;
    let line = symbols[0].get("Line").and_then(Value::as_u64).ok_or_else(denied)?;
    if line != u64::from(expected_line) || line == 0 { return Err(denied()); }
    Ok(())
}

/// Only the trusted native Windows DAP session, never a WebView or adapter,
/// can call this with independently OS-verified Job + frame + sealed target.
pub(crate) fn independently_verify_pdb_line_for_ci(
    staged_dap_bundle: &Path, original_bundle_sha256: &str,
    target: &SealedProjectDebugTarget, native_source_root: &Path,
    module: &WindowsLoadedModuleWitness, thread: &WindowsThreadRipSample,
    candidate: &SourceLineCandidate,
) -> Result<IndependentPdbLine, SupervisorError> {
    if !thread.two_matching_samples || !thread.job_membership_rechecked
        || !module.job_member_verified || !module.os_loaded_pe_image_verified
        || !module.process_exe_path_verified || thread.process_id != module.pid
        || thread.observed_rip != module.observed_pc
        || candidate.instruction_address != Some(module.observed_pc)
        || candidate.line == 0 || thread.kernel_debug_event_independently_verified
        || thread.pdb_line_independently_verified || thread.debugger_control_authorized {
        return Err(denied());
    }
    if module.binary_sha256 != target.binary.binary_sha256
        || target.symbols.symbols_kind != "external_pdb_guid_age_dbi_match"
        || !target.symbols.debug_symbols_attested
        || target.symbols.source_line_mapping_verified
        || target.symbols.debugger_execution_authorized {
        return Err(denied());
    }
    let pdb_rel = target.symbols.symbols_relative_path.as_deref().ok_or_else(denied)?;
    if !Path::new(pdb_rel).components()
        .all(|c| matches!(c, Component::Normal(_))) { return Err(denied()); }
    target.verify()?; // structural RSDS ↔ MSF7 GUID/age/DBI + SHA
    let pe = target.root.join(target.confined_relative_binary());
    let pdb = target.root.join(pdb_rel);
    if bounded_sha(&pe, 64 * 1024 * 1024)? != target.binary.binary_sha256
        || bounded_sha(&pdb, 64 * 1024 * 1024)? != target.symbols.symbols_sha256 {
        return Err(denied());
    }
    // First accepted CI project has a fixed, one-file source snapshot.
    // Production must use a reviewed source-file identity from the host
    // project index, never guess a basename from an adapter response.
    let source_rel = "src/main.rs";
    let original_source = native_source_root.join(source_rel);
    exact_native_source(&original_source, &candidate.adapter_path)?;
    let source_sha256 = bounded_sha(&original_source, MAX_SOURCE_BYTES)?;
    let content = fs::read(&original_source).map_err(|_| denied())?;
    if candidate.line as usize > content.split(|b| *b == b'\n').count() {
        return Err(denied());
    }
    if module.loaded_base < module.preferred_base
        || module.observed_pc < module.loaded_base
        || module.preferred_pc != module.preferred_base
            .checked_add(module.observed_pc - module.loaded_base)
            .ok_or_else(denied)? {
        return Err(denied());
    }
    let tool = staged_dap_bundle.join("bin/llvm-symbolizer.exe");
    // The executable and all DLLs belong to the already reviewed, immutable
    // CI bundle bound to this separate one-use debugger plan SHA.
    let tree_sha = crate::windows_worker::lsp_native::native_lsp_bundle_digest(
        staged_dap_bundle, "dap_lldb_project",
    )?;
    if tree_sha != original_bundle_sha256
        || bounded_sha(&tool, 64 * 1024 * 1024)?.len() != 64 {
        return Err(denied());
    }
    let json = run_measured_reader(
        &tool, &pe, module.loaded_base - module.preferred_base,
        thread.observed_rip, staged_dap_bundle,
    )?;
    equal_mapped_line(&json, thread.observed_rip, &original_source, &pe, candidate.line)?;
    // Any mutation during the lookup is a denial, not an achieved mapping.
    target.verify()?;
    if bounded_sha(&original_source, MAX_SOURCE_BYTES)? != source_sha256
        || crate::windows_worker::lsp_native::native_lsp_bundle_digest(
            staged_dap_bundle, "dap_lldb_project",
        )? != original_bundle_sha256 {
        return Err(denied());
    }
    Ok(IndependentPdbLine {
        source_relative_path: source_rel.into(),
        source_sha256,
        line: candidate.line,
        binary_sha256: target.binary.binary_sha256.clone(),
        pdb_sha256: target.symbols.symbols_sha256.clone(),
        oracle_bundle_sha256: original_bundle_sha256.into(),
        os_thread_rip_matches_lldb_stack: true,
        os_loaded_pe_mapping_verified: true,
        independent_pe_pdb_source_line_verified: true,
        lldb_owned_kernel_debug_event_attested: false,
        debugger_commands_authorized: false,
        installed_app_accepted: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn rejects_forged_pdb_line_and_unknown_module() {
        let pe = Path::new(r"C:\sealed\target\debug\app.exe");
        let source = Path::new(r"C:\reviewed\src\main.rs");
        let good = json!([{"Address": "0x140001123", "ModuleName": "app.exe",
            "Symbol": [{"FileName": r"C:\reviewed\src\main.rs", "Line": 2}]}]);
        assert!(equal_mapped_line(&good, 0x140001123, source, pe, 2).is_ok());
        assert!(equal_mapped_line(&good, 0x140001124, source, pe, 2).is_err());
        assert!(equal_mapped_line(&good, 0x140001123, source, pe, 3).is_err());
        assert!(equal_mapped_line(&good, 0x140001123,
            Path::new(r"C:\elsewhere\src\main.rs"), pe, 2).is_err());
        let swapped = json!([{"Address": "0x140001123", "ModuleName": "foreign.exe",
            "Symbol": [{"FileName": r"C:\reviewed\src\main.rs", "Line": 2}]}]);
        assert!(equal_mapped_line(&swapped, 0x140001123, source, pe, 2).is_err());
    }
    #[test]
    fn source_identity_denies_unc_traversal_and_relative_paths() {
        let path = Path::new(r"C:\reviewed\src\main.rs");
        for fake in [r"C:\reviewed\..\src\main.rs",
                     r"\\server\share\src\main.rs",
                     r"C:src\main.rs",
                     ""] {
            assert!(exact_native_source(path, fake).is_err());
        }
    }
}
