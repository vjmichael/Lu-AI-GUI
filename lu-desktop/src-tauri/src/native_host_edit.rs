//! Trusted desktop host executor for one fully reviewed existing-file edit.
//! This code is not callable with paths or bytes from the WebView. The caller
//! holds an immutable host-selected snapshot and a pinned directory handle.
use crate::{chat_store::ChatStore, edit_proposal::SelectedEdit};
use lu_core::{
    contracts::{ProtoPayload,ToolEffect,ToolInvocation},
    conversation_authority::{AuthorizedCapability,BoundCallOrigin,
        ConversationAuthority,TaskAuthority},
    journal::EffectJournal,
    native_edit_existing::{register_edit_tools_pinned,ExactEditPolicy,
        EDIT_TOOL_ID,EDIT_REQUEST_TYPE,EDIT_RESULT_TYPE},
    policy::{CapabilityGrant,PolicyEngine},
    registry::NativeToolRegistry,
    wire::proto,
};
use prost::Message;
use serde::Serialize;

#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct EditedFile {
    pub file_name:String,
    pub exact_target:String,
    pub bytes_written:u64,
    pub original_sha256:String,
    pub replacement_sha256:String,
    pub effect_call_id:String,
}
pub async fn execute_approved_edit_pinned(
    chats:&ChatStore, effects:&EffectJournal, chat_id:&str, user_turn_id:&str,
    effect_call_id:&str, selected:&SelectedEdit
) -> Result<EditedFile,String> {
    let filename=&selected.proposal.file_name;
    if selected.plan.filename!=*filename ||
        selected.original.len()!=selected.plan.original_bytes ||
        selected.proposal.content.len()!=selected.plan.replacement_bytes ||
        selected.reviewed.host_relative_path!=*filename {
        return Err("reviewed edit snapshot is inconsistent".into());
    }
    let target=selected.directory.join(filename);
    // The trusted host creates this persisted human turn only in response to
    // the native confirmation dialog; a model's old generation is no grant.
    chats.append_user_with_turn_id(chat_id,user_turn_id,&format!(
        "Approved editing selected file {} (original SHA-256 {}; replacement SHA-256 {}; {} bytes).",
        target.display(),selected.plan.expected_original_sha256,
        selected.plan.replacement_sha256,selected.proposal.bytes()
    ))?;
    let registry=NativeToolRegistry::new();
    register_edit_tools_pinned(&registry,selected.directory_cap.clone())
        .map_err(|e|format!("native edit registry unavailable: {e:?}"))?;
    let authority=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task=format!("edit-{user_turn_id}");
    let scope=format!("edit-scope-{user_turn_id}");
    authority.open_user_task(TaskAuthority {
        conversation_id:chat_id.into(),
        user_turn_id:user_turn_id.into(),
        task_id:task.clone(),execution_scope_id:scope.clone(),
        capabilities:vec![AuthorizedCapability {
            tool_id:EDIT_TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        }],expires_at:None
    },chats).map_err(|e|format!("edit user turn not authenticated: {e:?}"))?;
    policy.grant(CapabilityGrant {
        tool_id:EDIT_TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:scope.clone(),expires_at:None
    }).map_err(|e|format!("edit policy rejected: {e:?}"))?;
    let call=ToolInvocation {
        call_id:effect_call_id.into(),tool_id:EDIT_TOOL_ID.into(),
        workspace_id:Some(scope),
        input:ProtoPayload {
            type_url:EDIT_REQUEST_TYPE.into(),
            bytes:proto::EditExistingRequest {
                relative_path:filename.clone(),
                expected_original_sha256:selected.plan.expected_original_sha256.clone(),
                replacement:selected.proposal.content.as_bytes().to_vec(),
                reviewed_approval_sha256:selected.reviewed.approval_sha256.clone(),
            }.encode_to_vec(),
        },
    };
    let fence=ExactEditPolicy {
        host_policy:&policy,
        plan:&selected.plan,reviewed:&selected.reviewed,
        approved_original:&selected.original,
        approved_replacement:selected.proposal.content.as_bytes(),
    };
    let result=authority.dispatch_bound_durable(
        &registry,effects,call,
        BoundCallOrigin {
            conversation_id:chat_id.into(),user_turn_id:user_turn_id.into(),
            task_id:task.clone(),
        },&fence
    ).await;
    let _=authority.revoke_task(&task);
    let _=policy.revoke_scope(&format!("edit-scope-{user_turn_id}"));
    match result {
        Ok(output)=>{
            if output.payload.type_url!=EDIT_RESULT_TYPE {
                return Err("native edit output contract mismatch; inspect recovery".into());
            }
            let reply=proto::EditExistingResult::decode(output.payload.bytes.as_slice())
                .map_err(|_|"native edit receipt unreadable; inspect recovery")?;
            if !reply.committed || reply.backup_id!=effect_call_id ||
                reply.bytes_written!=selected.proposal.bytes() as u64 ||
                reply.original_sha256!=selected.plan.expected_original_sha256 ||
                reply.replacement_sha256!=selected.plan.replacement_sha256 {
                return Err("native edit receipt inconsistent with approved bytes".into());
            }
            chats.append_progress(chat_id,user_turn_id,&format!(
                "Edited {} ({} bytes). Original bytes retained in Lu transaction backup; effect {}.",
                target.display(),reply.bytes_written,effect_call_id
            ))?;
            chats.complete(chat_id,user_turn_id)?;
            Ok(EditedFile {
                file_name:filename.clone(),
                exact_target:target.to_string_lossy().into_owned(),
                bytes_written:reply.bytes_written,
                original_sha256:reply.original_sha256,
                replacement_sha256:reply.replacement_sha256,
                effect_call_id:effect_call_id.into(),
            })
        }
        Err(error)=>{
            let _=chats.append_progress(chat_id,user_turn_id,
                "Edit did not return verified success. The target may have changed; inspect Edit Recovery. Never retry an uncertain edit automatically.");
            let _=chats.complete(chat_id,user_turn_id);
            Err(format!("native edit unconfirmed: {error:?}; reconcile before any new attempt"))
        }
    }
}
