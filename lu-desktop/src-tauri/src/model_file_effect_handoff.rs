//! P0-TOOLS-02 — one-way model text-draft → EXISTING reviewed native effect
//! queues. This module dispatches NO filesystem operation; it only stages the
//! original bounded assistant bytes for the existing exact-path/hash/approval
//! ledgers. The WebView can neither fabricate draft bytes nor choose a path.
use super::*;
use crate::native_action_proposal::{ActionProposalView,NativeAction};

/// A completed-model native action was staged against one original HUMAN
/// prompt. A later operator can accept the review suggestion, never reuse an
/// old human authority grant. Any bridge failure consumes the inert suggestion
/// and requires a new original human request.
pub(super) fn stage_existing_text_review(
    app:&AppHandle,
    state:&Arc<DesktopState>,
    chat_id:&str,
    model_generation:&str,
    proposal:&ActionProposalView,
)->Result<(),String>{
    let operation=match proposal.action.as_str(){
        "create_text_file"=>NativeAction::CreateTextFile,
        "edit_text_file"=>NativeAction::EditTextFile,
        _=>return Ok(()),
    };
    let original=state.chats.trusted_user_turn_text(chat_id,model_generation)
        .ok_or("Original human request not authenticated")?;
    if state.chats.latest_authenticated_user_turn_id(chat_id).as_deref()
        !=Some(model_generation)
        ||native_action_proposal::classify_current_user_action(&original)
            !=Some(operation){
        return Err("Original current human action no longer matches this model draft".into())
    }
    let selected=proposal.text_file_draft.as_ref()
        .ok_or("No complete whole-file text draft was recorded")?;
    if proposal.document_draft.is_some()||proposal.office_patch.is_some()
        ||proposal.office_batch.is_some(){
        return Err("Text draft cannot contain another proposed effect".into())
    }
    let id=next_id(if operation==NativeAction::CreateTextFile{"pw-"}else{"pe-"});
    let payload=serde_json::json!({
        "kind":if operation==NativeAction::CreateTextFile{
            "propose_create_file"
        }else{"propose_edit_file"},
        "file_name":selected.file_name,
        "purpose":proposal.purpose,
        "content":selected.content,
    }).to_string();
    if operation==NativeAction::CreateTextFile{
        let staged=state.write_proposals.stage(&id,chat_id,model_generation,&payload)
            .map_err(|e|format!("Native create review rejected: {e:?}"))?;
        if let Err(e)=state.write_approval_ledger.stage(
            &id,chat_id,model_generation,&staged.file_name,
            staged.bytes(),&staged.digest()
        ){
            let _=state.write_proposals.dismiss(&id,chat_id);
            return Err(format!("Durable create review unavailable: {e:?}"))
        }
        let format=lu_core::write_format::identify_text_format(
            &staged.file_name,&staged.content
        ).map(lu_core::write_format::format_label)
         .map_err(|_|"Text format no longer admissible")?;
        let receipt_digest=staged.digest();
        let proposal_bytes=staged.bytes();
        let event=WriteProposalEvent{
            chat_id:chat_id.into(),proposal_id:id.clone(),
            file_name:staged.file_name,
            purpose:staged.purpose,
            content:staged.content,
            bytes:proposal_bytes,
            sha256:receipt_digest,
            file_format:format.into(),
        };
        if app.emit("lu-write-proposal",event).is_err(){
            let _=state.write_approval_ledger.deny(&id,chat_id);
            let _=state.write_proposals.dismiss(&id,chat_id);
            return Err("Native create review UI unavailable; no effect executed".into())
        }
    }else{
        let staged=state.edit_proposals.stage(&id,chat_id,model_generation,&payload)
            .map_err(|e|format!("Native exact edit review rejected: {e:?}"))?;
        if let Err(e)=state.edit_approval_ledger.stage(
            &id,chat_id,model_generation,&staged.file_name,
            staged.bytes(),&staged.digest()
        ){
            let _=state.edit_proposals.dismiss(&id,chat_id);
            return Err(format!("Durable edit review unavailable: {e:?}"))
        }
        let event=EditProposalEvent{
            chat_id:chat_id.into(),proposal_id:id.clone(),
            file_name:staged.file_name,purpose:staged.purpose,
            content:staged.content,
        };
        if app.emit("lu-edit-proposal",event).is_err(){
            let _=state.edit_approval_ledger.deny(&id,chat_id);
            let _=state.edit_proposals.dismiss(&id,chat_id);
            return Err("Native edit review UI unavailable; no effect executed".into())
        }
    }
    Ok(())
}
