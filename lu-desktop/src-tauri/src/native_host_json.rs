//! Host-selected, read-only JSON inspection. Model output, WebView strings
//! and retrieved content cannot choose a path, grant or observed file handle.
//! The real OS picker supplies the file; Lu Core's native tool checks its
//! syntax through a pinned handle, typed Protobuf and exact task authority.

use crate::{chat_store::ChatStore, native_host_read::open_verified_selected_file};
use lu_core::{
    contracts::ToolEffect,
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
    },
    journal::EffectJournal,
    model::ModelToolCatalog,
    native_json_inspect::{register_json_inspection_tool_pinned, MAX_JSON_INSPECTION_BYTES},
    policy::{CapabilityGrant, PolicyEngine},
    registry::NativeToolRegistry,
};
use serde::Serialize;
use std::{collections::BTreeSet, path::Path};

const MODEL_NAME: &str = "lu_json_inspect";
const TOOL_ID: &str = "lu.json.inspect";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectedJsonFile {
    pub file_name: String,
    pub valid: bool,
    pub root_kind: String,
    pub bytes_read: u64,
    pub content_sha256: String,
    pub top_level_keys: Vec<String>,
    pub array_length: u32,
    pub error_kind: String,
    pub error_line: u32,
    pub error_column: u32,
}

fn result_from_json(value: &serde_json::Value, file_name: String)
    -> Result<InspectedJsonFile, String>
{
    let field = value.get("result").ok_or("JSON inspection result is unavailable")?;
    let text = |name| field.get(name).and_then(|x| x.as_str()).map(str::to_owned);
    let number = |name| field.get(name).and_then(|x| x.as_u64());
    let list = field.get("top_level_keys").and_then(|x| x.as_array())
        .ok_or("JSON key summary unavailable")?
        .iter().map(|x| x.as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>().ok_or("JSON key summary invalid")?;
    let narrow = |name| number(name)
        .and_then(|n| u32::try_from(n).ok()).ok_or("JSON offset invalid");
    Ok(InspectedJsonFile {
        file_name,
        valid: field.get("valid").and_then(|x| x.as_bool()).ok_or("JSON validity missing")?,
        root_kind: text("root_kind").ok_or("JSON root kind missing")?,
        bytes_read: number("bytes_read").ok_or("JSON size missing")?,
        content_sha256: text("content_sha256").ok_or("JSON hash missing")?,
        top_level_keys: list,
        array_length: narrow("array_length")?,
        error_kind: text("error_kind").ok_or("JSON error classification missing")?,
        error_line: narrow("error_line")?,
        error_column: narrow("error_column")?,
    })
}

/// Only a caller that actually received the local OS file-picker selection
/// may call this function. This routine never trusts a model/webview filepath.
/// A persisted host user turn is verified before a one-call native tool grant.
pub async fn inspect_selected_json_with_receipt(
    chats: &ChatStore,
    effects: &EffectJournal,
    chat_id: &str,
    turn_id: &str,
    selected_path: &Path,
) -> Result<InspectedJsonFile, String> {
    let pinned = open_verified_selected_file(selected_path)?;
    let file_name = selected_path.file_name()
        .and_then(|x| x.to_str())
        .ok_or("selected JSON filename is not valid UTF-8")?
        .to_owned();
    if file_name.len() > 4096 || file_name.is_empty()
        || file_name.chars().any(|c| c.is_control() || matches!(c, '\\' | ':' | '/'))
    {
        return Err("selected JSON filename is not allowed".into());
    }
    let root = selected_path.parent().ok_or("JSON selection has no containing directory")?;
    let registry = NativeToolRegistry::new();
    let mut catalog = ModelToolCatalog::new();
    register_json_inspection_tool_pinned(
        &registry, &mut catalog, root, &file_name, pinned,
    ).map_err(|e| format!("native JSON registration failed: {e}"))?;
    let permitted = BTreeSet::from([MODEL_NAME.to_owned()]);

    // Only real user-triggered native selection can establish this user turn.
    chats.append_user_with_turn_id(
        chat_id, turn_id,
        "Inspect the JSON file I selected in the native file picker (read only)",
    )?;
    let authority = ConversationAuthority::new();
    let task_id = format!("inspect-json-{turn_id}");
    let scope_id = format!("json-file-{turn_id}");
    authority.open_user_task(
        TaskAuthority {
            conversation_id: chat_id.to_owned(),
            user_turn_id: turn_id.to_owned(),
            task_id: task_id.clone(),
            execution_scope_id: scope_id.clone(),
            capabilities: vec![AuthorizedCapability {
                tool_id: TOOL_ID.into(), effect: ToolEffect::ReadWorkspace,
            }],
            expires_at: None,
        }, chats,
    ).map_err(|e| format!("JSON user turn not authorized: {e:?}"))?;
    let policy = PolicyEngine::new();
    policy.grant(CapabilityGrant {
        tool_id: TOOL_ID.into(),
        scope_id: scope_id.clone(),
        effect: ToolEffect::ReadWorkspace,
        expires_at: None,
    }).map_err(|e| format!("JSON inspection grant refused: {e:?}"))?;

    let call = catalog.encode_call(
        &permitted, MODEL_NAME, turn_id, Some(&scope_id),
        &serde_json::json!({
            "path": file_name,
            "max_bytes": MAX_JSON_INSPECTION_BYTES,
        }).to_string(),
    ).map_err(|e| format!("JSON inspection arguments invalid: {e:?}"))?;
    let origin = BoundCallOrigin {
        conversation_id: chat_id.to_owned(),
        user_turn_id: turn_id.to_owned(),
        task_id: task_id.clone(),
    };
    let response = authority.dispatch_bound_durable(
        &registry, effects, call, origin, &policy,
    ).await;
    let _ = authority.revoke_task(&task_id);
    let _ = policy.revoke_scope(&scope_id);
    let value = catalog.decode_result(&permitted, MODEL_NAME, &response)
        .map_err(|e| format!("native JSON inspection failed: {e:?}"))?;
    if value.get("ok").and_then(|x| x.as_bool()) != Some(true) {
        return Err("native JSON inspection rejected the selected file".into());
    }
    let report = result_from_json(&value, file_name)?;
    let summary = if report.valid {
        format!(
            "JSON inspection — {}\nValid JSON · {} · {} bytes\nTop-level keys: {}\nSHA-256: {}",
            report.file_name, report.root_kind, report.bytes_read,
            if report.top_level_keys.is_empty() {
                "(none)".into()
            } else {
                report.top_level_keys.join(", ")
            }, report.content_sha256,
        )
    } else {
        format!(
            "JSON inspection — {}\nInvalid JSON · {} at line {}, column {}\nObserved {} bytes\nSHA-256: {}",
            report.file_name, report.error_kind, report.error_line,
            report.error_column, report.bytes_read, report.content_sha256,
        )
    };
    chats.append_progress(chat_id, turn_id, &summary)?;
    chats.complete(chat_id, turn_id)?;
    Ok(report)
}
