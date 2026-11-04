//! WRITE-01E: host-scoped, journal-dispatched existing-text edit and rollback.
//!
//! Exactly one already-opened directory is supplied by the host. No model,
//! UI pathname, extension, or approval digest confers authority by itself.
//! All filesystem effects run only after DurableDispatcher's EXECUTING fence.
//! Recovery is observation only: an uncertain effect is NEVER replayed.
//!
//! Windows now requires owner/group/DACL equality and explicit creation-time
//! custody before publishing. This is not comprehensive metadata preservation:
//! SACLs, EAs, object IDs, directory flush and hostile cross-process races
//! remain release gates. Named streams and multiple hardlinks are refused.
use std::{io::{Read, Write}, path::Path, sync::{Arc, Mutex}};
// Process-local serialization across ALL registrations/roots. This is not an
// operating-system lock against a non-cooperating external editor.
static EDIT_TRANSACTION_GATE: Mutex<()> = Mutex::new(());
use cap_std::fs::{Dir, OpenOptions};
use prost::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use crate::{
    contracts::{AuthorizationError, ModuleKind, ModuleManifest, NativeTool,
        ProtoPayload, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolFailure,
        ToolFuture, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION},
    journal::{EffectRecord, EffectState},
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
    /// Hash of Win32 owner/group/DACL plus creation time, not raw ACL data.
    /// Legacy manifests without this evidence are not promoted on Windows.
    #[serde(default,skip_serializing_if="Option::is_none")]
    native_metadata_sha256: Option<String>,
}

fn sha(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn valid_sha(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
/// Windows recovery must have native metadata evidence. A legacy Windows
/// manifest without the witness remains available for manual custody review,
/// but cannot be automatically classified as verified.
fn valid_native_metadata_sha(value:Option<&str>)->bool {
    #[cfg(windows)]
    {value.is_some_and(valid_sha)}
    #[cfg(not(windows))]
    {value.is_none()}
}
#[cfg(windows)]
fn native_metadata_sha(file:&std::fs::File)->Result<String,EditCommitError> {
    let metadata=crate::windows_file_guard::snapshot_metadata(file)
        .map_err(|_|EditCommitError::UnsafeTarget)?;
    Ok(sha(&metadata.immutable_identity_bytes()))
}
#[cfg(windows)]
fn matches_native_metadata(root:&Dir,target:&str,digest:&str)->bool {
    let Ok(file)=root.open(target) else {return false;};
    native_metadata_sha(&file.into_std()).is_ok_and(|sha|sha==digest)
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
        // cap-std may hold an O_PATH directory descriptor on Linux; fsync
        // on that descriptor fails EBADF. Open a readable directory handle
        // relative to the pinned capability instead of syncing O_PATH.
        dir.open(".").map_err(io)?.sync_all().map_err(io)?;
    }
    // Windows directory FlushFileBuffers requires special native directory
    // handles. Synced files and the WAL journal do not constitute an NTFS
    // power-failure guarantee; defer that guarantee to Windows acceptance.
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

#[cfg(windows)]
fn strict_windows_file_identity(
    root:&Dir,name:&str
)->Result<crate::windows_file_guard::WindowsFileIdentity,EditCommitError> {
    let handle=root.open(name).map_err(io)?.into_std();
    crate::windows_file_guard::inspect_open_file(&handle)
        .map_err(|_|EditCommitError::UnsafeTarget)
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
    #[cfg(windows)]
    let opened_identity=crate::windows_file_guard::inspect_open_file(&file)
        .map_err(|_|EditCommitError::UnsafeTarget)?;
    if !meta.is_file() || meta.len() > MAX_EDIT_DOCUMENT_BYTES as u64 {
        return Err(EditCommitError::UnsafeTarget);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // Never silently sever other hardlinks to the original inode.
        if meta.nlink() != 1 { return Err(EditCommitError::UnsafeTarget); }
    }
    // The opened-handle guard above also refuses Windows reparse points,
    // multiple hardlinks, unsupported attributes, named data streams and
    // missing native file IDs. Do not silently strip these on replacement.
    let perms = meta.permissions();
    let mut bytes = Vec::new();
    (&mut file).take((MAX_EDIT_DOCUMENT_BYTES as u64) + 1)
        .read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() > MAX_EDIT_DOCUMENT_BYTES {
        return Err(EditCommitError::InvalidContent);
    }
    let after_name_meta=root.symlink_metadata(name).map_err(io)?;
    if !after_name_meta.is_file() || after_name_meta.file_type().is_symlink() {
        return Err(EditCommitError::UnsafeTarget);
    }
    #[cfg(windows)]
    if strict_windows_file_identity(root,name)?!=opened_identity {
        return Err(EditCommitError::SourceConflict);
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
    // Treat sidecars as untrusted filesystem entries too. Plain open/read
    // could otherwise follow an unsupported Windows reparse point or ignore
    // a modified stream/link state on a previously journaled artifact.
    let (data, _)=regular_bytes(root,&path)
        .map_err(|_|EditCommitError::InvalidRecoveryRecord)?;
    if data.len()>MAX_MANIFEST_BYTES as usize {
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    let manifest: EditManifest = serde_json::from_slice(&data)
        .map_err(|_| EditCommitError::InvalidRecoveryRecord)?;
    if manifest.schema != 1 || manifest.edit_id != id ||
        !filename(&manifest.target) || !valid_sha(&manifest.original_sha256) ||
        !valid_sha(&manifest.replacement_sha256) || !valid_sha(&manifest.approval_sha256) ||
        !valid_native_metadata_sha(manifest.native_metadata_sha256.as_deref()) {
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    Ok(manifest)
}

/// A separate prepublication witness ties the final native FileId to the
/// exact bytes/metadata that were staged. This is written and synced BEFORE
/// rename, so read-only recovery can reject same-byte identity substitutions.
#[cfg(windows)]
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct NativePublishSeal {
    schema:u32,
    operation_id:String,
    target:String,
    target_sha256:String,
    native_file_id:String,
    native_metadata_sha256:String,
}
#[cfg(windows)]
fn valid_file_id_token(value:&str)->bool {
    value.len()==48 && value.bytes().all(|b|
        b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[cfg(windows)]
fn write_publish_seal(
    root:&Dir,operation_id:&str,target:&str,stage:&str,
    expected_sha:&str,expected_metadata_sha:&str,
    perms:&std::fs::Permissions
)->Result<(),EditCommitError>{
    let file=root.open(stage).map_err(io)?.into_std();
    let id=crate::windows_file_guard::inspect_open_file(&file)
        .map_err(|_|EditCommitError::UnsafeTarget)?.token();
    if native_metadata_sha(&file)?!=expected_metadata_sha ||
        sha(&regular_bytes(root,stage)?.0)!=expected_sha {
        return Err(EditCommitError::Uncertain);
    }
    let record=NativePublishSeal{
        schema:1,operation_id:operation_id.into(),target:target.into(),
        target_sha256:expected_sha.into(),native_file_id:id,
        native_metadata_sha256:expected_metadata_sha.into(),
    };
    let bytes=serde_json::to_vec(&record)
        .map_err(|_|EditCommitError::InvalidRecoveryRecord)?;
    write_new_synced(root,&sidecar(operation_id,"seal"),&bytes,perms)?;
    Ok(())
}
#[cfg(windows)]
fn matches_publish_seal(
    root:&Dir,operation_id:&str,target:&str,expected_sha:&str,
    expected_metadata_sha:&str
)->bool{
    let Ok((bytes,_))=regular_bytes(root,&sidecar(operation_id,"seal")) else {
        return false;
    };
    if bytes.len()>MAX_MANIFEST_BYTES as usize {return false;}
    let Ok(seal)=serde_json::from_slice::<NativePublishSeal>(&bytes) else {
        return false;
    };
    if seal.schema!=1 ||seal.operation_id!=operation_id ||
        seal.target!=target ||seal.target_sha256!=expected_sha ||
        seal.native_metadata_sha256!=expected_metadata_sha ||
        !valid_file_id_token(&seal.native_file_id) ||
        !valid_sha(&seal.target_sha256) ||
        !valid_sha(&seal.native_metadata_sha256) {
        return false;
    }
    let Ok(file)=root.open(target) else {return false;};
    let file=file.into_std();
    let Ok(id)=crate::windows_file_guard::inspect_open_file(&file) else{
        return false;
    };
    if id.token()!=seal.native_file_id {
        return false;
    }
    native_metadata_sha(&file)
        .is_ok_and(|sha|sha==expected_metadata_sha)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopPoint {
    Never, AfterManifest, AfterBackup, AfterStage, AfterPublish,
    #[cfg(all(test,windows))]
    BeforePublishSameBytesIdentitySwap,
    #[cfg(all(test,windows))]
    AfterPublishSameBytesIdentitySwap,
}

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
    commit_in_dir_selected(root,target,operation_id,baseline_sha,replacement,
        approval_sha,stop,
        #[cfg(windows)]
        None,
    )
}
fn commit_in_dir_selected(
    root: &Dir,
    target: &str,
    operation_id: &str,
    baseline_sha: &str,
    replacement: &[u8],
    approval_sha: &str,
    stop: StopPoint,
    #[cfg(windows)] picked_identity:Option<crate::windows_file_guard::WindowsFileIdentity>,
) -> Result<EditReceipt, EditCommitError> {
    let _serialization = EDIT_TRANSACTION_GATE.lock()
        .map_err(|_| EditCommitError::Uncertain)?;
    if !filename(target) { return Err(EditCommitError::InvalidTarget); }
    if !valid_id(operation_id) { return Err(EditCommitError::InvalidOperationId); }
    if !valid_sha(baseline_sha) || !valid_sha(approval_sha) {
        return Err(EditCommitError::InvalidDigest);
    }
    if replacement.len() > MAX_EDIT_DOCUMENT_BYTES {
        return Err(EditCommitError::InvalidContent);
    }
    let (original, perms) = regular_bytes(root, target)?;
    #[cfg(windows)]
    let original_metadata={
        let handle=root.open(target).map_err(io)?.into_std();
        crate::windows_file_guard::snapshot_metadata(&handle)
            .map_err(|_|EditCommitError::UnsafeTarget)?
    };
    #[cfg(windows)]
    let original_identity=strict_windows_file_identity(root,target)?;
    #[cfg(windows)]
    if picked_identity.is_some_and(|id|id!=original_identity) {
        return Err(EditCommitError::SourceConflict);
    }
    if sha(&original) != baseline_sha { return Err(EditCommitError::SourceConflict); }
    plan_text_edit(target, &original, baseline_sha, replacement)
        .map_err(|_| EditCommitError::InvalidContent)?;
    let manifest = EditManifest {
        schema: 1, edit_id: operation_id.into(), target: target.into(),
        original_sha256: baseline_sha.into(),
        replacement_sha256: sha(replacement),
        approval_sha256: approval_sha.into(),
        native_metadata_sha256:{
            #[cfg(windows)]
            {Some(sha(&original_metadata.immutable_identity_bytes()))}
            #[cfg(not(windows))]
            {None}
        },
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
    #[cfg(windows)]
    {
        // Refuse if a new stage would discard the original's owner/group/
        // DACL. Keep original creation/access times; last-write is new.
        // The Windows FFI is isolated in lu-win-worker-job.
        let original_file=root.open(target).map_err(io)?.into_std();
        let mut opts=OpenOptions::new();
        opts.read(true).write(true);
        let stage_file=root.open_with(&stage_name,&opts).map_err(io)?.into_std();
        crate::windows_file_guard::prepare_stage_metadata(
            &original_file,&stage_file,&original_metadata
        ).map_err(|_|EditCommitError::Uncertain)?;
    }
    #[cfg(windows)]
    write_publish_seal(root,operation_id,target,&stage_name,
        &manifest.replacement_sha256,
        manifest.native_metadata_sha256.as_deref().unwrap_or(""),&perms)?;
    if stop == StopPoint::AfterStage { return Err(EditCommitError::Uncertain); }
    #[cfg(windows)]
    let staged_identity=strict_windows_file_identity(root,&stage_name)?;

    // Bounded last recheck. A non-cooperating external writer may still race
    // between this read and rename; backup custody retains the old bytes.
    #[cfg(all(test,windows))]
    if stop==StopPoint::BeforePublishSameBytesIdentitySwap {
        // Deterministic adversarial fixture: external actor replaces the
        // original file with an identical-byte *different* native file ID
        // between the stage barrier and final prepublication checks.
        let displaced=sidecar(operation_id,"external-displaced");
        root.rename(target,root,&displaced).map_err(io)?;
        write_new_synced(root,target,&original,&perms)?;
    }
    #[cfg(windows)]
    let _no_write_lease={
        let source=root.open(target).map_err(io)?.into_std();
        crate::windows_file_guard::no_write_lease(&source)
            .map_err(|_|EditCommitError::Uncertain)?
    };
    // The lease remains live until after rename and post-publish checks.
    // SHARE_DELETE is necessary for Lu's own rename: this is not a
    // cross-process name-bound atomic compare-and-swap.
    let (latest, _) = regular_bytes(root, target)?;
    if sha(&latest) != baseline_sha {
        return Err(EditCommitError::SourceConflict);
    }
    #[cfg(windows)]
    {
        if strict_windows_file_identity(root,target)? != original_identity {
            return Err(EditCommitError::SourceConflict);
        }
        let current=root.open(target).map_err(io)?.into_std();
        crate::windows_file_guard::check_source_metadata(&current,&original_metadata)
            .map_err(|_|EditCommitError::SourceConflict)?;
    }
    // This detects identity substitutions even if a different inode has
    // identical bytes. It is not an OS-atomic cross-process compare-and-swap.
    root.rename(&stage_name, root, target).map_err(io)?;
    #[cfg(all(test,windows))]
    if stop==StopPoint::AfterPublishSameBytesIdentitySwap {
        // External writer moves Lu's published version and supplies a
        // different object whose bytes are identical to the approved output.
        // No new journal effect is authorized; success must be REFUSED.
        let displaced=sidecar(operation_id,"published-displaced");
        root.rename(target,root,&displaced).map_err(io)?;
        write_new_synced(root,target,replacement,&perms)?;
        // Give the impostor identical bytes AND identical security and
        // creation metadata. The persisted staged FileId seal, not ACL or
        // byte comparison, must detect the wrong published object.
        let published=root.open(&displaced).map_err(io)?.into_std();
        let snapshot=crate::windows_file_guard::snapshot_metadata(&published)
            .map_err(|_|EditCommitError::Uncertain)?;
        let mut opts=OpenOptions::new();opts.read(true).write(true);
        let impostor=root.open_with(target,&opts).map_err(io)?.into_std();
        crate::windows_file_guard::prepare_stage_metadata(
            &published,&impostor,&snapshot).map_err(|_|EditCommitError::Uncertain)?;
    }
    if stop == StopPoint::AfterPublish { return Err(EditCommitError::Uncertain); }
    sync_parent(root).map_err(|_| EditCommitError::Uncertain)?;
    let published = regular_bytes(root, target).map_err(|_| EditCommitError::Uncertain)?.0;
    if sha(&published) != manifest.replacement_sha256 {
        return Err(EditCommitError::Uncertain);
    }
    #[cfg(windows)]
    {
        if strict_windows_file_identity(root,target)
            .map_err(|_|EditCommitError::Uncertain)?!=staged_identity ||
            !matches_publish_seal(root,operation_id,target,
                &manifest.replacement_sha256,
                manifest.native_metadata_sha256.as_deref().unwrap_or("")) {
            return Err(EditCommitError::Uncertain);
        }
        let published_file=root.open(target).map_err(|_|EditCommitError::Uncertain)?.into_std();
        crate::windows_file_guard::check_source_metadata(&published_file,&original_metadata)
            .map_err(|_|EditCommitError::Uncertain)?;
    }
    Ok(EditReceipt {
        backup_id: operation_id.into(), original_sha256: baseline_sha.into(),
        replacement_sha256: manifest.replacement_sha256,
        bytes_written: replacement.len() as u64,
    })
}

/// A journal row marked APPLIED is not itself proof of a successful, exact
/// edit. Decode the durable ToolReply AND typed EditExistingResult and bind
/// the receipt to the originally reviewed bytes, length and call ID.
///
/// This is a pure verifier; it does not authorize a restore or make an
/// UNCERTAIN effect eligible for replay.
pub fn verified_edit_receipt(
    effect: &EffectRecord, call_id: &str, original_sha: &str,
    replacement_sha: &str, replacement_bytes: u64,
) -> bool {
    if effect.state != EffectState::Applied ||
        effect.tool_id != EDIT_TOOL_ID ||
        effect.call_id != call_id ||
        !valid_id(call_id) || !valid_sha(original_sha) ||
        !valid_sha(replacement_sha) {
        return false;
    }
    let Some(bytes) = effect.receipt.as_deref() else { return false; };
    let Ok(reply) = crate::wire::decode_reply(bytes) else { return false; };
    if reply.call_id != call_id || reply.output_type_url != EDIT_RESULT_TYPE {
        return false;
    }
    let Some(proto::tool_reply::Outcome::Output(payload)) = reply.outcome else {
        return false;
    };
    let Ok(result) = proto::EditExistingResult::decode(payload.as_slice()) else {
        return false;
    };
    result.committed && result.backup_id == call_id &&
        result.original_sha256 == original_sha &&
        result.replacement_sha256 == replacement_sha &&
        result.bytes_written == replacement_bytes
}

/// Verify an independently journaled restore receipt; an APPLIED state
/// without a typed success result never proves a restoration completed.
pub fn verified_rollback_receipt(
    effect: &EffectRecord, rollback_id: &str, original_edit_id: &str,
    restored_sha: &str, restored_bytes: u64,
) -> bool {
    if effect.state != EffectState::Applied ||
        effect.tool_id != ROLLBACK_TOOL_ID ||
        effect.call_id != rollback_id ||
        !valid_id(rollback_id) || !valid_id(original_edit_id) ||
        !valid_sha(restored_sha) {
        return false;
    }
    let Some(bytes) = effect.receipt.as_deref() else { return false; };
    let Ok(reply) = crate::wire::decode_reply(bytes) else { return false; };
    if reply.call_id != rollback_id || reply.output_type_url != ROLLBACK_RESULT_TYPE {
        return false;
    }
    let Some(proto::tool_reply::Outcome::Output(payload)) = reply.outcome else {
        return false;
    };
    let Ok(result) = proto::EditRollbackResult::decode(payload.as_slice()) else {
        return false;
    };
    result.committed && result.original_edit_id == original_edit_id &&
        result.restored_sha256 == restored_sha &&
        result.restored_bytes == restored_bytes
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
    let target_result=regular_bytes(root,&manifest.target);
    #[cfg(windows)]
    if target_result.is_ok() && !matches_native_metadata(
        root,&manifest.target,manifest.native_metadata_sha256.as_deref().unwrap_or("")
    ) {
        return EditObservation::ChangedOrDamaged;
    }
    #[cfg(windows)]
    if let Ok((bytes,_))=&target_result {
        if sha(bytes)==manifest.replacement_sha256 &&
            !matches_publish_seal(root,operation_id,&manifest.target,
                &manifest.replacement_sha256,
                manifest.native_metadata_sha256.as_deref().unwrap_or("")) {
            return EditObservation::ChangedOrDamaged;
        }
    }
    match target_result {
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
    rollback_with_stop(root, target, original_edit_id, rollback_id,
        expected_current, expected_restored, StopPoint::Never)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RollbackManifest {
    schema: u32,
    rollback_id: String,
    original_edit_id: String,
    target: String,
    prior_sha256: String,
    restore_sha256: String,
    #[serde(default,skip_serializing_if="Option::is_none")]
    native_metadata_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackObservation {
    NoManifest,
    StillAtReplacement,
    RestoredFromBackup,
    ChangedOrDamaged,
    Unavailable,
}

/// An observation alone never changes a journal state or authorizes replay.
pub fn observe_rollback(root: &Dir, id: &str) -> RollbackObservation {
    if !valid_id(id) { return RollbackObservation::Unavailable; }
    let path = sidecar(id, "rollback-meta");
    match root.symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound =>
            return RollbackObservation::NoManifest,
        Err(_) => return RollbackObservation::Unavailable,
        Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() ||
            meta.len() > MAX_MANIFEST_BYTES => return RollbackObservation::ChangedOrDamaged,
        Ok(_) => {}
    };
    let Ok((data,_))=regular_bytes(root,&path) else {
        return RollbackObservation::ChangedOrDamaged;
    };
    if data.len()>MAX_MANIFEST_BYTES as usize {
        return RollbackObservation::ChangedOrDamaged;
    }
    let Ok(m) = serde_json::from_slice::<RollbackManifest>(&data) else {
        return RollbackObservation::ChangedOrDamaged;
    };
    if m.schema != 1 || m.rollback_id != id || !valid_id(&m.original_edit_id) ||
        !filename(&m.target) || !valid_sha(&m.prior_sha256) ||
        !valid_sha(&m.restore_sha256) ||
        !valid_native_metadata_sha(m.native_metadata_sha256.as_deref()) {
        return RollbackObservation::ChangedOrDamaged;
    }
    let Ok(original_manifest) = read_manifest(root, &m.original_edit_id) else {
        return RollbackObservation::ChangedOrDamaged;
    };
    if original_manifest.target != m.target ||
        original_manifest.original_sha256 != m.restore_sha256 ||
        original_manifest.replacement_sha256 != m.prior_sha256 ||
        original_manifest.native_metadata_sha256!=m.native_metadata_sha256 {
        return RollbackObservation::ChangedOrDamaged;
    }
    let Ok((backup,_)) = regular_bytes(root, &sidecar(&m.original_edit_id, "orig")) else {
        return RollbackObservation::ChangedOrDamaged;
    };
    if sha(&backup) != m.restore_sha256 {
        return RollbackObservation::ChangedOrDamaged;
    }
    // A rollback must preserve the displaced edited version. It may not
    // exist if a crash occurred immediately after the intent record, but it
    // must be present and verified when the target is already restored.
    let prior=match regular_bytes(root,&sidecar(id,"prior")) {
        Ok((bytes,_)) if sha(&bytes)==m.prior_sha256=>true,
        Err(EditCommitError::MissingTarget)=>false,
        _=>return RollbackObservation::ChangedOrDamaged,
    };
    let target_result=regular_bytes(root,&m.target);
    #[cfg(windows)]
    if target_result.is_ok() && !matches_native_metadata(
        root,&m.target,m.native_metadata_sha256.as_deref().unwrap_or("")
    ){
        return RollbackObservation::ChangedOrDamaged;
    }
    #[cfg(windows)]
    if let Ok((bytes,_))=&target_result {
        if sha(bytes)==m.restore_sha256 &&
            !matches_publish_seal(root,id,&m.target,&m.restore_sha256,
                m.native_metadata_sha256.as_deref().unwrap_or("")) {
            return RollbackObservation::ChangedOrDamaged;
        }
    }
    match target_result {
        Ok((bytes,_)) if sha(&bytes) == m.restore_sha256 && prior =>
            RollbackObservation::RestoredFromBackup,
        Ok((bytes,_)) if sha(&bytes) == m.restore_sha256 =>
            RollbackObservation::ChangedOrDamaged,
        Ok((bytes,_)) if sha(&bytes) == m.prior_sha256 =>
            RollbackObservation::StillAtReplacement,
        Ok(_) => RollbackObservation::ChangedOrDamaged,
        Err(_) => RollbackObservation::Unavailable,
    }
}

/// Read-only, byte-exact rollback review. A correct backup is not sufficient:
/// the current target must also be exactly the replacement recorded by the
/// original edit intent. This function never authorizes or performs rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRollbackPreview {
    pub original_edit_id: String,
    pub file_name: String,
    pub original_sha256: String,
    pub replacement_sha256: String,
    /// Complete UTF-8 text, not a truncated excerpt.
    pub current_text: String,
    /// Complete verified original UTF-8 text, not a truncated excerpt.
    pub restore_text: String,
}
/// Small review ceiling keeps the UI bounded. It does not change the
/// separately authorized transaction limit or automatically restore anything.
pub const MAX_ROLLBACK_REVIEW_BYTES: usize = 64 * 1024;

pub fn inspect_rollback_candidate(
    root: &Dir, target: &str, original_edit_id: &str,
) -> Result<VerifiedRollbackPreview, EditCommitError> {
    if !filename(target) { return Err(EditCommitError::InvalidTarget); }
    if !valid_id(original_edit_id) {
        return Err(EditCommitError::InvalidOperationId);
    }
    let manifest = read_manifest(root, original_edit_id)?;
    if manifest.target != target {
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    let (restore, _) = regular_bytes(root, &sidecar(original_edit_id, "orig"))?;
    if restore.len() > MAX_ROLLBACK_REVIEW_BYTES
        || sha(&restore) != manifest.original_sha256 {
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    let (current, _) = regular_bytes(root, target)?;
    #[cfg(windows)]
    if !matches_native_metadata(
        root,target,manifest.native_metadata_sha256.as_deref().unwrap_or("")
    ){
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    if sha(&current) != manifest.replacement_sha256 {
        return Err(EditCommitError::SourceConflict);
    }
    #[cfg(windows)]
    if !matches_publish_seal(root,original_edit_id,target,
        &manifest.replacement_sha256,
        manifest.native_metadata_sha256.as_deref().unwrap_or("")) {
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    if current.len() > MAX_ROLLBACK_REVIEW_BYTES {
        return Err(EditCommitError::InvalidContent);
    }
    // Validate the *restored* file as though it were a fresh edit, so
    // a damaged JSON backup, missing UTF-8 or a no-op cannot be offered.
    plan_text_edit(target, &current, &manifest.replacement_sha256, &restore)
        .map_err(|_| EditCommitError::InvalidRecoveryRecord)?;
    let current_text = std::str::from_utf8(&current)
        .map_err(|_| EditCommitError::InvalidContent)?;
    let restore_text = std::str::from_utf8(&restore)
        .map_err(|_| EditCommitError::InvalidRecoveryRecord)?;
    if current_text.contains('\0') || restore_text.contains('\0') {
        return Err(EditCommitError::InvalidRecoveryRecord);
    }
    Ok(VerifiedRollbackPreview {
        original_edit_id: original_edit_id.into(),
        file_name: target.into(),
        original_sha256: manifest.original_sha256,
        replacement_sha256: manifest.replacement_sha256,
        current_text: current_text.into(),
        restore_text: restore_text.into(),
    })
}

fn rollback_with_stop(
    root: &Dir, target: &str, original_edit_id: &str, rollback_id: &str,
    expected_current: &str, expected_restored: &str, stop: StopPoint
) -> Result<u64, EditCommitError> {
    rollback_with_stop_selected(root,target,original_edit_id,rollback_id,
        expected_current,expected_restored,stop,
        #[cfg(windows)]
        None,
    )
}
fn rollback_with_stop_selected(
    root: &Dir, target: &str, original_edit_id: &str, rollback_id: &str,
    expected_current: &str, expected_restored: &str, stop: StopPoint,
    #[cfg(windows)] picked_identity:Option<crate::windows_file_guard::WindowsFileIdentity>,
) -> Result<u64, EditCommitError> {
    let _serialization = EDIT_TRANSACTION_GATE.lock()
        .map_err(|_| EditCommitError::Uncertain)?;
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
    #[cfg(windows)]
    let original_metadata={
        let handle=root.open(target).map_err(io)?.into_std();
        crate::windows_file_guard::snapshot_metadata(&handle)
            .map_err(|_|EditCommitError::UnsafeTarget)?
    };
    #[cfg(windows)]
    if !matches_native_metadata(
        root,target,manifest.native_metadata_sha256.as_deref().unwrap_or("")
    ){
        return Err(EditCommitError::SourceConflict);
    }
    #[cfg(windows)]
    let original_identity=strict_windows_file_identity(root,target)?;
    #[cfg(windows)]
    if picked_identity.is_some_and(|id|id!=original_identity) {
        return Err(EditCommitError::SourceConflict);
    }
    if sha(&current) != expected_current {
        return Err(EditCommitError::SourceConflict);
    }
    let rollback_meta = RollbackManifest {
        schema: 1, rollback_id: rollback_id.into(),
        original_edit_id: original_edit_id.into(), target: target.into(),
        prior_sha256: expected_current.into(), restore_sha256: expected_restored.into(),
        native_metadata_sha256:{
            #[cfg(windows)]
            {Some(sha(&original_metadata.immutable_identity_bytes()))}
            #[cfg(not(windows))]
            {None}
        },
    };
    let meta_bytes = serde_json::to_vec(&rollback_meta)
        .map_err(|_| EditCommitError::InvalidRecoveryRecord)?;
    write_new_synced(root, &sidecar(rollback_id, "rollback-meta"), &meta_bytes, &perms)?;
    if stop == StopPoint::AfterManifest { return Err(EditCommitError::Uncertain); }
    // Independently preserve the currently edited bytes as well, so an
    // interrupted/complete restore never destroys the just-replaced version.
    let prior_name=sidecar(rollback_id, "prior");
    write_new_synced(root, &prior_name, &current, &perms)?;
    if sha(&regular_bytes(root, &prior_name)?.0) != expected_current {
        return Err(EditCommitError::Uncertain);
    }
    if stop == StopPoint::AfterBackup { return Err(EditCommitError::Uncertain); }
    let stage = sidecar(rollback_id, "rollback");
    write_new_synced(root, &stage, &backup, &perms)?;
    if sha(&regular_bytes(root, &stage)?.0) != expected_restored {
        return Err(EditCommitError::Uncertain);
    }
    #[cfg(windows)]
    {
        let current_file=root.open(target).map_err(io)?.into_std();
        let mut opts=OpenOptions::new();opts.read(true).write(true);
        let stage_file=root.open_with(&stage,&opts).map_err(io)?.into_std();
        crate::windows_file_guard::prepare_stage_metadata(
            &current_file,&stage_file,&original_metadata
        ).map_err(|_|EditCommitError::Uncertain)?;
    }
    #[cfg(windows)]
    write_publish_seal(root,rollback_id,target,&stage,expected_restored,
        rollback_meta.native_metadata_sha256.as_deref().unwrap_or(""),&perms)?;
    if stop == StopPoint::AfterStage { return Err(EditCommitError::Uncertain); }
    #[cfg(windows)]
    let staged_identity=strict_windows_file_identity(root,&stage)?;
    #[cfg(windows)]
    let _no_write_lease={
        let source=root.open(target).map_err(io)?.into_std();
        crate::windows_file_guard::no_write_lease(&source)
            .map_err(|_|EditCommitError::Uncertain)?
    };
    if sha(&regular_bytes(root, target)?.0) != expected_current {
        return Err(EditCommitError::SourceConflict);
    }
    #[cfg(windows)]
    {
        if strict_windows_file_identity(root,target)?!=original_identity {
            return Err(EditCommitError::SourceConflict);
        }
        let original=root.open(target).map_err(io)?.into_std();
        crate::windows_file_guard::check_source_metadata(&original,&original_metadata)
            .map_err(|_|EditCommitError::SourceConflict)?;
    }
    root.rename(&stage, root, target).map_err(io)?;
    #[cfg(all(test,windows))]
    if stop==StopPoint::AfterPublishSameBytesIdentitySwap {
        let displaced=sidecar(rollback_id,"published-displaced");
        root.rename(target,root,&displaced).map_err(io)?;
        write_new_synced(root,target,&backup,&perms)?;
        let published=root.open(&displaced).map_err(io)?.into_std();
        let snapshot=crate::windows_file_guard::snapshot_metadata(&published)
            .map_err(|_|EditCommitError::Uncertain)?;
        let mut opts=OpenOptions::new();opts.read(true).write(true);
        let impostor=root.open_with(target,&opts).map_err(io)?.into_std();
        crate::windows_file_guard::prepare_stage_metadata(
            &published,&impostor,&snapshot).map_err(|_|EditCommitError::Uncertain)?;
    }
    if stop == StopPoint::AfterPublish { return Err(EditCommitError::Uncertain); }
    sync_parent(root).map_err(|_| EditCommitError::Uncertain)?;
    if sha(&regular_bytes(root, target).map_err(|_| EditCommitError::Uncertain)?.0)
        != expected_restored {
        return Err(EditCommitError::Uncertain);
    }
    #[cfg(windows)]
    {
        if strict_windows_file_identity(root,target)
            .map_err(|_|EditCommitError::Uncertain)?!=staged_identity ||
            !matches_publish_seal(root,rollback_id,target,expected_restored,
                rollback_meta.native_metadata_sha256.as_deref().unwrap_or("")) {
            return Err(EditCommitError::Uncertain);
        }
        let current=root.open(target).map_err(|_|EditCommitError::Uncertain)?.into_std();
        crate::windows_file_guard::check_source_metadata(&current,&original_metadata)
            .map_err(|_|EditCommitError::Uncertain)?;
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
        #[cfg(windows)]
        if self.reviewed.host_file_identity.is_none() {
            return Err(AuthorizationError::new("Windows review missing native picked FileId"));
        }
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

struct EditTool {
    descriptor: ToolDescriptor, root: Arc<Dir>, rollback: bool,
    #[cfg(windows)]
    selected_target_identity:Option<(String,crate::windows_file_guard::WindowsFileIdentity)>,
}
impl NativeTool for EditTool {
    fn descriptor(&self) -> &ToolDescriptor { &self.descriptor }
    fn invoke(&self, call: ToolInvocation) -> ToolFuture<'_> {
        let root = Arc::clone(&self.root);
        let rollback = self.rollback;
        #[cfg(windows)]
        let selected_target_identity=self.selected_target_identity.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || -> Result<ToolOutput, ToolFailure> {
                let (type_url, reply) = if rollback {
                    let req = proto::EditRollbackRequest::decode(call.input.bytes.as_slice())
                        .map_err(|_| tool_failure("BAD_PROTOBUF"))?;
                    #[cfg(windows)]
                    let approved_id={
                        let (name,id)=selected_target_identity.as_ref()
                            .ok_or_else(||tool_failure("WIN_SELECTED_FILE_ID_REQUIRED"))?;
                        if name!=&req.relative_path {
                            return Err(tool_failure("WIN_SELECTED_TARGET_MISMATCH"));
                        }
                        *id
                    };
                    let len = rollback_with_stop_selected(
                        &root, &req.relative_path, &req.original_edit_id,
                        &call.call_id, &req.expected_current_sha256,
                        &req.expected_restored_sha256,StopPoint::Never,
                        #[cfg(windows)] Some(approved_id)
                    ).map_err(|_| tool_failure("ROLLBACK_UNCERTAIN"))?;
                    let result = proto::EditRollbackResult {
                        original_edit_id: req.original_edit_id, restored_bytes: len,
                        restored_sha256: req.expected_restored_sha256, committed: true
                    };
                    (ROLLBACK_RESULT_TYPE, result.encode_to_vec())
                } else {
                    let req = proto::EditExistingRequest::decode(call.input.bytes.as_slice())
                        .map_err(|_| tool_failure("BAD_PROTOBUF"))?;
                    #[cfg(windows)]
                    let approved_id={
                        let (name,id)=selected_target_identity.as_ref()
                            .ok_or_else(||tool_failure("WIN_SELECTED_FILE_ID_REQUIRED"))?;
                        if name!=&req.relative_path {
                            return Err(tool_failure("WIN_SELECTED_TARGET_MISMATCH"));
                        }
                        *id
                    };
                    let receipt = commit_in_dir_selected(
                        &root, &req.relative_path, &call.call_id,
                        &req.expected_original_sha256, &req.replacement,
                        &req.reviewed_approval_sha256, StopPoint::Never,
                        #[cfg(windows)] Some(approved_id)
                    ).map_err(|_| tool_failure("EDIT_UNCERTAIN"))?;
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
    register_edit_tools_pinned_internal(registry,root,
        #[cfg(windows)]
        None,
    )
}
/// Only the trusted native picker may supply this identity. On Windows the
/// legacy unbound registrar can discover tools but they refuse to execute.
#[cfg(windows)]
pub fn register_edit_tools_pinned_selected(
    registry:&NativeToolRegistry,root:Arc<Dir>,
    selected_name:&str,
    selected_identity:crate::windows_file_guard::WindowsFileIdentity
)->Result<(),RegistryError>{
    register_edit_tools_pinned_internal(registry,root,Some((selected_name.into(),selected_identity)))
}
fn register_edit_tools_pinned_internal(
    registry:&NativeToolRegistry,root:Arc<Dir>,
    #[cfg(windows)] selected_target_identity:Option<(
        String,crate::windows_file_guard::WindowsFileIdentity
    )>,
)->Result<(),RegistryError>{
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
            Arc::new(EditTool { descriptor: edit, root: Arc::clone(&root), rollback: false,
                #[cfg(windows)] selected_target_identity:selected_target_identity.clone() }),
            Arc::new(EditTool { descriptor: rollback, root, rollback: true,
                #[cfg(windows)] selected_target_identity }),
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
    fn simultaneous_lu_edits_serialize_and_refuse_the_loser() {
        let path = temp();
        fs::write(path.join("t.txt"), b"baseline").unwrap();
        let root = Arc::new(Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap());
        let threads: Vec<_> = (0..2).map(|i| {
            let dir = Arc::clone(&root);
            std::thread::spawn(move || {
                let new = if i == 0 { b"first".as_slice() } else { b"second".as_slice() };
                commit_in_dir(&dir, "t.txt", &format!("g-process-race-edit-{i:04}"),
                    &sha(b"baseline"), new, &sha(b"review"),
                    StopPoint::Never)
            })
        }).collect();
        let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|r| r.is_err()).count(), 1);
        assert!(results.iter().any(|r| r.as_ref().err() == Some(&EditCommitError::SourceConflict)));
        let final_bytes = fs::read(path.join("t.txt")).unwrap();
        assert!(final_bytes == b"first" || final_bytes == b"second");
        drop(root);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn read_only_undo_review_requires_verified_backup_and_unchanged_published_file() {
        let path = temp();
        let original = b"{\"version\":1}\n";
        let edited = b"{\"version\":2}\n";
        fs::write(path.join("notes.json"), original).unwrap();
        let root = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
        let id = "g-undo-preview-test-00001";
        commit_in_dir(&root, "notes.json", id,
            &sha(original), edited, &sha(b"reviewed"), StopPoint::Never).unwrap();
        let preview = inspect_rollback_candidate(&root, "notes.json", id).unwrap();
        assert_eq!(preview.current_text.as_bytes(), edited);
        assert_eq!(preview.restore_text.as_bytes(), original);
        assert_eq!(preview.original_sha256, sha(original));
        assert_eq!(preview.replacement_sha256, sha(edited));
        assert_eq!(
            inspect_rollback_candidate(&root, "different.json", id),
            Err(EditCommitError::InvalidRecoveryRecord)
        );
        assert!(root.symlink_metadata("different.json").is_err());
        fs::write(path.join("notes.json"), b"{\"version\":3}\n").unwrap();
        assert_eq!(
            inspect_rollback_candidate(&root, "notes.json", id),
            Err(EditCommitError::SourceConflict)
        );
        fs::write(path.join("notes.json"), edited).unwrap();
        fs::write(path.join(sidecar(id, "orig")), b"{\"version\":0}\n").unwrap();
        assert_eq!(
            inspect_rollback_candidate(&root, "notes.json", id),
            Err(EditCommitError::InvalidRecoveryRecord)
        );
        assert_eq!(fs::read(path.join("notes.json")).unwrap(), edited);
        drop(root);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn undo_review_does_not_consume_the_original_or_create_a_second_effect() {
        let path = temp();
        fs::write(path.join("memo.md"), b"Before\n").unwrap();
        let root = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
        let id = "g-undo-preview-memo-00001";
        commit_in_dir(&root, "memo.md", id, &sha(b"Before\n"), b"After\n",
            &sha(b"reviewed"), StopPoint::Never).unwrap();
        let one = inspect_rollback_candidate(&root, "memo.md", id).unwrap();
        let two = inspect_rollback_candidate(&root, "memo.md", id).unwrap();
        assert_eq!(one, two);
        assert_eq!(one.current_text, "After\n");
        assert_eq!(one.restore_text, "Before\n");
        assert_eq!(fs::read(path.join("memo.md")).unwrap(), b"After\n");
        assert_eq!(fs::read(path.join(sidecar(id, "orig"))).unwrap(), b"Before\n");
        assert!(!path.join(sidecar(id, "rollback-meta")).exists());
        drop(root);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn durable_receipt_requires_a_matching_successful_typed_edit_not_merely_applied_state() {
        let id="g-verified-edit-receipt-001";
        let old=sha(b"{\"a\":1}");
        let new=sha(b"{\"a\":2}");
        let encoded=proto::ToolReply {
            call_id:id.into(), output_type_url:EDIT_RESULT_TYPE.into(),
            outcome:Some(proto::tool_reply::Outcome::Output(
                proto::EditExistingResult {
                    backup_id:id.into(), bytes_written:7,
                    original_sha256:old.clone(), replacement_sha256:new.clone(),
                    committed:true,
                }.encode_to_vec()
            ))
        }.encode_to_vec();
        let mut effect=EffectRecord {
            call_id:id.into(), tool_id:EDIT_TOOL_ID.into(),
            fingerprint:"some-authenticated-call-hash".into(),
            state:EffectState::Applied, receipt:Some(encoded.clone()),
        };
        assert!(verified_edit_receipt(&effect,id,&old,&new,7));
        assert!(!verified_edit_receipt(&effect,id,&old,&new,8));
        assert!(!verified_edit_receipt(&effect,id,&new,&old,7));
        effect.state=EffectState::Uncertain;
        assert!(!verified_edit_receipt(&effect,id,&old,&new,7));
        effect.state=EffectState::Applied;
        effect.tool_id=ROLLBACK_TOOL_ID.into();
        assert!(!verified_edit_receipt(&effect,id,&old,&new,7));
        effect.tool_id=EDIT_TOOL_ID.into();
        effect.receipt=Some(vec![0xff,0xff,0xff]);
        assert!(!verified_edit_receipt(&effect,id,&old,&new,7));
        let mut wrong_reply=proto::ToolReply::decode(encoded.as_slice()).unwrap();
        wrong_reply.call_id="g-other-edit-receipt-0001".into();
        effect.receipt=Some(wrong_reply.encode_to_vec());
        assert!(!verified_edit_receipt(&effect,id,&old,&new,7));
        let mut wrong_type=proto::ToolReply::decode(encoded.as_slice()).unwrap();
        wrong_type.output_type_url=ROLLBACK_RESULT_TYPE.into();
        effect.receipt=Some(wrong_type.encode_to_vec());
        assert!(!verified_edit_receipt(&effect,id,&old,&new,7));
        let mut denied=proto::ToolReply::decode(encoded.as_slice()).unwrap();
        denied.outcome=Some(proto::tool_reply::Outcome::Failure(proto::ToolError{
            code:"DENIED".into(), message:"no".into(), retryable:false,
        }));
        effect.receipt=Some(denied.encode_to_vec());
        assert!(!verified_edit_receipt(&effect,id,&old,&new,7));
        effect.receipt=None;
        assert!(!verified_edit_receipt(&effect,id,&old,&new,7));
    }

    #[test]
    fn restored_version_requires_exact_new_journal_receipt_not_original_edit_receipt(){
        let rollback_id="g-new-restore-effect-00001";
        let original_edit_id="g-original-edit-effect-001";
        let old=sha(b"Original");
        let payload=proto::EditRollbackResult {
            original_edit_id:original_edit_id.into(),
            restored_bytes:8,restored_sha256:old.clone(),committed:true,
        }.encode_to_vec();
        let mut row=EffectRecord{
            call_id:rollback_id.into(),tool_id:ROLLBACK_TOOL_ID.into(),
            fingerprint:"signed-invocation".into(),state:EffectState::Applied,
            receipt:Some(proto::ToolReply{
                call_id:rollback_id.into(),output_type_url:ROLLBACK_RESULT_TYPE.into(),
                outcome:Some(proto::tool_reply::Outcome::Output(payload))
            }.encode_to_vec()),
        };
        assert!(verified_rollback_receipt(&row,rollback_id,original_edit_id,&old,8));
        assert!(!verified_rollback_receipt(&row,rollback_id,original_edit_id,&old,7));
        assert!(!verified_rollback_receipt(&row,rollback_id,"g-other-original-edit-id",&old,8));
        row.state=EffectState::Uncertain;
        assert!(!verified_rollback_receipt(&row,rollback_id,original_edit_id,&old,8));
        row.state=EffectState::Applied;
        row.tool_id=EDIT_TOOL_ID.into();
        assert!(!verified_rollback_receipt(&row,rollback_id,original_edit_id,&old,8));
        row.tool_id=ROLLBACK_TOOL_ID.into();
        row.receipt=Some(vec![0xFF,0xFF,0xFF]);
        assert!(!verified_rollback_receipt(&row,rollback_id,original_edit_id,&old,8));
        row.receipt=None;
        assert!(!verified_rollback_receipt(&row,rollback_id,original_edit_id,&old,8));
    }

    #[test]
    fn a_rollback_with_damaged_displaced_version_cannot_be_accepted_as_recovered() {
        let path=temp();
        fs::write(path.join("memo.md"),b"Earlier").unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let edit_id="g-edit-prior-check-0001";
        let rollback_id="g-rollback-prior-check-001";
        commit_in_dir(&root,"memo.md",edit_id,&sha(b"Earlier"),b"Later",
            &sha(b"approved"),StopPoint::Never).unwrap();
        rollback_in_dir(&root,"memo.md",edit_id,rollback_id,
            &sha(b"Later"),&sha(b"Earlier")).unwrap();
        assert_eq!(observe_rollback(&root,rollback_id),RollbackObservation::RestoredFromBackup);
        fs::write(path.join(sidecar(rollback_id,"prior")),b"corrupted").unwrap();
        assert_eq!(observe_rollback(&root,rollback_id),RollbackObservation::ChangedOrDamaged);
        assert_eq!(fs::read(path.join("memo.md")).unwrap(),b"Earlier");
        drop(root);fs::remove_dir_all(path).unwrap();
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
    fn rollback_crash_barriers_never_erase_original_backup() {
        for (i, (point, observation, expected)) in [
            (StopPoint::AfterManifest, RollbackObservation::StillAtReplacement, b"after".as_slice()),
            (StopPoint::AfterBackup, RollbackObservation::StillAtReplacement, b"after".as_slice()),
            (StopPoint::AfterStage, RollbackObservation::StillAtReplacement, b"after".as_slice()),
            (StopPoint::AfterPublish, RollbackObservation::RestoredFromBackup, b"before".as_slice()),
        ].into_iter().enumerate() {
            let path = temp();
            fs::write(path.join("t.txt"), b"before").unwrap();
            let root = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
            let orig_id = format!("g-original-rollback-test-{i:04}");
            let rollback_id = format!("g-rollback-crash-test-{i:04}");
            commit_in_dir(&root, "t.txt", &orig_id, &sha(b"before"),
                b"after", &sha(b"reviewed"), StopPoint::Never).unwrap();
            assert_eq!(rollback_with_stop(&root, "t.txt", &orig_id, &rollback_id,
                &sha(b"after"), &sha(b"before"), point),
                Err(EditCommitError::Uncertain));
            assert_eq!(observe_rollback(&root, &rollback_id), observation);
            assert_eq!(fs::read(path.join("t.txt")).unwrap(), expected);
            assert_eq!(fs::read(path.join(sidecar(&orig_id, "orig"))).unwrap(), b"before");
            if point != StopPoint::AfterManifest {
                assert_eq!(fs::read(path.join(sidecar(&rollback_id, "prior"))).unwrap(), b"after");
            }
            drop(root);
            fs::remove_dir_all(path).unwrap();
        }
    }

    #[test]
    fn interrupted_id_is_never_reused_and_corrupt_backup_blocks_rollback() {
        let path = temp();
        fs::write(path.join("t.txt"), b"old").unwrap();
        let root = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
        let id = "g-edit-duplicate-stage-00001";
        assert_eq!(commit_in_dir(&root, "t.txt", id, &sha(b"old"),
            b"new", &sha(b"review"), StopPoint::AfterManifest),
            Err(EditCommitError::Uncertain));
        assert_eq!(commit_in_dir(&root, "t.txt", id, &sha(b"old"),
            b"new", &sha(b"review"), StopPoint::Never),
            Err(EditCommitError::ArtifactCollision));
        assert_eq!(fs::read(path.join("t.txt")).unwrap(), b"old");
        let valid = "g-edit-backup-tamper-0001";
        commit_in_dir(&root, "t.txt", valid, &sha(b"old"),
            b"new", &sha(b"review"), StopPoint::Never).unwrap();
        fs::write(path.join(sidecar(valid,"orig")), b"tampered").unwrap();
        assert_eq!(observe_edit(&root, valid), EditObservation::ChangedOrDamaged);
        assert_eq!(rollback_in_dir(&root,"t.txt",valid,"g-rollback-tamper-0001",
            &sha(b"new"), &sha(b"old")), Err(EditCommitError::InvalidRecoveryRecord));
        assert_eq!(fs::read(path.join("t.txt")).unwrap(), b"new");
        drop(root);
        fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_admission_refuses_hardlinked_existing_file_before_transaction_intent() {
        let path=temp();
        fs::write(path.join("notes.txt"),b"old").unwrap();
        fs::hard_link(path.join("notes.txt"),path.join("alias.txt")).unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let operation="g-windows-hardlink-reject-00001";
        assert_eq!(commit_in_dir(&root,"notes.txt",operation,&sha(b"old"),
            b"new",&sha(b"review"),StopPoint::Never),
            Err(EditCommitError::UnsafeTarget));
        assert_eq!(fs::read(path.join("notes.txt")).unwrap(),b"old");
        assert_eq!(fs::read(path.join("alias.txt")).unwrap(),b"old");
        assert!(!path.join(sidecar(operation,"meta")).exists());
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_named_stream_must_not_be_discarded_by_edit_or_restore() {
        let path=temp();
        let target=path.join("stream.txt");
        fs::write(&target,b"old").unwrap();
        let ads=std::path::PathBuf::from(format!("{}:LuMetadata",target.display()));
        match fs::write(&ads,b"must preserve") {
            Ok(())=>{
                let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
                let id="g-windows-stream-reject-0001";
                assert_eq!(commit_in_dir(&root,"stream.txt",id,&sha(b"old"),
                    b"new",&sha(b"review"),StopPoint::Never),
                    Err(EditCommitError::UnsafeTarget));
                assert!(!path.join(sidecar(id,"meta")).exists());
                assert_eq!(fs::read(&target).unwrap(),b"old");
                assert_eq!(fs::read(&ads).unwrap(),b"must preserve");
                drop(root);
            }
            Err(e)=>eprintln!("ADS unavailable on this volume: {e}; no ADS acceptance"),
        }
        fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_preflight_blocks_readonly_file_before_creating_a_manifest() {
        let path=temp();
        let target=path.join("readonly.md");
        fs::write(&target,b"old").unwrap();
        let mut perms=fs::metadata(&target).unwrap().permissions();
        perms.set_readonly(true);fs::set_permissions(&target,perms).unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let operation="g-windows-readonly-reject-0001";
        assert_eq!(commit_in_dir(&root,"readonly.md",operation,&sha(b"old"),
            b"new",&sha(b"review"),StopPoint::Never),
            Err(EditCommitError::UnsafeTarget));
        assert!(!path.join(sidecar(operation,"meta")).exists());
        drop(root);
        let mut perms=fs::metadata(&target).unwrap().permissions();
        perms.set_readonly(false);fs::set_permissions(&target,perms).unwrap();
        fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_replacement_retains_creation_time_owner_group_and_dacl_across_undo() {
        let path=temp();
        fs::write(path.join("notes.txt"),b"before").unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let native=|name:&str|{
            let f=root.open(name).unwrap().into_std();
            crate::windows_file_guard::snapshot_metadata(&f).unwrap()
        };
        let original=native("notes.txt");
        let edit_id="g-windows-security-and-time-edit-001";
        commit_in_dir(&root,"notes.txt",edit_id,&sha(b"before"),b"after",
            &sha(b"review"),StopPoint::Never).unwrap();
        let edited=native("notes.txt");
        assert_eq!(original.creation,edited.creation,
            "creation timestamp must not reset on replacement");
        let undo_id="g-windows-security-and-time-undo-001";
        rollback_in_dir(&root,"notes.txt",edit_id,undo_id,
            &sha(b"after"),&sha(b"before")).unwrap();
        let restored=native("notes.txt");
        assert_eq!(original.creation,restored.creation,
            "creation timestamp must survive separately authorized undo");
        assert_eq!(fs::read(path.join("notes.txt")).unwrap(),b"before");
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_postpublish_same_bytes_wrong_file_id_is_never_acknowledged() {
        let path=temp();
        fs::write(path.join("notes.md"),b"before").unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let id="g-win-postpublish-id-swap-edit-001";
        let outcome=commit_in_dir(&root,"notes.md",id,&sha(b"before"),
            b"after",&sha(b"approved"),StopPoint::AfterPublishSameBytesIdentitySwap);
        assert_eq!(outcome,Err(EditCommitError::Uncertain));
        assert_eq!(fs::read(path.join("notes.md")).unwrap(),b"after");
        assert_eq!(fs::read(path.join(sidecar(id,"orig"))).unwrap(),b"before");
        assert_eq!(fs::read(path.join(sidecar(id,"published-displaced"))).unwrap(),b"after");
        let fake=root.open("notes.md").unwrap().into_std();
        let displaced=root.open(&sidecar(id,"published-displaced")).unwrap().into_std();
        assert_eq!(native_metadata_sha(&fake).unwrap(),native_metadata_sha(&displaced).unwrap());
        assert_ne!(
            crate::windows_file_guard::inspect_open_file(&fake).unwrap(),
            crate::windows_file_guard::inspect_open_file(&displaced).unwrap()
        );
        drop(fake);drop(displaced);
        assert_eq!(observe_edit(&root,id),EditObservation::ChangedOrDamaged);
        assert_eq!(inspect_rollback_candidate(&root,"notes.md",id),
            Err(EditCommitError::InvalidRecoveryRecord));
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_undo_postpublish_same_bytes_wrong_file_id_is_never_acknowledged() {
        let path=temp();
        fs::write(path.join("notes.md"),b"before").unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let edit_id="g-win-postpublish-id-swap-old-001";
        let rollback_id="g-win-postpublish-id-swap-undo-001";
        commit_in_dir(&root,"notes.md",edit_id,&sha(b"before"),
            b"after",&sha(b"approved"),StopPoint::Never).unwrap();
        let result=rollback_with_stop(
            &root,"notes.md",edit_id,rollback_id,&sha(b"after"),
            &sha(b"before"),StopPoint::AfterPublishSameBytesIdentitySwap
        );
        assert_eq!(result,Err(EditCommitError::Uncertain));
        assert_eq!(fs::read(path.join("notes.md")).unwrap(),b"before");
        assert_eq!(fs::read(path.join(sidecar(rollback_id,"prior"))).unwrap(),b"after");
        assert_eq!(observe_rollback(&root,rollback_id),RollbackObservation::ChangedOrDamaged);
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_recovery_requires_metadata_witness_even_when_text_bytes_match() {
        use std::process::Command;
        let path=temp();
        fs::write(path.join("notes.md"),b"before").unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let id="g-win-security-witness-rollback-001";
        commit_in_dir(&root,"notes.md",id,&sha(b"before"),
            b"after",&sha(b"approved"),StopPoint::Never).unwrap();
        assert_eq!(observe_edit(&root,id),EditObservation::ReplacementAndBackupVerified);
        assert!(inspect_rollback_candidate(&root,"notes.md",id).is_ok());
        // This modifies only the DACL and inheritance flag; content remains
        // identical. A byte-hash-only recovery system would falsely pass.
        let status=Command::new("icacls").arg(path.join("notes.md"))
            .arg("/inheritance:d").status().expect("Windows icacls unavailable");
        assert!(status.success());
        assert_eq!(fs::read(path.join("notes.md")).unwrap(),b"after");
        assert_eq!(observe_edit(&root,id),EditObservation::ChangedOrDamaged);
        assert_eq!(inspect_rollback_candidate(&root,"notes.md",id),
            Err(EditCommitError::InvalidRecoveryRecord));
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_legacy_manifest_without_security_witness_fails_closed() {
        let path=temp();
        fs::write(path.join("notes.md"),b"before").unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let id="g-win-legacy-manifest-refusal-001";
        commit_in_dir(&root,"notes.md",id,&sha(b"before"),
            b"after",&sha(b"approved"),StopPoint::Never).unwrap();
        let meta_path=path.join(sidecar(id,"meta"));
        let mut data:serde_json::Value=serde_json::from_slice(&fs::read(&meta_path).unwrap()).unwrap();
        data.as_object_mut().unwrap().remove("native_metadata_sha256");
        fs::write(&meta_path,serde_json::to_vec(&data).unwrap()).unwrap();
        assert_eq!(observe_edit(&root,id),EditObservation::ChangedOrDamaged);
        assert_eq!(inspect_rollback_candidate(&root,"notes.md",id),
            Err(EditCommitError::InvalidRecoveryRecord));
        assert_eq!(fs::read(path.join("notes.md")).unwrap(),b"after");
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_external_writer_forces_uncertain_but_never_publishes() {
        let path=temp();
        let target=path.join("notes.txt");
        fs::write(&target,b"old").unwrap();
        // The writer has a genuine writable Windows FILE handle. The
        // original file can be read, but Lu's no-write ReOpenFile lease
        // must not be granted until this handle is closed.
        let external=fs::OpenOptions::new().write(true).open(&target).unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let id="g-windows-no-write-lease-refuse-001";
        let result=commit_in_dir(&root,"notes.txt",id,&sha(b"old"),
            b"new",&sha(b"review"),StopPoint::Never);
        assert_eq!(result,Err(EditCommitError::Uncertain));
        assert_eq!(fs::read(&target).unwrap(),b"old");
        assert!(path.join(sidecar(id,"meta")).exists());
        assert!(path.join(sidecar(id,"orig")).exists());
        assert!(path.join(sidecar(id,"new")).exists());
        drop(external);drop(root);
        fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_explicit_acl_refuses_instead_of_silently_replacing_custom_security() {
        use std::process::Command;
        let path=temp();
        let target=path.join("acl-notes.txt");
        fs::write(&target,b"old").unwrap();
        // Windows icacls copies inherited ACEs into explicit ACEs: this
        // changes the security descriptor, not the default stream's bytes.
        // An ordinarily inherited stage must not silently replace it.
        let status=Command::new("icacls").arg(&target)
            .arg("/inheritance:d").status().expect("Windows icacls missing");
        assert!(status.success(),"cannot establish custom ACL acceptance fixture");
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let effect="g-windows-explicit-acl-refusal-001";
        let result=commit_in_dir(&root,"acl-notes.txt",effect,
            &sha(b"old"),b"new",&sha(b"review"),StopPoint::Never);
        assert_eq!(result,Err(EditCommitError::Uncertain),
            "unsupported custom ACL must be refused before publication");
        assert_eq!(fs::read(&target).unwrap(),b"old");
        assert!(path.join(sidecar(effect,"orig")).exists(),
            "custody must survive uncertain metadata refusal");
        assert!(path.join(sidecar(effect,"new")).exists(),
            "staged data must stay available for manual recovery");
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn native_transaction_rejects_picker_identity_swap_before_manifest_or_backup() {
        let path=temp();
        fs::write(path.join("original.md"),b"reviewed bytes").unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let picked=strict_windows_file_identity(&root,"original.md").unwrap();
        root.rename("original.md",&root,"displaced.md").unwrap();
        fs::write(path.join("original.md"),b"reviewed bytes").unwrap();
        assert_ne!(picked,strict_windows_file_identity(&root,"original.md").unwrap());
        let id="g-windows-picked-identity-swap-0001";
        assert_eq!(commit_in_dir_selected(
            &root,"original.md",id,&sha(b"reviewed bytes"),b"replacement",
            &sha(b"review"),
            StopPoint::Never,Some(picked)
        ),Err(EditCommitError::SourceConflict));
        assert!(!path.join(sidecar(id,"meta")).exists());
        assert_eq!(fs::read(path.join("original.md")).unwrap(),b"reviewed bytes");
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn native_rollback_rejects_picker_identity_swap_before_manifest_or_prior_backup() {
        let path=temp();
        fs::write(path.join("original.md"),b"before").unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let edit_id="g-windows-rollback-idbind-edit-001";
        commit_in_dir(&root,"original.md",edit_id,&sha(b"before"),b"after",
            &sha(b"review"),StopPoint::Never).unwrap();
        let picked=strict_windows_file_identity(&root,"original.md").unwrap();
        root.rename("original.md",&root,"moved-after.md").unwrap();
        fs::write(path.join("original.md"),b"after").unwrap();
        assert_ne!(picked,strict_windows_file_identity(&root,"original.md").unwrap());
        let rollback_id="g-windows-rollback-idbind-000001";
        assert_eq!(rollback_with_stop_selected(
            &root,"original.md",edit_id,rollback_id,
            &sha(b"after"),&sha(b"before"),StopPoint::Never,Some(picked)
        ),Err(EditCommitError::SourceConflict));
        assert!(!path.join(sidecar(rollback_id,"rollback-meta")).exists());
        assert!(!path.join(sidecar(rollback_id,"prior")).exists());
        assert_eq!(fs::read(path.join("original.md")).unwrap(),b"after");
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_same_bytes_different_file_identity_blocks_publication() {
        let path=temp();
        fs::write(path.join("document.md"),b"old").unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        let id="g-windows-same-bytes-race-001";
        assert_eq!(commit_in_dir(
            &root,"document.md",id,&sha(b"old"),b"new",
            &sha(b"review"),StopPoint::BeforePublishSameBytesIdentitySwap
        ),Err(EditCommitError::SourceConflict));
        assert_eq!(fs::read(path.join("document.md")).unwrap(),b"old");
        assert_eq!(fs::read(path.join(sidecar(id,"external-displaced"))).unwrap(),b"old");
        assert_eq!(fs::read(path.join(sidecar(id,"orig"))).unwrap(),b"old");
        assert_eq!(fs::read(path.join(sidecar(id,"new"))).unwrap(),b"new");
        // The guard catches a same-content identity substitution, not just
        // a changed byte digest. No effect receipt may be issued.
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_native_identity_distinguishes_identical_bytes_at_different_file_ids() {
        let path=temp();
        fs::write(path.join("a.txt"),b"identical").unwrap();
        fs::write(path.join("b.txt"),b"identical").unwrap();
        let root=Dir::open_ambient_dir(&path,cap_std::ambient_authority()).unwrap();
        assert_ne!(strict_windows_file_identity(&root,"a.txt").unwrap(),
            strict_windows_file_identity(&root,"b.txt").unwrap());
        drop(root);fs::remove_dir_all(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_and_multiple_hardlinks_are_not_treated_as_safe_edits() {
        use std::os::unix::fs::symlink;
        let path = temp();
        fs::write(path.join("real.txt"), b"old").unwrap();
        symlink("real.txt", path.join("alias.txt")).unwrap();
        let root = Dir::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
        assert_eq!(commit_in_dir(&root,"alias.txt","g-edit-symlink-denial-0001",
            &sha(b"old"), b"new", &sha(b"review"), StopPoint::Never),
            Err(EditCommitError::UnsafeTarget));
        fs::hard_link(path.join("real.txt"),path.join("second-link.txt")).unwrap();
        assert_eq!(commit_in_dir(&root,"real.txt","g-edit-hardlink-denial-0001",
            &sha(b"old"), b"new", &sha(b"review"), StopPoint::Never),
            Err(EditCommitError::UnsafeTarget));
        assert_eq!(fs::read(path.join("real.txt")).unwrap(), b"old");
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
