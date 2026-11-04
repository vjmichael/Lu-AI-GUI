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


/// Independently authorized undo. The earlier edit approval is NOT reused;
/// this produces a new authenticated human turn, scope and effect ID.
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct RestoredFile {
    pub file_name:String,
    pub exact_target:String,
    pub restored_bytes:u64,
    pub restored_sha256:String,
    pub original_edit_id:String,
    pub rollback_effect_id:String,
}
pub async fn execute_approved_rollback_pinned(
    chats:&ChatStore, effects:&EffectJournal, chat_id:&str, user_turn_id:&str,
    rollback_effect_id:&str, selected:&crate::rollback_review::SelectedRollback
) -> Result<RestoredFile,String> {
    use lu_core::native_edit_existing::{
        inspect_rollback_candidate,verified_edit_receipt,ExactRollbackPolicy,
        ROLLBACK_TOOL_ID,ROLLBACK_REQUEST_TYPE,ROLLBACK_RESULT_TYPE
    };
    let r=&selected.record;
    let p=&selected.preview;
    if r.chat_id!=chat_id || r.original_edit_id!=p.original_edit_id ||
       r.filename!=p.file_name ||
       selected.directory.join(&r.filename).to_string_lossy()!=r.target ||
       r.expected_current_sha256!=p.replacement_sha256 ||
       r.restored_sha256!=p.original_sha256 ||
       r.restore_bytes!=p.restore_text.len() as u64 {
        return Err("trusted rollback snapshot inconsistent".into());
    }
    // Require the original edit's exact persisted success receipt. A guessed
    // or UNCERTAIN transaction ID can never become an undo authority.
    let prior=effects.get(&r.original_edit_id)
        .map_err(|e|format!("original effect journal unavailable: {e:?}"))?
        .ok_or("original edit effect not found")?;
    if !verified_edit_receipt(&prior,&r.original_edit_id,&r.restored_sha256,
        &r.expected_current_sha256,p.current_text.len() as u64) {
        return Err("original edit lacks verified exact applied receipt".into());
    }
    // Re-read both source and original backup on the pinned capability just
    // before granting any effect. The native tool separately revalidates them
    // after the effect's EXECUTING fence.
    let fresh=inspect_rollback_candidate(
        &selected.directory_cap,&r.filename,&r.original_edit_id
    ).map_err(|e|format!("rollback source or backup changed: {e:?}"))?;
    if fresh!=*p {
        return Err("rollback bytes changed since full human review".into());
    }
    chats.append_user_with_turn_id(chat_id,user_turn_id,&format!(
        "Approved separately restoring the verified original of {}. Original edit {}. Current SHA-256 {}; restore SHA-256 {}; {} bytes. New rollback effect {}.",
        r.target,r.original_edit_id,r.expected_current_sha256,r.restored_sha256,
        r.restore_bytes,rollback_effect_id
    ))?;
    let registry=NativeToolRegistry::new();
    register_edit_tools_pinned(&registry,selected.directory_cap.clone())
        .map_err(|e|format!("rollback native registry unavailable: {e:?}"))?;
    let authority=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task=format!("rollback-{user_turn_id}");
    let scope=format!("rollback-scope-{user_turn_id}");
    authority.open_user_task(TaskAuthority {
        conversation_id:chat_id.into(),user_turn_id:user_turn_id.into(),
        task_id:task.clone(),execution_scope_id:scope.clone(),
        capabilities:vec![AuthorizedCapability {
            tool_id:ROLLBACK_TOOL_ID.into(),effect:ToolEffect::WriteWorkspace
        }],expires_at:None
    },chats).map_err(|e|format!("rollback turn not authenticated: {e:?}"))?;
    policy.grant(CapabilityGrant{
        tool_id:ROLLBACK_TOOL_ID.into(),effect:ToolEffect::WriteWorkspace,
        scope_id:scope.clone(),expires_at:None
    }).map_err(|e|format!("rollback policy rejected: {e:?}"))?;
    let call=ToolInvocation{
        call_id:rollback_effect_id.into(),tool_id:ROLLBACK_TOOL_ID.into(),
        workspace_id:Some(scope),
        input:ProtoPayload {
            type_url:ROLLBACK_REQUEST_TYPE.into(),
            bytes:proto::EditRollbackRequest{
                relative_path:r.filename.clone(),original_edit_id:r.original_edit_id.clone(),
                expected_current_sha256:r.expected_current_sha256.clone(),
                expected_restored_sha256:r.restored_sha256.clone()
            }.encode_to_vec()
        }
    };
    let fence=ExactRollbackPolicy{
        host_policy:&policy,expected_target:&r.filename,
        expected_original_edit_id:&r.original_edit_id,
        expected_current_sha256:&r.expected_current_sha256,
        expected_restored_sha256:&r.restored_sha256
    };
    let result=authority.dispatch_bound_durable(
        &registry,effects,call,BoundCallOrigin{
            conversation_id:chat_id.into(),user_turn_id:user_turn_id.into(),
            task_id:task.clone()
        },&fence
    ).await;
    let _=authority.revoke_task(&task);
    let _=policy.revoke_scope(&format!("rollback-scope-{user_turn_id}"));
    match result {
        Ok(out)=>{
            if out.payload.type_url!=ROLLBACK_RESULT_TYPE {
                return Err("rollback effect receipt type unexpected; inspect recovery".into());
            }
            let reply=proto::EditRollbackResult::decode(out.payload.bytes.as_slice())
                .map_err(|_|"rollback effect receipt unreadable; inspect recovery")?;
            if !reply.committed || reply.original_edit_id!=r.original_edit_id ||
                reply.restored_sha256!=r.restored_sha256 ||
                reply.restored_bytes!=r.restore_bytes {
                return Err("rollback receipt differs from exact approval".into());
            }
            chats.append_progress(chat_id,user_turn_id,&format!(
                "Restored original text to {} ({} bytes). Undo transaction {} was separately authorized and journaled.",
                r.target,reply.restored_bytes,rollback_effect_id
            ))?;
            chats.complete(chat_id,user_turn_id)?;
            Ok(RestoredFile{
                file_name:r.filename.clone(),exact_target:r.target.clone(),
                restored_bytes:reply.restored_bytes,
                restored_sha256:reply.restored_sha256,
                original_edit_id:r.original_edit_id.clone(),
                rollback_effect_id:rollback_effect_id.into(),
            })
        }
        Err(error)=>{
            let _=chats.append_progress(chat_id,user_turn_id,
                "Restoration did not return a verified success receipt. It may have changed the file; inspect the rollback journal and artifacts. Never retry the same or new restore automatically.");
            let _=chats.complete(chat_id,user_turn_id);
            Err(format!("rollback unconfirmed: {error:?}; inspect recovery before any further changes"))
        }
    }
}
