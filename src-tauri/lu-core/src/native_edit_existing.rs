//! WRITE-01E: host-scoped, journal-dispatched existing-text edit and rollback.
//!
//! Exactly one already-opened directory is supplied by the host. No model,
//! UI pathname, extension, or approval digest confers authority by itself.
//! All filesystem effects run only after DurableDispatcher's EXECUTING fence.
//! Recovery is observation only: an uncertain effect is NEVER replayed.
//!
//! Compatibility caveat: replacement may not preserve ACLs, alternate data
//! streams, xattrs or hardlink identities. Windows native metadata/directory
//! flush and hostile cross-process race acceptance remain release gates.
use std::{io::{Read, Write}, path::Path, sync::Arc};
use cap_std::fs::{Dir, OpenOptions};
use prost::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use crate::{
    contracts::{AuthorizationError, ModuleKind, ModuleManifest, NativeTool,
        ProtoPayload, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolFailure,
        ToolFuture, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION},
    edit_safety::{plan_text_edit, verify_reviewed_edit, ReviewedEdit, TextEditPlan,
        MAX_EDIT_DOCUMENT_BYTES},
    registry::{NativeToolRegistry, RegistryError},
    wire::proto,
};

pub const EDIT_TOOL_ID: &str = "lu.edit_existing.commit";
pub const ROLLBACK_TOOL_ID: &str = "lu.edit_existing.rollback";
pub const EDIT_REQUEST_TYPE: &str = "type.googleapis.com/lu.v1.EditExistingRequest";
pub const EDIT_RESULT_TYPE: &str = "type.googleapis.com/lu.v1.EditExistingResult";
pub const ROLLBACK_REQUEST_TYPE: &str = "type.googleapis.com/lu.v1.EditRollbackRequest";
pub const ROLLBACK_RESULT_TYPE: &str = "type.googleapis.com/lu.v1.EditRollbackResult";
const MAX_MANIFEST_BYTES: u64 = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditCommitError {
    InvalidTarget,
    InvalidOperationId,
    InvalidDigest,
    InvalidContent,
    UnsafeTarget,
    SourceConflict,
    MissingTarget,
    ArtifactCollision,
    InvalidRecoveryRecord,
    Io(String),
    /// Some artifacts or a publication might have happened; never auto-retry.
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditReceipt {
    pub backup_id: String,
    pub original_sha256: String,
    pub replacement_sha256: String,
    pub bytes_written: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditObservation {
    NoManifest,
    OriginalAndBackupVerified,
    ReplacementAndBackupVerified,
    MissingTarget,
    ChangedOrDamaged,
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EditManifest {
    schema: u32,
    edit_id: String,
    target: String,
    original_sha256: String,
    replacement_sha256: String,
    approval_sha256: String,
}

fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn valid_sha(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_id(value: &str) -> bool {
    (16..=128).contains(&value.len()) &&
        value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn filename(name: &str) -> bool {
    !name.is_empty() && name.len() <= 200 &&
        !name.starts_with(".lu-") && !name.ends_with('.') && !name.ends_with(' ') &&
        name != "." && name != ".." &&
        !name.chars().any(|c| c.is_control() || matches!(c, '/' | '\\' | ':')) &&
        !Path::new(name).is_absolute()
}
fn sidecar(id: &str, suffix: &str) -> String { format!(".lu-edit-{id}.{suffix}") }
fn io(e: std::io::Error) -> EditCommitError { EditCommitError::Io(e.to_string()) }

fn sync_parent(dir: &Dir) -> Result<(), EditCommitError> {
    #[cfg(unix)]
    {
        dir.try_clone().map_err(io)?.into_std_file().sync_all().map_err(io)?;
    }
    // Windows directory FlushFileBuffers requires special native directory
    // handles. Synced files and the WAL journal do not constitute an NTFS
    // power-failure guarantee; defer that guarantee to Windows acceptance.
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

fn regular_bytes(
    root: &Dir, name: &str
) -> Result<(Vec<u8>, std::fs::Permissions), EditCommitError> {
    let name_meta = root.symlink_metadata(name).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            EditCommitError::MissingTarget
        } else { io(e) }
    })?;
    if !name_meta.is_file() || name_meta.file_type().is_symlink() {
        return Err(EditCommitError::UnsafeTarget);
    }
    let mut file = root.open(name).map_err(io)?.into_std();
    let meta = file.metadata().map_err(io)?;
    if !meta.is_file() || meta.len() > MAX_EDIT_DOCUMENT_BYTES as u64 {
        return Err(EditCommitError::UnsafeTarget);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Never silently sever other hardlinks to the original inode.
        if meta.nlink() != 1 { return Err(EditCommitError::UnsafeTarget); }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(EditCommitError::UnsafeTarget);
        }
    }
    let perms = meta.permissions();
    let mut bytes = Vec::new();
    (&mut file).take((MAX_EDIT_DOCUMENT_BYTES as u64) + 1)
        .read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() > MAX_EDIT_DOCUMENT_BYTES {
        return Err(EditCommitError::InvalidContent);
    }
    if root.symlink_metadata(name).map_err(io)?.file_type().is_symlink() {
        return Err(EditCommitError::UnsafeTarget);
    }
    Ok((bytes, perms))
}

fn write_new_synced(
    root: &Dir, name: &str, bytes: &[u8], permissions: &std::fs::Permissions
) -> Result<(), EditCommitError> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    let file = root.open_with(name, &opts).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            EditCommitError::ArtifactCollision
        } else { io(e) }
    })?;
    let mut file = file.into_std();
    file.write_all(bytes).map_err(io)?;
    file.set_permissions(permissions.clone()).map_err(io)?;
    file.sync_all().map_err(io)?;
    drop(file);
    sync_parent(root)
}

fn read_manifest(root: &Dir, id: &str) -> Result<EditManifest, EditCommitError> {
    if !valid_id(id) { return Err(EditCommitError::InvalidOperationId); }
    let path = sidecar(id, "meta");
    let meta = root.symlink_metadata(&path).map_err(io)?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_MANIFEST_BYTES {
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    let mut data = Vec::new();
    root.open(&path).map_err(io)?.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut data).map_err(io)?;
    let manifest: EditManifest = serde_json::from_slice(&data)
        .map_err(|_| EditCommitError::InvalidRecoveryRecord)?;
    if manifest.schema != 1 || manifest.edit_id != id ||
        !filename(&manifest.target) || !valid_sha(&manifest.original_sha256) ||
        !valid_sha(&manifest.replacement_sha256) || !valid_sha(&manifest.approval_sha256) {
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    Ok(manifest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopPoint { Never, AfterManifest, AfterBackup, AfterStage, AfterPublish }

/// Actual mutation primitive. Caller MUST have entered EffectJournal::Executing
/// via dispatch_bound_durable and must supply host-approved exact bytes.
fn commit_in_dir(
    root: &Dir,
    target: &str,
    operation_id: &str,
    baseline_sha: &str,
    replacement: &[u8],
    approval_sha: &str,
    stop: StopPoint,
) -> Result<EditReceipt, EditCommitError> {
    if !filename(target) { return Err(EditCommitError::InvalidTarget); }
    if !valid_id(operation_id) { return Err(EditCommitError::InvalidOperationId); }
    if !valid_sha(baseline_sha) || !valid_sha(approval_sha) {
        return Err(EditCommitError::InvalidDigest);
    }
    if replacement.len() > MAX_EDIT_DOCUMENT_BYTES {
        return Err(EditCommitError::InvalidContent);
    }
    let (original, perms) = regular_bytes(root, target)?;
    if sha(&original) != baseline_sha { return Err(EditCommitError::SourceConflict); }
    plan_text_edit(target, &original, baseline_sha, replacement)
        .map_err(|_| EditCommitError::InvalidContent)?;
    let manifest = EditManifest {
        schema: 1, edit_id: operation_id.into(), target: target.into(),
        original_sha256: baseline_sha.into(),
        replacement_sha256: sha(replacement),
        approval_sha256: approval_sha.into(),
    };
    let manifest_data = serde_json::to_vec(&manifest)
        .map_err(|_| EditCommitError::InvalidRecoveryRecord)?;
    let manifest_name = sidecar(operation_id, "meta");
    let backup_name = sidecar(operation_id, "orig");
    let stage_name = sidecar(operation_id, "new");
    // First durable on-disk breadcrumb. New operation IDs can NEVER overwrite
    // artifacts from an old interrupted or successful transaction.
    write_new_synced(root, &manifest_name, &manifest_data, &perms)?;
    if stop == StopPoint::AfterManifest { return Err(EditCommitError::Uncertain); }
    write_new_synced(root, &backup_name, &original, &perms)?;
    if sha(&regular_bytes(root, &backup_name)?.0) != baseline_sha {
        return Err(EditCommitError::Uncertain);
    }
    if stop == StopPoint::AfterBackup { return Err(EditCommitError::Uncertain); }
    write_new_synced(root, &stage_name, replacement, &perms)?;
    if sha(&regular_bytes(root, &stage_name)?.0) != manifest.replacement_sha256 {
        return Err(EditCommitError::Uncertain);
    }
    if stop == StopPoint::AfterStage { return Err(EditCommitError::Uncertain); }

    // Bounded last recheck. A non-cooperating external writer may still race
    // between this read and rename; backup custody retains the old bytes.
    let (latest, _) = regular_bytes(root, target)?;
    if sha(&latest) != baseline_sha {
        return Err(EditCommitError::SourceConflict);
    }
    root.rename(&stage_name, root, target).map_err(io)?;
    if stop == StopPoint::AfterPublish { return Err(EditCommitError::Uncertain); }
    sync_parent(root).map_err(|_| EditCommitError::Uncertain)?;
    let published = regular_bytes(root, target).map_err(|_| EditCommitError::Uncertain)?.0;
    if sha(&published) != manifest.replacement_sha256 {
        return Err(EditCommitError::Uncertain);
    }
    Ok(EditReceipt {
        backup_id: operation_id.into(), original_sha256: baseline_sha.into(),
        replacement_sha256: manifest.replacement_sha256,
        bytes_written: replacement.len() as u64,
    })
}

/// READ-ONLY observation for recovery. This does not update the effect journal
/// or classify a successful publication as an authorized completed effect.
pub fn observe_edit(root: &Dir, operation_id: &str) -> EditObservation {
    if !valid_id(operation_id) { return EditObservation::Unavailable; }
    let meta_name = sidecar(operation_id, "meta");
    match root.symlink_metadata(&meta_name) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return EditObservation::NoManifest,
        Err(_) => return EditObservation::Unavailable,
        Ok(_) => {}
    }
    let Ok(manifest) = read_manifest(root, operation_id) else {
        return EditObservation::ChangedOrDamaged;
    };
    let backup = regular_bytes(root, &sidecar(operation_id, "orig"));
    let Ok((backup, _)) = backup else { return EditObservation::ChangedOrDamaged; };
    if sha(&backup) != manifest.original_sha256 {
        return EditObservation::ChangedOrDamaged;
    }
    match regular_bytes(root, &manifest.target) {
        Ok((bytes, _)) if sha(&bytes) == manifest.replacement_sha256 =>
            EditObservation::ReplacementAndBackupVerified,
        Ok((bytes, _)) if sha(&bytes) == manifest.original_sha256 =>
            EditObservation::OriginalAndBackupVerified,
        Err(EditCommitError::MissingTarget) => EditObservation::MissingTarget,
        Ok(_) => EditObservation::ChangedOrDamaged,
        Err(_) => EditObservation::Unavailable,
    }
}

fn rollback_in_dir(
    root: &Dir, target: &str, original_edit_id: &str, rollback_id: &str,
    expected_current: &str, expected_restored: &str
) -> Result<u64, EditCommitError> {
    if !filename(target) || !valid_id(original_edit_id) || !valid_id(rollback_id) ||
        !valid_sha(expected_current) || !valid_sha(expected_restored) ||
        original_edit_id == rollback_id {
        return Err(EditCommitError::InvalidTarget);
    }
    let manifest = read_manifest(root, original_edit_id)?;
    if manifest.target != target || manifest.original_sha256 != expected_restored ||
        manifest.replacement_sha256 != expected_current {
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    let (backup, _) = regular_bytes(root, &sidecar(original_edit_id, "orig"))?;
    if sha(&backup) != expected_restored {
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    let (current, perms) = regular_bytes(root, target)?;
    if sha(&current) != expected_current {
        return Err(EditCommitError::SourceConflict);
    }
    let stage = sidecar(rollback_id, "rollback");
    write_new_synced(root, &stage, &backup, &perms)?;
    if sha(&regular_bytes(root, &stage)?.0) != expected_restored {
        return Err(EditCommitError::Uncertain);
    }
    if sha(&regular_bytes(root, target)?.0) != expected_current {
        return Err(EditCommitError::SourceConflict);
    }
    root.rename(&stage, root, target).map_err(io)?;
    sync_parent(root).map_err(|_| EditCommitError::Uncertain)?;
    if sha(&regular_bytes(root, target).map_err(|_| EditCommitError::Uncertain)?.0)
        != expected_restored {
        return Err(EditCommitError::Uncertain);
    }
    Ok(backup.len() as u64)
}

/// Host-created exact per-call fence. A digest copied out of model text cannot
/// satisfy this guard without the separate, authenticated human task grant.
pub struct ExactEditPolicy<'a> {
    pub host_policy: &'a dyn ToolAuthorizer,
    pub plan: &'a TextEditPlan,
    pub reviewed: &'a ReviewedEdit,
    pub approved_original: &'a [u8],
    pub approved_replacement: &'a [u8],
}
impl ToolAuthorizer for ExactEditPolicy<'_> {
    fn authorize(&self, call: &ToolInvocation, desc: &ToolDescriptor)
        -> Result<(), AuthorizationError> {
        if desc.id != EDIT_TOOL_ID || desc.effect != ToolEffect::WriteWorkspace ||
            call.tool_id != EDIT_TOOL_ID || call.input.type_url != EDIT_REQUEST_TYPE ||
            call.input.bytes.len() > MAX_EDIT_DOCUMENT_BYTES + 8192 {
            return Err(AuthorizationError::new("edit contract denied"));
        }
        let req = proto::EditExistingRequest::decode(call.input.bytes.as_slice())
            .map_err(|_| AuthorizationError::new("malformed edit protobuf"))?;
        if !filename(&req.relative_path) || req.relative_path != self.plan.filename ||
            req.relative_path != self.reviewed.host_relative_path.rsplit('/').next().unwrap_or("") ||
            req.expected_original_sha256 != self.plan.expected_original_sha256 ||
            req.replacement_sha256() != self.plan.replacement_sha256 ||
            req.replacement.as_slice() != self.approved_replacement ||
            req.reviewed_approval_sha256 != self.reviewed.approval_sha256 ||
            verify_reviewed_edit(self.plan, self.reviewed, &self.reviewed.host_relative_path,
                self.approved_original, self.approved_replacement,
                &req.reviewed_approval_sha256).is_err() {
            return Err(AuthorizationError::new("edited target, bytes or review do not match trusted approval"));
        }
        self.host_policy.authorize(call, desc)
    }
}
// Local helper avoids duplicating expected hash comparisons in the policy.
trait ReplacementDigest {
    fn replacement_sha256(&self) -> String;
}
impl ReplacementDigest for proto::EditExistingRequest {
    fn replacement_sha256(&self) -> String { sha(&self.replacement) }
}

pub struct ExactRollbackPolicy<'a> {
    pub host_policy: &'a dyn ToolAuthorizer,
    pub expected_target: &'a str,
    pub expected_original_edit_id: &'a str,
    pub expected_current_sha256: &'a str,
    pub expected_restored_sha256: &'a str,
}
impl ToolAuthorizer for ExactRollbackPolicy<'_> {
    fn authorize(&self, call: &ToolInvocation, desc: &ToolDescriptor)
        -> Result<(), AuthorizationError> {
        if desc.id != ROLLBACK_TOOL_ID || desc.effect != ToolEffect::WriteWorkspace ||
            call.tool_id != ROLLBACK_TOOL_ID || call.input.type_url != ROLLBACK_REQUEST_TYPE ||
            call.input.bytes.len() > 8192 {
            return Err(AuthorizationError::new("rollback contract denied"));
        }
        let req = proto::EditRollbackRequest::decode(call.input.bytes.as_slice())
            .map_err(|_| AuthorizationError::new("malformed rollback protobuf"))?;
        if !filename(&req.relative_path) || !valid_id(&req.original_edit_id) ||
            req.relative_path != self.expected_target ||
            req.original_edit_id != self.expected_original_edit_id ||
            req.expected_current_sha256 != self.expected_current_sha256 ||
            req.expected_restored_sha256 != self.expected_restored_sha256 {
            return Err(AuthorizationError::new("rollback differs from explicit trusted approval"));
        }
        self.host_policy.authorize(call, desc)
    }
}

struct EditTool { descriptor: ToolDescriptor, root: Arc<Dir>, rollback: bool }
impl NativeTool for EditTool {
    fn descriptor(&self) -> &ToolDescriptor { &self.descriptor }
    fn invoke(&self, call: ToolInvocation) -> ToolFuture<'_> {
        let root = Arc::clone(&self.root);
        let rollback = self.rollback;
        Box::pin(async move {
            tokio::task::spawn_blocking(move || -> Result<ToolOutput, ToolFailure> {
                let (type_url, reply) = if rollback {
                    let req = proto::EditRollbackRequest::decode(call.input.bytes.as_slice())
                        .map_err(|_| tool_failure("BAD_PROTOBUF"))?;
                    let len = rollback_in_dir(&root, &req.relative_path, &req.original_edit_id,
                        &call.call_id, &req.expected_current_sha256, &req.expected_restored_sha256)
                        .map_err(|_| tool_failure("ROLLBACK_UNCERTAIN"))?;
                    let result = proto::EditRollbackResult {
                        original_edit_id: req.original_edit_id, restored_bytes: len,
                        restored_sha256: req.expected_restored_sha256, committed: true
                    };
                    (ROLLBACK_RESULT_TYPE, result.encode_to_vec())
                } else {
                    let req = proto::EditExistingRequest::decode(call.input.bytes.as_slice())
                        .map_err(|_| tool_failure("BAD_PROTOBUF"))?;
                    let receipt = commit_in_dir(&root, &req.relative_path, &call.call_id,
                        &req.expected_original_sha256, &req.replacement,
                        &req.reviewed_approval_sha256, StopPoint::Never)
                        .map_err(|_| tool_failure("EDIT_UNCERTAIN"))?;
                    let result = proto::EditExistingResult {
                        backup_id: receipt.backup_id,
                        bytes_written: receipt.bytes_written,
                        original_sha256: receipt.original_sha256,
                        replacement_sha256: receipt.replacement_sha256,
                        committed: true,
                    };
                    (EDIT_RESULT_TYPE, result.encode_to_vec())
                };
                Ok(ToolOutput {
                    payload: ProtoPayload { type_url: type_url.into(), bytes: reply },
                })
            }).await.map_err(|_| tool_failure("WORKER_UNCERTAIN"))?
        })
    }
}
fn tool_failure(code: &str) -> ToolFailure {
    ToolFailure::new(code, "file edit or rollback unconfirmed; reconcile, never retry automatically", false)
}

/// Register with a directory pinned by a real trusted host picker/selection.
/// This does NOT grant permission; ConversationAuthority and Exact*Policy do.
pub fn register_edit_tools_pinned(
    registry: &NativeToolRegistry, root: Arc<Dir>
) -> Result<(), RegistryError> {
    let descriptor = |id: &str, name: &str, input: &str, output: &str| ToolDescriptor {
        id: id.into(), display_name: name.into(),
        effect: ToolEffect::WriteWorkspace,
        input_type_url: input.into(), output_type_url: output.into(),
    };
    let edit = descriptor(EDIT_TOOL_ID, "Replace existing approved UTF-8 text",
        EDIT_REQUEST_TYPE, EDIT_RESULT_TYPE);
    let rollback = descriptor(ROLLBACK_TOOL_ID, "Restore exact prior bytes with new approval",
        ROLLBACK_REQUEST_TYPE, ROLLBACK_RESULT_TYPE);
    registry.register(
        ModuleManifest {
            id: "lu.edit_existing".into(), display_name: "Lu journaled text edits".into(),
            contract_version: CORE_CONTRACT_VERSION, kind: ModuleKind::Core,
            tools: vec![edit.clone(), rollback.clone()], panels: vec![], workflows: vec![],
        },
        vec![
            Arc::new(EditTool { descriptor: edit, root: Arc::clone(&root), rollback: false }),
            Arc::new(EditTool { descriptor: rollback, root, rollback: true }),
        ]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::atomic::{AtomicUsize, Ordering}};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn temp() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "lu-edit-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn run_at(point: StopPoint, id: &str) -> (EditObservation, Vec<u8>) {
        let path = temp();
        fs::write(path.join("t.json"), b"{\"a\":1}").unwrap();
        let root = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
        let result = commit_in_dir(&root, "t.json", id, &sha(b"{\"a\":1}"), b"{\"a\":2}",
            &sha(b"approved"), point);
        if point == StopPoint::Never { assert!(result.is_ok()); }
        else { assert!(result.is_err()); }
        let observation = observe_edit(&root, id);
        let bytes = fs::read(path.join("t.json")).unwrap();
        drop(root);
        fs::remove_dir_all(path).unwrap();
        (observation, bytes)
    }

    #[test]
    fn crash_barriers_keep_old_or_new_and_never_destroy_verified_original() {
        let cases = [
            (StopPoint::AfterManifest, EditObservation::ChangedOrDamaged, b"{\"a\":1}".as_slice()),
            (StopPoint::AfterBackup, EditObservation::OriginalAndBackupVerified, b"{\"a\":1}".as_slice()),
            (StopPoint::AfterStage, EditObservation::OriginalAndBackupVerified, b"{\"a\":1}".as_slice()),
            (StopPoint::AfterPublish, EditObservation::ReplacementAndBackupVerified, b"{\"a\":2}".as_slice()),
            (StopPoint::Never, EditObservation::ReplacementAndBackupVerified, b"{\"a\":2}".as_slice()),
        ];
        for (i, (point, state, expected)) in cases.into_iter().enumerate() {
            let (actual, bytes) = run_at(point, &format!("g-edit-crash-case-{i:04}"));
            assert_eq!(actual, state);
            assert_eq!(bytes, expected);
        }
    }
    #[test]
    fn source_conflict_and_invalid_json_cannot_publish() {
        let path = temp();
        fs::write(path.join("t.json"), b"{\"a\":1}").unwrap();
        let root = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
        assert_eq!(
            commit_in_dir(&root, "t.json", "g-edit-conflict-0001", &sha(b"stale"),
                b"{\"a\":2}", &sha(b"ok"), StopPoint::Never),
            Err(EditCommitError::SourceConflict)
        );
        assert_eq!(
            commit_in_dir(&root, "t.json", "g-edit-invalid-00001", &sha(b"{\"a\":1}"),
                b"{broken", &sha(b"ok"), StopPoint::Never),
            Err(EditCommitError::InvalidContent)
        );
        assert_eq!(fs::read(path.join("t.json")).unwrap(), b"{\"a\":1}");
        drop(root);
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn approved_rollback_restores_only_when_current_matches_and_backup_is_intact() {
        let path = temp();
        fs::write(path.join("t.txt"), b"old").unwrap();
        let root = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
        let receipt = commit_in_dir(&root, "t.txt", "g-edit-first-change-0001",
            &sha(b"old"), b"new", &sha(b"ok"), StopPoint::Never).unwrap();
        assert_eq!(receipt.bytes_written, 3);
        assert_eq!(rollback_in_dir(&root, "t.txt", "g-edit-first-change-0001",
            "g-edit-rollback-000001", &sha(b"new"), &sha(b"old")).unwrap(), 3);
        assert_eq!(fs::read(path.join("t.txt")).unwrap(), b"old");
        assert_eq!(rollback_in_dir(&root, "t.txt", "g-edit-first-change-0001",
            "g-edit-rollback-000002", &sha(b"new"), &sha(b"old")),
            Err(EditCommitError::SourceConflict));
        drop(root);
        fs::remove_dir_all(path).unwrap();
    }
}
