//! CP-06E-R3 ordinary Send adapter for the original TOOLS-10 native services.
use std::sync::{Arc,Mutex};
use lu_core::{conversation_authority::TrustedUserTurnVerifier,inference_host::Cancellation,model_turn_journal::{ModelAuditKind,ModelTurnJournal},model_web_research_turn::{ApprovedWebTurn,Request},supervised_tool_generation::SupervisorModelStep};
use tauri::Emitter;
use tauri_plugin_dialog::{DialogExt,MessageDialogButtons};

pub(crate) fn try_model_web_research(app:&tauri::AppHandle,state:&Arc<crate::DesktopState>,sid:&str,chat:&str,gid:&str,original:&str,ticket:&crate::generation_queue::Ticket)->Result<Option<String>,String>{
    let Some(request)=Request::parse(original) else{return Ok(None)};
    let verifier=crate::turn_authority::PersistedHumanTurn::current_chat(&state.chats,chat,gid,original);
    let cancel=Cancellation::default();let watermark=state.chats.p0_auth_requested_watermark(chat)?;
    let checkpoint=||->Result<(),String>{
        ticket.verify_active_foreground(sid,chat,gid)?;
        if cancel.is_cancelled()||ticket.cancelled()||state.chats.p0_auth_requested_watermark(chat)?!=watermark||!verifier.verify_user_turn(chat,gid){return Err("Network request cancelled or revoked".into())}
        state.observe.deny_observation_network_egress(chat)?;
        if state.supervisor.status().map_err(|_|"Model unavailable")?.session_id.as_deref()!=Some(sid){return Err("Model changed during retrieval".into())}
        Ok(())
    };
    checkpoint()?;
    let journal=ModelTurnJournal::open(&state.catalog_path.with_file_name("lu-model-web-research-audit.sqlite")).map_err(|_|"Network audit unavailable")?;
    let step=Arc::new(Mutex::new(None));
    {let mut all=state.reader_generations.lock().map_err(|_|"Network cancellation unavailable")?;if all.contains_key(gid){return Err("Duplicate network request ownership".into())}all.insert(gid.into(),crate::ReaderRunControl{cancellation:cancel.clone(),current_step:Arc::clone(&step),session_id:sid.into()});}
    struct Control<'a>{state:&'a crate::DesktopState,id:&'a str}
    impl Drop for Control<'_>{fn drop(&mut self){let _=self.state.reader_generations.lock().map(|mut all|all.remove(self.id));}}
    let _control=Control{state,id:gid};
    let progress=crate::native_turn_lifecycle::NativeTurnProgress::new(app,chat,gid,&step);
    let _=app.emit("lu-generation-start",crate::GenerationEvent{chat_id:chat.into(),generation_id:gid.into(),phase:"running",position:0});
    let complete=|answer:String|->Result<Option<String>,String>{checkpoint()?;ticket.commit_completed(sid,||{state.chats.append_progress(chat,gid,&answer)?;state.chats.complete_reviewed_if_not_revoked(chat,gid,watermark)})?;Ok(Some(answer))};
    // The resource must exist before asking for permission; never fall back to
    // PATH/system curl, another provider, the Browser or a model-only answer.
    if state.research.pinned_transfer_client().is_err(){return complete("Web access is unavailable in this installation. No web request was made.".into())}
    let approved=app.dialog().message(request.review()).title("Allow web access?").buttons(MessageDialogButtons::OkCancelCustom("Allow once".into(),"Cancel".into())).blocking_show();
    checkpoint()?;
    if !approved{
        journal.append(chat,gid,ModelAuditKind::UserRequest,original,None).map_err(|_|"Network audit unavailable")?;
        journal.append(chat,gid,ModelAuditKind::Rejected,"HUMAN_DECLINED_NETWORK",None).map_err(|_|"Network audit unavailable")?;
        return complete("Web request declined. No network request was made.".into());
    }
    let curl=state.research.pinned_transfer_client().map_err(|_|"Reviewed HTTPS runtime changed")?;
    // Record the actual host review decision before the model may dispatch.
    // This is a descriptive audit event, never a durable grant or replay token.
    journal.append(chat,gid,ModelAuditKind::HostRouteSelected,"HOST_NATIVE_APPROVAL_ONE_EXACT_PUBLIC_HTTPS_REQUEST",None).map_err(|_|"Network audit unavailable")?;
    let mut native=ApprovedWebTurn::new_after_review(&verifier,chat,gid,original,&curl,cancel.clone())?;
    let live=||{checkpoint()?;if !state.research.available(){return Err("Reviewed HTTPS runtime changed".into())}Ok(())};
    let mut model=SupervisorModelStep::new(&state.supervisor,sid.into(),crate::next_id("mweb-")).map_err(|_|"Network model unavailable")?.with_step_start_observer(progress.model_observer());
    let result=tauri::async_runtime::block_on(native.run(&mut model,&verifier,&journal,&live,&progress.dispatch_observer()));
    *step.lock().map_err(|_|"Network active step unavailable")?=None;
    let answer=match result{Ok(report)=>report.answer,Err(_)=>"I couldn’t retrieve a usable response. No further web request was made.".into()};
    complete(answer)
}
