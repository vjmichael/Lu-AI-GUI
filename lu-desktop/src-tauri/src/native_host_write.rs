//! Trusted desktop executor for a reviewed create-new request.
//! No WebView-supplied path, capability or model-originated authority.
use crate::{chat_store::ChatStore, write_proposal::ProposedWrite};
use lu_core::{
    contracts::{ProtoPayload, ToolEffect, ToolInvocation},
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
    },
    journal::EffectJournal,
    native_create_new::{register_create_new, ExactCreateNewPolicy, REQUEST_TYPE, TOOL_ID},
    policy::{CapabilityGrant, PolicyEngine},
    registry::NativeToolRegistry,
    wire::proto,
};
use prost::Message;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedFile {
    pub file_name: String,
    pub exact_target: String,
    pub bytes_written: u64,
    pub sha256: String,
    pub effect_call_id: String,
}
pub async fn execute_approved_create(
    chats: &ChatStore,
    effects: &EffectJournal,
    chat_id: &str,
    user_turn_id: &str,
    effect_call_id: &str,
    trusted_directory: &Path,
    proposal: &ProposedWrite,
) -> Result<CreatedFile, String> {
    if proposal.kind != "propose_create_file"
        || proposal.content.len() > crate::write_proposal::MAX_WRITE_PREVIEW_BYTES
    {
        return Err("invalid reviewed create-new proposal".into());
    }
    let target = trusted_directory.join(&proposal.file_name);
    let sha = format!("{:x}", Sha256::digest(proposal.content.as_bytes()));
    // The host generated this persisted USER turn in direct response to the
    // explicit user approval. The completed model's old turn is not a grant.
    chats.append_user_with_turn_id(
        chat_id, user_turn_id,
        &format!("Approved creating NEW {} in Lu Drafts ({} bytes; SHA-256 {})",
            proposal.file_name, proposal.content.len(), sha),
    )?;
    let registry = NativeToolRegistry::new();
    register_create_new(&registry, trusted_directory)
        .map_err(|e| format!("cannot register native create-new tool: {e:?}"))?;
    let authority = ConversationAuthority::new();
    let policy = PolicyEngine::new();
    let task_id = format!("create-{}", user_turn_id);
    let scope = format!("create-scope-{}", user_turn_id);
    authority.open_user_task(TaskAuthority {
        conversation_id: chat_id.into(),
        user_turn_id: user_turn_id.into(),
        task_id: task_id.clone(),
        execution_scope_id: scope.clone(),
        capabilities: vec![AuthorizedCapability {
            tool_id: TOOL_ID.into(),
            effect: ToolEffect::WriteWorkspace,
        }],
        expires_at: None,
    }, chats).map_err(|e| format!("write task origin rejected: {e:?}"))?;
    policy.grant(CapabilityGrant {
        tool_id: TOOL_ID.into(),
        effect: ToolEffect::WriteWorkspace,
        scope_id: scope.clone(),
        expires_at: None,
    }).map_err(|e| format!("native write capability refused: {e:?}"))?;
    let call = ToolInvocation {
        call_id: effect_call_id.into(),
        tool_id: TOOL_ID.into(),
        workspace_id: Some(scope.clone()),
        input: ProtoPayload {
            type_url: REQUEST_TYPE.into(),
            bytes: proto::CeWriteRequest {
                relative_path: proposal.file_name.clone(),
                data: proposal.content.as_bytes().to_vec(),
            }.encode_to_vec(),
        },
    };
    let fence = ExactCreateNewPolicy {
        host_policy: &policy,
        expected_target: &proposal.file_name,
        expected_sha256: &sha,
    };
    let result = authority.dispatch_bound_durable(
        &registry, effects, call,
        BoundCallOrigin {
            conversation_id: chat_id.into(),
            user_turn_id: user_turn_id.into(),
            task_id: task_id.clone(),
        },
        &fence,
    ).await;
    let _ = authority.revoke_task(&task_id);
    let _ = policy.revoke_scope(&scope);
    match result {
        Ok(output) => {
            let reply = proto::CeWriteResult::decode(output.payload.bytes.as_slice())
                .map_err(|_| "native create-new result unreadable")?;
            if !reply.committed || reply.bytes_written != proposal.content.len() as u64 {
                return Err("native write receipt differs from approved content".into());
            }
            chats.append_progress(
                chat_id, user_turn_id,
                &format!("Created new file: {} ({} bytes). SHA-256: {}",
                    target.display(), reply.bytes_written, sha),
            )?;
            chats.complete(chat_id, user_turn_id)?;
            Ok(CreatedFile {
                file_name: proposal.file_name.clone(),
                exact_target: target.to_string_lossy().into_owned(),
                bytes_written: reply.bytes_written,
                sha256: sha,
                effect_call_id: effect_call_id.into(),
            })
        }
        Err(error) => {
            // The effect might have happened! Do not retry without journal
            // reconciliation or issue a second automatic approval.
            let _ = chats.append_progress(
                chat_id, user_turn_id,
                "Create-new did not return a verified success receipt. The effect may be uncertain; inspect Write Recovery. No retry was performed.",
            );
            let _ = chats.complete(chat_id, user_turn_id);
            Err(format!("native create-new unconfirmed: {error:?}; do not retry blindly"))
        }
    }
}

/// Read-only, bounded recovery observation for the app-owned Lu Drafts
/// directory. Matching bytes are NOT proof of which process published them.
pub fn inspect_recovery_target(
    drafts: &Path, approved_name: &str, expected_bytes: u32, expected_sha: &str,
) -> &'static str {
    if approved_name.is_empty()
        || approved_name.len() > 255
        || approved_name == "." || approved_name == ".."
        || approved_name.starts_with(".lu-stage-")
        || approved_name.chars().any(|c| matches!(c, '/' | '\\' | ':' | '\0'))
        || expected_bytes as usize > crate::write_proposal::MAX_WRITE_PREVIEW_BYTES
        || expected_sha.len() != 64 {
        return "INVALID_EVIDENCE";
    }
    use std::io::Read;
    let path = drafts.join(approved_name);
    match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return "MISSING",
        Err(_) => return "NOT_READABLE",
        Ok(_) => {}
    }
    let mut file = match crate::native_host_read::open_verified_selected_file(&path) {
        Ok(file) => file,
        Err(_) => return "NOT_ORDINARY_FILE",
    };
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(_) => return "NOT_READABLE",
    };
    if metadata.len() != u64::from(expected_bytes) {
        return "DIFFERENT_BYTES";
    }
    let mut bytes = Vec::with_capacity(expected_bytes as usize + 1);
    if (&mut file).take(u64::from(expected_bytes) + 1).read_to_end(&mut bytes).is_err() {
        return "NOT_READABLE";
    }
    if bytes.len() != expected_bytes as usize {
        return "DIFFERENT_BYTES";
    }
    if format!("{:x}", Sha256::digest(&bytes)) == expected_sha {
        "MATCHES_APPROVED_BYTES"
    } else {
        "DIFFERENT_BYTES"
    }
}
