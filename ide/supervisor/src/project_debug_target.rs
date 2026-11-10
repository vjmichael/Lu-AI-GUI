//! DEV-04.4: host-only, one-use project debug-target custody.
//! A protected Cargo build's terminal receipt and exact sealed output are
//! copied to an exclusive private target root. This does not start LLDB,
//! approve execution, or give the WebView a binary path or grant.
use crate::{
    debug_binary_witness::{audit_debug_binary_candidate, DebugBinaryWitness},
    debug_symbol_verification::{read_attested, verify_build_debug_symbols, DebugSymbolWitness},
    wo6_output_evidence::{audit_published_output, ArtifactEntry, OutputWitness},
    SupervisorError,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};
const MAX_BYTES: u64 = 128 * 1024 * 1024;
fn denied() -> SupervisorError {
    SupervisorError::NoContainment
}
fn sha(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
/// All fields originate inside native supervisor/app, never from a renderer.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedBuildLineage {
    pub project_id: String,
    pub build_job_id: String,
    pub build_plan_sha256: String,
    pub terminal_receipt_sha256: String,
    pub source_head: String,
    pub source_snapshot_sha256: String,
    pub manifest_sha256: String,
    pub build_toolchain_sha256: String,
    pub output_tree_sha256: String,
    pub build_successfully_completed: bool,
    pub requires_reconciliation: bool,
}
#[derive(Debug)]
pub struct SealedProjectDebugTarget {
    /// Native-private, non-WebView directory. Never serialize it.
    pub root: PathBuf,
    pub binding_sha256: String,
    pub lineage: CompletedBuildLineage,
    pub binary: DebugBinaryWitness,
    pub symbols: DebugSymbolWitness,
}
impl Drop for SealedProjectDebugTarget {
    fn drop(&mut self) {
        // Never retain a runnable-looking artifact after the last native
        // debugger/build owner releases it; no reauthorization on restart.
        let _ = fs::remove_dir_all(&self.root);
    }
}
impl SealedProjectDebugTarget {
    pub fn verify(&self) -> Result<(), SupervisorError> {
        if !sha(&self.binding_sha256, 64) {
            return Err(denied());
        }
        let observed = audit_published_output(&self.root)?;
        let binary = audit_debug_binary_candidate(&self.root, &observed)?.ok_or_else(denied)?;
        let symbols =
            verify_build_debug_symbols(&self.root, &observed, &binary)?.ok_or_else(denied)?;
        if binary.binary_sha256 != self.binary.binary_sha256
            || binary.relative_path != self.binary.relative_path
            || symbols.symbols_sha256 != self.symbols.symbols_sha256
            || symbols.symbols_kind != self.symbols.symbols_kind
            || symbols.symbols_relative_path != self.symbols.symbols_relative_path
            || !symbols.debug_symbols_attested
            || symbols.debugger_execution_authorized
            || self.binding_sha256 != binding_hash(&self.lineage, &self.binary, &self.symbols)?
        {
            return Err(denied());
        }
        Ok(())
    }
    pub fn confined_relative_binary(&self) -> &str {
        &self.binary.relative_path
    }
}
pub fn binding_hash(
    lineage: &CompletedBuildLineage,
    binary: &DebugBinaryWitness,
    symbols: &DebugSymbolWitness,
) -> Result<String, SupervisorError> {
    if !lineage.build_successfully_completed
        || lineage.requires_reconciliation
        || !lineage.project_id.starts_with("ide-")
        || lineage.project_id.len() != 36
        || !lineage.build_job_id.starts_with("job-")
        || lineage.build_job_id.len() != 36
        || !sha(&lineage.build_plan_sha256, 64)
        || !sha(&lineage.terminal_receipt_sha256, 64)
        || !sha(&lineage.source_head, 40)
        || !sha(&lineage.source_snapshot_sha256, 64)
        || !sha(&lineage.manifest_sha256, 64)
        || !sha(&lineage.build_toolchain_sha256, 64)
        || !sha(&lineage.output_tree_sha256, 64)
        || binary.output_tree_sha256 != lineage.output_tree_sha256
        || symbols.binary_sha256 != binary.binary_sha256
        || symbols.output_tree_sha256 != binary.output_tree_sha256
        || symbols.binary_relative_path != binary.relative_path
        || !symbols.debug_symbols_attested
        || symbols.source_line_mapping_verified
        || symbols.debugger_execution_authorized
    {
        return Err(denied());
    }
    let payload = serde_json::to_vec(&(
        "lu.ide.dev04.4.protected_project_debug_target.v1",
        lineage,
        binary.relative_path.as_str(),
        binary.binary_sha256.as_str(),
        symbols.symbols_sha256.as_str(),
        symbols.symbols_kind,
        symbols.symbols_relative_path.as_deref(),
    ))
    .map_err(|_| denied())?;
    Ok(format!("{:x}", Sha256::digest(payload)))
}
fn fixed_entry<'a>(
    witness: &'a OutputWitness,
    path: &str,
) -> Result<&'a ArtifactEntry, SupervisorError> {
    let mut matches = witness
        .files
        .iter()
        .filter(|entry| entry.relative_path == path);
    let one = matches.next().ok_or_else(denied)?;
    if matches.next().is_some() {
        return Err(denied());
    }
    Ok(one)
}
fn write_new(path: &Path, bytes: &[u8], executable: bool) -> Result<(), SupervisorError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file: File = options.open(path).map_err(|_| denied())?;
    file.write_all(bytes).map_err(|_| denied())?;
    file.sync_all().map_err(|_| denied())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if executable { 0o500 } else { 0o400 }),
        )
        .map_err(|_| denied())?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}
/// Called only for a real completed native protected BuildRunView with a
/// matching terminal receipt and independently reverified output tree.
pub fn seal_project_debug_target(
    output_root: &Path,
    private_parent: &Path,
    lineage: CompletedBuildLineage,
    expected_binary: &DebugBinaryWitness,
    expected_symbols: &DebugSymbolWitness,
) -> Result<SealedProjectDebugTarget, SupervisorError> {
    if !private_parent.is_absolute()
        || !private_parent.is_dir()
        || fs::symlink_metadata(private_parent)
            .map_err(|_| denied())?
            .file_type()
            .is_symlink()
    {
        return Err(denied());
    }
    let output = audit_published_output(output_root)?;
    if output.truncated || output.tree_sha256 != lineage.output_tree_sha256 {
        return Err(denied());
    }
    let binary = audit_debug_binary_candidate(output_root, &output)?.ok_or_else(denied)?;
    let symbols = verify_build_debug_symbols(output_root, &output, &binary)?.ok_or_else(denied)?;
    if binary.relative_path != expected_binary.relative_path
        || binary.binary_sha256 != expected_binary.binary_sha256
        || symbols.symbols_sha256 != expected_symbols.symbols_sha256
        || symbols.symbols_relative_path != expected_symbols.symbols_relative_path
    {
        return Err(denied());
    }
    // This hash is the D3 project debugger plan's target binding; all
    // inputs, including build terminal receipt, are included.
    let binding_sha256 = binding_hash(&lineage, &binary, &symbols)?;
    let nonce = format!("debug-target-{:032x}", rand::random::<u128>());
    let dest = private_parent.join(nonce);
    fs::create_dir(&dest).map_err(|_| denied())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o700)).map_err(|_| denied())?;
    }
    let result = (|| {
        let mut paths = vec![binary.relative_path.as_str()];
        if let Some(ref path) = symbols.symbols_relative_path {
            paths.push(path);
        }
        let mut total = 0u64;
        for path in paths {
            let rel = Path::new(path);
            if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
                return Err(denied());
            }
            let entry = fixed_entry(&output, path)?;
            total = total.checked_add(entry.bytes).ok_or_else(denied)?;
            if total > MAX_BYTES {
                return Err(denied());
            }
            let data = read_attested(output_root, entry, &output)?;
            let out = dest.join(rel);
            let parent = out.parent().ok_or_else(denied)?;
            fs::create_dir_all(parent).map_err(|_| denied())?;
            write_new(&out, &data, path == binary.relative_path)?;
        }
        let sealed = SealedProjectDebugTarget {
            root: dest.clone(),
            binding_sha256,
            lineage,
            binary,
            symbols,
        };
        sealed.verify()?; // independent rehash of copied image + symbols
        Ok(sealed)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(dest);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn specimen() -> CompletedBuildLineage {
        CompletedBuildLineage {
            project_id: format!("ide-{}", "a".repeat(32)),
            build_job_id: format!("job-{}", "b".repeat(32)),
            build_plan_sha256: "c".repeat(64),
            terminal_receipt_sha256: "d".repeat(64),
            source_head: "e".repeat(40),
            source_snapshot_sha256: "f".repeat(64),
            manifest_sha256: "1".repeat(64),
            build_toolchain_sha256: "2".repeat(64),
            output_tree_sha256: "3".repeat(64),
            build_successfully_completed: true,
            requires_reconciliation: false,
        }
    }
    #[test]
    fn target_binding_fails_closed_on_bad_receipts_and_denied_execution() {
        let mut lineage = specimen();
        let binary = DebugBinaryWitness {
            relative_path: "target/debug/example".into(),
            bytes: 10,
            binary_sha256: "4".repeat(64),
            output_tree_sha256: lineage.output_tree_sha256.clone(),
            format: "elf_candidate_magic_only",
            role: "post_termination_unique_cargo_binary_readback_not_launch_authority",
            debug_symbols_attested: false,
            debugger_execution_authorized: false,
            installed_app_accepted: false,
        };
        let symbols = DebugSymbolWitness {
            binary_relative_path: binary.relative_path.clone(),
            binary_sha256: binary.binary_sha256.clone(),
            output_tree_sha256: binary.output_tree_sha256.clone(),
            executable_format: "elf64_x86_64_executable_dwarf_sections",
            symbols_kind: "embedded_dwarf_info_abbrev_line",
            symbols_relative_path: None,
            symbols_sha256: "5".repeat(64),
            validation_scope: "structural_only",
            role: "structural_binary_and_debug_information_only_no_debugger_authority",
            debug_symbols_attested: true,
            source_line_mapping_verified: false,
            debugger_execution_authorized: false,
            installed_app_accepted: false,
        };
        let a = binding_hash(&lineage, &binary, &symbols).unwrap();
        lineage.terminal_receipt_sha256 = "6".repeat(64);
        let b = binding_hash(&lineage, &binary, &symbols).unwrap();
        assert_ne!(a, b);
        lineage.requires_reconciliation = true;
        assert!(binding_hash(&lineage, &binary, &symbols).is_err());
    }
}
