//! Host-initiated file inspection: explicit native OS file selection is the
//! trusted read consent. Model-visible text, WebView path strings, and remembered
//! context cannot select the file or enlarge the grant.
use crate::chat_store::ChatStore;
use lu_core::{
    contracts::ToolEffect,
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
    },
    native_files::register_bundled_file_tools_pinned,
    policy::{CapabilityGrant, PolicyEngine},
    registry::NativeToolRegistry,
};
use serde::Serialize;
use std::{collections::BTreeSet, fs, path::Path};

const MAX_INSPECTION_BYTES: u64 = 64 * 1024;
const TOOL_NAME: &str = "lu_files_read_text";
const NATIVE_TOOL_ID: &str = "lu.files.read_text";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectedFile {
    pub file_name: String,
    pub text: String,
    pub truncated: bool,
}

/// Open a native-picked file with OS-level identity validation before
/// authorizing a read. This handle stays alive until Protobuf dispatch ends.
pub(crate) fn open_verified_selected_file(path: &Path) -> Result<fs::File, String> {
    let before = fs::symlink_metadata(path)
        .map_err(|_| "selected file unavailable")?;
    if !before.file_type().is_file() {
        return Err("select an ordinary file, not a link or directory".into());
    }
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_SHARE_READ: u32 = 0x0000_0001;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if before.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("selected reparse point refused".into());
        }
        // No final-component traversal of junctions/symlinks, and no
        // competing writer/delete sharing while this pinned read runs.
        let file = fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|e| format!("approved file open failed: {e}"))?;
        let after = file.metadata().map_err(|e| format!("approved file metadata: {e}"))?;
        if !after.is_file() || after.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("opened file is not an ordinary file".into());
        }
        // Fail closed where stable by-handle identity is unavailable.
        let same_identity = match (
            before.volume_serial_number(), before.file_index(),
            after.volume_serial_number(), after.file_index()
        ) {
            (Some(v1), Some(i1), Some(v2), Some(i2)) => v1 == v2 && i1 == i2,
            _ => false,
        };
        if !same_identity {
            return Err("selected file changed or identity unavailable".into());
        }
        file
    };
    #[cfg(not(windows))]
    let file = fs::File::open(path)
        .map_err(|e| format!("approved file open failed: {e}"))?;
    let after = file.metadata().map_err(|e| format!("approved file metadata: {e}"))?;
    if !after.is_file() {
        return Err("approved file handle is not a regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != after.dev() || before.ino() != after.ino() {
            return Err("selected file changed before handle acquisition".into());
        }
    }
    Ok(file)
}

/// The caller obtains `path` from the native file picker and `turn_id`
/// from host-generated randomness. Neither may originate from a WebView
/// argument or an LLM tool call.
pub async fn inspect_selected_file(
    chats: &ChatStore,
    chat_id: &str,
    turn_id: &str,
    path: &Path,
) -> Result<InspectedFile, String> {
    inspect_selected_file_limited(chats, chat_id, turn_id, path, MAX_INSPECTION_BYTES).await
}

/// The selected file cannot increase the model-proposed byte budget.
pub async fn inspect_selected_file_limited(
    chats: &ChatStore,
    chat_id: &str,
    turn_id: &str,
    path: &Path,
    max_bytes: u64,
) -> Result<InspectedFile, String> {
    inspect_selected_file_with_receipt(chats, chat_id, turn_id, path, max_bytes, None).await
}

/// Durable host dispatch. The journal is opened and owned by the native
/// desktop host, not a model-visible or WebView-provided path. This records
/// PREPARED -> EXECUTING -> APPLIED / UNCERTAIN before chat presentation.
pub async fn inspect_selected_file_with_receipt(
    chats: &ChatStore,
    chat_id: &str,
    turn_id: &str,
    path: &Path,
    max_bytes: u64,
    effects: Option<&lu_core::journal::EffectJournal>,
) -> Result<InspectedFile, String> {
    if max_bytes == 0 || max_bytes > MAX_INSPECTION_BYTES {
        return Err("invalid approved read limit".into());
    }
    // Host pins the selected file before any model-controlled dispatch.
    // Windows uses OPEN_REPARSE_POINT to inspect the final object itself,
    // not the target of a last-moment symlink substitution.
    let pinned = open_verified_selected_file(path)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("invalid selected filename")?
        .to_owned();
    if file_name.is_empty() || file_name.chars().any(|c| matches!(c, '\\' | ':' | '/' | '\0')) {
        return Err("selected filename is not supported by this tool".into());
    }
    let root = path.parent().ok_or("selected file has no parent folder")?;
    let registry = NativeToolRegistry::new();
    let catalog = register_bundled_file_tools_pinned(&registry, root, &file_name, pinned)
        .map_err(|e| format!("native file registration: {e:?}"))?;
    let allowed = BTreeSet::from([TOOL_NAME.to_string()]);

    // This is an explicit human-initiated host action, not a model-generated
    // approval. Persist the genuine user turn BEFORE opening its authority.
    chats.append_user_with_turn_id(
        chat_id,
        turn_id,
        "Inspect the text file I selected through the native file picker",
    )?;
    let authority = ConversationAuthority::new();
    let task_id = format!("inspect-{turn_id}");
    let scope_id = format!("selected-read-{turn_id}");
    authority
        .open_user_task(
            TaskAuthority {
                conversation_id: chat_id.to_owned(),
                user_turn_id: turn_id.to_owned(),
                task_id: task_id.clone(),
                execution_scope_id: scope_id.clone(),
                capabilities: vec![AuthorizedCapability {
                    tool_id: NATIVE_TOOL_ID.to_owned(),
                    effect: ToolEffect::ReadWorkspace,
                }],
                expires_at: None,
            },
            chats,
        )
        .map_err(|e| format!("user turn not admitted: {e:?}"))?;

    let policy = PolicyEngine::new();
    policy
        .grant(CapabilityGrant {
            tool_id: NATIVE_TOOL_ID.to_owned(),
            scope_id: scope_id.clone(),
            effect: ToolEffect::ReadWorkspace,
            expires_at: None,
        })
        .map_err(|e| format!("read grant failed: {e:?}"))?;

    let args = serde_json::json!({
        "path": file_name,
        "max_bytes": max_bytes,
    }).to_string();
    let call = catalog
        .encode_call(&allowed, TOOL_NAME, turn_id, Some(&scope_id), &args)
        .map_err(|e| format!("read arguments refused: {e:?}"))?;
    let origin = BoundCallOrigin {
        conversation_id: chat_id.to_owned(),
        user_turn_id: turn_id.to_owned(),
        task_id: task_id.clone(),
    };
    let response = if let Some(journal) = effects {
        authority.dispatch_bound_durable(&registry, journal, call, origin, &policy).await
    } else {
        authority.dispatch_bound(&registry, call, origin, &policy).await
    };
    // Revoke even if the read fails. No approval can outlive this operation.
    let _ = authority.revoke_task(&task_id);
    let _ = policy.revoke_scope(&scope_id);
    let value = catalog
        .decode_result(&allowed, TOOL_NAME, &response)
        .map_err(|e| format!("native read failed: {e:?}"))?;
    if value.get("ok").and_then(|v| v.as_bool()) != Some(true) {
        return Err("native read refused or file is not UTF-8 text".into());
    }
    let result = value.get("result").ok_or("missing native file result")?;
    let text = result
        .get("content").and_then(|v| v.as_str())
        .ok_or("native result lacks text")?
        .to_owned();
    let truncated = result.get("truncated").and_then(|v| v.as_bool()).unwrap_or(false);
    chats.append_progress(chat_id, turn_id, &text)?;
    chats.complete(chat_id, turn_id)?;
    Ok(InspectedFile { file_name, text, truncated })
}
