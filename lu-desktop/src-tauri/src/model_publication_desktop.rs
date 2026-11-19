//! CP-06D-R2 trusted desktop boundary. Private review, native confirmation,
//! authenticated original turn. No WebView or model consent/paths.
use std::sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}};
use std::time::Duration;
use lu_core::{conversation_authority::TrustedUserTurnVerifier,inference_host::Cancellation,model_publication_turn::{Action,Intent},model_turn_journal::ModelTurnJournal,native_archive_tree::ArchiveCancellation,supervised_tool_generation::SupervisorModelStep};
use tauri::Emitter;
use tauri_plugin_dialog::{DialogExt,MessageDialogButtons};
struct ArchiveStop{done:Arc<AtomicBool>,thread:Option<std::thread::JoinHandle<()>>}
impl ArchiveStop{
    fn new(cancel:Cancellation,archive:ArchiveCancellation)->Self{
        let done=Arc::new(AtomicBool::new(false));let stop=Arc::clone(&done);
        let thread=std::thread::spawn(move||{while !stop.load(Ordering::Acquire){if cancel.is_cancelled(){archive.cancel();return}std::thread::sleep(Duration::from_millis(20));}});
        Self{done,thread:Some(thread)}
    }
}
impl Drop for ArchiveStop{fn drop(&mut self){self.done.store(true,Ordering::Release);if let Some(t)=self.thread.take(){let _=t.join();}}}
// Native message boxes are not a scrollable document viewer. Paginate all
// visible review text without splitting a UTF-8 scalar or silently truncating.
fn review_pages(s:&str)->Vec<String>{
    let mut pages=Vec::new();let mut page=String::new();let mut lines=0;
    for ch in s.chars(){if page.chars().count()>=660||lines>=10{pages.push(std::mem::take(&mut page));lines=0;}if ch=='\n'{lines+=1;}page.push(ch);}
    if !page.is_empty(){pages.push(page)}pages
}
pub(crate) fn try_model_publication(app:&tauri::AppHandle,state:&Arc<crate::DesktopState>,sid:&str,chat:&str,gid:&str,original:&str,ticket:&crate::generation_queue::Ticket)->Result<Option<String>,String>{
    let Some(intent)=Intent::parse(original) else{return Ok(None)};
    if lu_core::capability_router::user_declines_tools(original){return Ok(None)}
    let verifier=crate::turn_authority::PersistedHumanTurn::current_chat(&state.chats,chat,gid,original);
    let cancellation=Cancellation::default();let watermark=state.chats.p0_auth_requested_watermark(chat)?;
    let checkpoint=||->Result<(),String>{ticket.verify_active_foreground(sid,chat,gid)?;if cancellation.is_cancelled()||ticket.cancelled()||state.chats.p0_auth_requested_watermark(chat)?!=watermark||!verifier.verify_user_turn(chat,gid){return Err("Cancelled original publication".into())}if state.supervisor.status().map_err(|_|"Model unavailable")?.session_id.as_deref()!=Some(sid){return Err("Model changed during publication".into())}Ok(())};
    checkpoint()?;let journal=ModelTurnJournal::open(&state.catalog_path.with_file_name("lu-model-publication-audit.sqlite")).map_err(|_|"Publication audit unavailable")?;let step=Arc::new(Mutex::new(None));
    {let mut all=state.reader_generations.lock().map_err(|_|"Publication cancellation unavailable")?;if all.contains_key(gid){return Err("Duplicate publication ownership".into())}all.insert(gid.into(),crate::ReaderRunControl{cancellation:cancellation.clone(),current_step:Arc::clone(&step),session_id:sid.into()});}
    struct Control<'a>{state:&'a crate::DesktopState,id:&'a str}
    impl Drop for Control<'_>{fn drop(&mut self){let _=self.state.reader_generations.lock().map(|mut all|all.remove(self.id));}}
    let _control=Control{state,id:gid};let progress=crate::native_turn_lifecycle::NativeTurnProgress::new(app,chat,gid,&step);let _=app.emit("lu-generation-start",crate::GenerationEvent{chat_id:chat.into(),generation_id:gid.into(),phase:"running",position:0});
    let complete=|answer:String|->Result<Option<String>,String>{checkpoint()?;ticket.commit_completed(sid,||{state.chats.append_progress(chat,gid,&answer)?;state.chats.complete_reviewed_if_not_revoked(chat,gid,watermark)})?;Ok(Some(answer))};
    let mut model=SupervisorModelStep::new(&state.supervisor,sid.into(),crate::next_id("mpub-")).map_err(|_|"Publication model unavailable")?.with_step_start_observer(progress.model_observer());
    let proposed=intent.propose(chat,gid,&verifier,&mut model,&cancellation,&journal,&checkpoint);*step.lock().map_err(|_|"Publication step unavailable")?=None;
    let Some(proposal)=proposed? else{return complete("No publication was authorized or performed. I could not select the requested action.".into())};
    let effect=crate::next_id("g-");let review_id=crate::next_id("pub-");let mut run=None;let mut stop=None;
    let review=if proposal.action().is_member(){
        checkpoint()?;let Some(src)=app.dialog().file().blocking_pick_file() else{proposal.deny(&journal)?;return complete("Archive selection cancelled. No files extracted.".into())};let src=src.into_path().map_err(|_|"Invalid archive selection")?;let source_display=src.to_str().ok_or("Archive path cannot be safely displayed")?;let (name,source_parent,source)=crate::archive_desktop::physical_file(&src)?;if !proposal.matches_source(&name){proposal.deny(&journal)?;return complete("Selected archive differs from the requested file. No files extracted.".into())}
        checkpoint()?;let Some(dest)=app.dialog().file().blocking_pick_folder() else{proposal.deny(&journal)?;return complete("Destination selection cancelled. No files extracted.".into())};let dest=dest.into_path().map_err(|_|"Invalid destination")?;if dest.to_str().is_none(){return Err("Destination cannot be safely displayed".into())}let root=crate::archive_desktop::physical_folder(&dest)?;
        let guard=state.archive_runs.register(chat,&review_id)?;let cancel=guard.cancellation();stop=Some(ArchiveStop::new(cancellation.clone(),cancel.clone()));checkpoint()?;let tasks=Arc::new(lu_core::tasks::TaskStore::open(&state.catalog_path.with_file_name(format!("lu-reviewed-task-{gid}.sqlite"))).map_err(|_|"Archive task journal unavailable")?);tasks.bind_reviewed_human_turn(chat,gid).map_err(|_|"Archive original turn unavailable")?;
        let processes=Arc::new(lu_core::native_background_process::BackgroundProcessSupervisor::new_for_reviewed_host(Arc::clone(&tasks),state.catalog_path.parent().ok_or("Host profile missing")?.to_path_buf()));let plan=lu_core::native_selected_archive_process::WorkerPlan::new(processes,tasks,std::env::current_exe().map_err(|_|"Archive worker unavailable")?,source_parent,dest.clone(),&source,&root,&effect,gid,cancel)?;
        let member=proposal.member_path().ok_or("Member unavailable")?;let leaf=member.rsplit('/').next().ok_or("Member unavailable")?;let target=format!("Lu-{}-{leaf}",&effect[2..14]);let display=dest.join(&target).to_string_lossy().into_owned();let r=proposal.review_member(source,&name,source_display,root,&target,&display,plan,&journal)?;run=Some(guard);r
    }else if proposal.action()==Action::Archive{
        checkpoint()?;let Some(src)=app.dialog().file().blocking_pick_file() else{proposal.deny(&journal)?;return complete("Archive selection cancelled. No files extracted.".into())};
        let src=src.into_path().map_err(|_|"Invalid archive selection")?;if src.to_str().is_none(){return Err("Archive path cannot be safely displayed".into())}if !src.file_name().and_then(|n|n.to_str()).is_some_and(|n|proposal.matches_source(n)){proposal.deny(&journal)?;return complete("Selected archive does not match the requested file. No files extracted.".into())}
        checkpoint()?;let Some(dest)=app.dialog().file().blocking_pick_folder() else{proposal.deny(&journal)?;return complete("Destination selection cancelled. No files extracted.".into())};let dest=dest.into_path().map_err(|_|"Invalid destination")?;if dest.to_str().is_none(){return Err("Destination path cannot be safely displayed".into())}
        checkpoint()?;let guard=state.archive_runs.register(chat,&review_id)?;let cancel=guard.cancellation();stop=Some(ArchiveStop::new(cancellation.clone(),cancel.clone()));
        let queue=crate::archive_desktop::ArchiveReviewQueue::default();queue.stage_supervised(chat,&review_id,&src,&dest,&cancel,&state.background_processes,&state.background_tasks)?;checkpoint()?;
        let pinned=queue.consume(chat,&review_id)?;let r=pinned.into_model_publication(proposal,&effect,cancel,Arc::clone(&state.background_processes),Arc::clone(&state.background_tasks),&journal)?;run=Some(guard);r
    }else if proposal.action().is_revision(){
        checkpoint()?;let Some(src)=app.dialog().file().blocking_pick_file() else{proposal.deny(&journal)?;return complete("Revision cancelled. Original file unchanged.".into())};let src=src.into_path().map_err(|_|"Invalid Office selection")?;let source_display=src.to_str().ok_or("Office path cannot be safely displayed")?;
        let (name,source)=crate::office_revision_desktop::open_parent(&src)?;if !proposal.matches_source(&name){proposal.deny(&journal)?;return complete("Selected document differs from the requested file. No revision created.".into())}
        checkpoint()?;let Some(dest)=app.dialog().file().blocking_pick_folder() else{proposal.deny(&journal)?;return complete("Destination selection cancelled. Original unchanged.".into())};let dest=dest.into_path().map_err(|_|"Invalid destination")?;if dest.to_str().is_none(){return Err("Destination cannot be safely displayed".into())}let root=crate::office_revision_desktop::open_folder(&dest)?;
        let target=crate::office_revision_desktop::output_name(&name,&effect)?;let display=dest.join(&target).to_string_lossy().into_owned();proposal.review_revision(source,&name,source_display,root,&target,&display,&journal)?
    }else{
        checkpoint()?;let Some(dest)=app.dialog().file().blocking_pick_folder() else{proposal.deny(&journal)?;return complete("Destination selection cancelled. No document created.".into())};let dest=dest.into_path().map_err(|_|"Invalid destination")?;if dest.to_str().is_none(){return Err("Destination path cannot be safely displayed".into())}let dir=crate::archive_desktop::physical_folder(&dest)?;
        let target=format!("Lu-{}.{}",&effect[2..14],proposal.action().extension());let display=dest.join(&target).to_string_lossy().into_owned();proposal.review_document(dir,&target,&display,&journal)?
    };
    let pages=review_pages(review.modal());let count=pages.len();
    for (i,page) in pages.into_iter().enumerate(){checkpoint()?;let ok=app.dialog().message(page).title(format!("Lu — Review output ({}/{count})",i+1)).buttons(MessageDialogButtons::OkCancelCustom("Continue review".into(),"Cancel".into())).blocking_show();checkpoint()?;if !ok{review.deny(&journal)?;return complete("Publication declined. No output was created.".into())}}
    let ok=app.dialog().message(format!("Create the exact reviewed output?\n\n{}\n\nExisting files and folders will never be replaced. Approval is single-use. Stopping after execution begins may not undo an operation.",review.display())).title("Lu — Confirm publication").buttons(MessageDialogButtons::OkCancelCustom("Approve exact output".into(),"Cancel".into())).blocking_show();checkpoint()?;
    if !ok{review.deny(&journal)?;return complete("Publication declined. No output was created.".into())}
    if let Some(manifest)=review.manifest(){state.wave2_recovery.record_archive_intent(chat,&effect,review.display(),review.target(),manifest,"Model-selected, separately approved archive extraction")?;}else{state.wave2_recovery.record_intent(chat,&effect,review.action().tool(),review.display(),review.target(),review.digest(),if review.action().is_member(){"Model-selected, separately approved selected-member extraction"}else if review.action().is_revision(){"Model-selected, separately approved non-destructive Office revision"}else{"Model-selected, separately approved document creation"})?;}
    checkpoint()?;let observe=progress.dispatch_observer();observe(lu_core::agent_tool_loop::ToolDispatchCheckpoint::Entered);
    let result=tauri::async_runtime::block_on(review.execute(true,&effect,&verifier,&state.effects,&journal,&checkpoint));observe(lu_core::agent_tool_loop::ToolDispatchCheckpoint::Returned);drop(stop);drop(run);
    let outcome=result?.ok_or("No confirmed publication receipt")?;let answer=outcome.present(&mut model,&cancellation,&journal,&state.effects,&checkpoint);*step.lock().map_err(|_|"Publication step unavailable")?=None;complete(answer?)
}
#[cfg(test)]
mod tests{use super::*;#[test]fn cp06dr2_review_paginates_all_text(){let text="漢字🦊\n".repeat(500);let pages=review_pages(&text);assert!(pages.len()>1);assert_eq!(pages.concat(),text);assert!(pages.iter().all(|p|p.chars().count()<=660&&p.lines().count()<=11));}}

/// Exact, current-human, chat-owned task read. No scheduler grant or process poll.
pub(crate) fn try_model_background(app:&tauri::AppHandle,state:&Arc<crate::DesktopState>,sid:&str,chat:&str,gid:&str,original:&str,ticket:&crate::generation_queue::Ticket)->Result<Option<String>,String>{
    use lu_core::model_background_observation::{Request,Observation};
    if Request::parse(original).is_none(){return Ok(None)}
    let verifier=crate::turn_authority::PersistedHumanTurn::current_chat(&state.chats,chat,gid,original);
    let cancellation=Cancellation::default();let watermark=state.chats.p0_auth_requested_watermark(chat)?;
    let checkpoint=||->Result<(),String>{ticket.verify_active_foreground(sid,chat,gid)?;if cancellation.is_cancelled()||ticket.cancelled()||state.chats.p0_auth_requested_watermark(chat)?!=watermark||!verifier.verify_user_turn(chat,gid){return Err("Cancelled background observation".into())}if state.supervisor.status().map_err(|_|"Model unavailable")?.session_id.as_deref()!=Some(sid){return Err("Model changed during task observation".into())}Ok(())};
    checkpoint()?;let journal=ModelTurnJournal::open(&state.catalog_path.with_file_name("lu-model-background-audit.sqlite")).map_err(|_|"Task audit unavailable")?;let step=Arc::new(Mutex::new(None));
    {let mut all=state.reader_generations.lock().map_err(|_|"Task cancellation unavailable")?;if all.contains_key(gid){return Err("Duplicate task observation ownership".into())}all.insert(gid.into(),crate::ReaderRunControl{cancellation:cancellation.clone(),current_step:Arc::clone(&step),session_id:sid.into()});}
    struct Control<'a>{state:&'a crate::DesktopState,id:&'a str}
    impl Drop for Control<'_>{fn drop(&mut self){let _=self.state.reader_generations.lock().map(|mut all|all.remove(self.id));}}
    let _control=Control{state,id:gid};let progress=crate::native_turn_lifecycle::NativeTurnProgress::new(app,chat,gid,&step);let _=app.emit("lu-generation-start",crate::GenerationEvent{chat_id:chat.into(),generation_id:gid.into(),phase:"running",position:0});
    let mut observation=Observation::new(&verifier,chat,gid,original,Arc::clone(&state.background_tasks))?;let mut model=SupervisorModelStep::new(&state.supervisor,sid.into(),crate::next_id("mbg-")).map_err(|_|"Task observation model unavailable")?.with_step_start_observer(progress.model_observer());
    let result=tauri::async_runtime::block_on(observation.run(&mut model,&verifier,&cancellation,&journal,&checkpoint,&progress.dispatch_observer()));*step.lock().map_err(|_|"Task observation step unavailable")?=None;
    checkpoint()?;let answer=match result{Ok(report)=>report.answer,Err(_)=>"I could not verify that task's requested status or result in this conversation. No task was started, stopped or modified.".into()};
    ticket.commit_completed(sid,||{state.chats.append_progress(chat,gid,&answer)?;state.chats.complete_reviewed_if_not_revoked(chat,gid,watermark)})?;Ok(Some(answer))
}

/// CP-06E-R2-B1: finite original-human cancellation intent.
/// No PID, executable, path, wildcard, compound command or model-authored
/// target can enter this boundary. Launch remains deliberately disconnected.
fn background_cancel_target(original:&str)->Option<&str>{
    if original.len()>256||!original.is_ascii()||original.chars().any(char::is_control)
        ||lu_core::capability_router::user_declines_tools(original){return None}
    let s=original.trim().trim_end_matches(['.','?','!']);
    let lower=s.to_ascii_lowercase();
    let prefix=["cancel background task ","stop background task "].into_iter()
        .find(|p|lower.starts_with(p))?;
    let target=&s[prefix.len()..];
    let suffix=target.strip_prefix("scan-")?;
    if suffix.is_empty()||target.len()>96||!target.bytes().all(|b|
        b.is_ascii_alphanumeric()||matches!(b,b'-'|b'_')){return None}
    Some(target)
}

#[cfg(any(windows,target_os="linux"))]
pub(crate) fn try_model_background_control(
    app:&tauri::AppHandle,state:&Arc<crate::DesktopState>,sid:&str,chat:&str,gid:&str,
    original:&str,ticket:&crate::generation_queue::Ticket,
)->Result<Option<String>,String>{
    use std::{collections::BTreeSet,time::{SystemTime,Duration}};
    use sha2::{Digest,Sha256};
    use serde_json::{json,Value};
    use lu_core::{
        contracts::{AuthorizationError,ProtoPayload,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
        conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority},
        journal::{EffectState,IndependentOutcomeVerdict},
        model_json_boundary::parse_exact_boundary_object,
        model_turn_journal::ModelAuditKind as K,
        native_background_control::register_bound_background_cancel,
        policy::{CapabilityGrant,PolicyEngine},
        registry::NativeToolRegistry,
        supervised_tool_generation::ModelStepGenerator,
        wire::{self,proto},
    };
    let Some(target)=background_cancel_target(original) else{return Ok(None)};
    const NAME:&str="lu_background_cancel";
    const TOOL:&str="lu.bgcancel.run";
    let verifier=crate::turn_authority::PersistedHumanTurn::current_chat(&state.chats,chat,gid,original);
    let cancel=Cancellation::default();
    let watermark=state.chats.p0_auth_requested_watermark(chat)?;
    let checkpoint=||->Result<(),String>{
        ticket.verify_active_foreground(sid,chat,gid)?;
        if cancel.is_cancelled()||ticket.cancelled()||state.chats.p0_auth_requested_watermark(chat)?!=watermark
            ||!verifier.verify_user_turn(chat,gid){return Err("Cancelled original task control".into())}
        if state.supervisor.status().map_err(|_|"Model unavailable")?.session_id.as_deref()!=Some(sid){
            return Err("Model changed during task control".into())
        }
        Ok(())
    };
    checkpoint()?;
    let journal=ModelTurnJournal::open(&state.catalog_path.with_file_name("lu-model-background-control-audit.sqlite"))
        .map_err(|_|"Task control audit unavailable")?;
    let log=|k:K,p:&str|journal.append(chat,gid,k,p,None).map(|_|()).map_err(|_|"Task control audit unavailable".to_owned());
    let hash=|b:&[u8]|format!("{:x}",Sha256::digest(b));
    if !journal.events_for(chat,gid).map_err(|_|"Task control audit unavailable")?.is_empty(){
        return Err("Original task control already attempted".into())
    }
    let step=Arc::new(Mutex::new(None));
    {let mut all=state.reader_generations.lock().map_err(|_|"Task cancellation unavailable")?;
        if all.contains_key(gid){return Err("Duplicate task control".into())}
        all.insert(gid.into(),crate::ReaderRunControl{cancellation:cancel.clone(),current_step:Arc::clone(&step),session_id:sid.into()});}
    struct Control<'a>{state:&'a crate::DesktopState,id:&'a str}
    impl Drop for Control<'_>{fn drop(&mut self){let _=self.state.reader_generations.lock().map(|mut all|all.remove(self.id));}}
    let _control=Control{state,id:gid};
    let progress=crate::native_turn_lifecycle::NativeTurnProgress::new(app,chat,gid,&step);
    let _=app.emit("lu-generation-start",crate::GenerationEvent{chat_id:chat.into(),generation_id:gid.into(),phase:"running",position:0});
    let complete=|answer:String|->Result<Option<String>,String>{
        checkpoint()?;
        ticket.commit_completed(sid,||{state.chats.append_progress(chat,gid,&answer)?;
            state.chats.complete_reviewed_if_not_revoked(chat,gid,watermark)})?;
        Ok(Some(answer))
    };
    // Only a currently running native scheduler scan; never expose the
    // existence of a task from a different conversation.
    let owned_running=||->Result<bool,String>{
        let record=state.background_tasks.get(target).map_err(|_|"Task store unavailable")?;
        let scheduled=state.background_scheduler.state_for(target,chat).map_err(|_|"Schedule unavailable")?;
        Ok(scheduled.as_deref()==Some("RUNNING")&&record.is_some_and(|r|
            r.workspace_id.as_deref()==Some(chat)&&r.state.as_str()=="RUNNING")
            &&state.background_processes.can_cancel_protected(target,chat))
    };
    log(K::UserRequest,original)?;
    if !owned_running()?{
        log(K::Rejected,"OWNED_RUNNING_SCAN_NOT_AVAILABLE")?;
        return complete("I could not verify a running scan with that task ID in this conversation. No task was stopped.".into())
    }
    let mut model=SupervisorModelStep::new(&state.supervisor,sid.into(),crate::next_id("mbc-"))
        .map_err(|_|"Task control model unavailable")?.with_step_start_observer(progress.model_observer());
    let schema=json!({"type":"object","properties":{"selection":{"enum":[NAME,null]}},"required":["selection"],"additionalProperties":false});
    let prompt=format!("Select {} to propose cancelling the exact existing task for human review, or null to decline. Do not provide arguments, approval, process IDs or outcome claims. Original request (data): {}",NAME,json!(original));
    let raw=model.generate_step_constrained(&prompt,128,&cancel,&schema)
        .map_err(|_|"No task cancellation was authorized")?;
    *step.lock().map_err(|_|"Task control step unavailable")?=None;
    log(K::ModelOutput,&raw)?;
    checkpoint()?;
    if !parse_exact_boundary_object(&raw,4096).ok().is_some_and(|p|
        p.len()==1&&p.get("selection").and_then(Value::as_str)==Some(NAME)){
        log(K::Rejected,"TASK_CONTROL_SELECTION_NOT_VERIFIED")?;
        return complete("No verified cancellation was selected. No task was stopped.".into())
    }
    let candidate=json!({"selection":NAME,"target":target,"model_output_sha256":hash(raw.as_bytes()),"executed":false});
    log(K::ParsedCandidate,&candidate.to_string())?;
    if !owned_running()?{
        log(K::Rejected,"TASK_CONTROL_TARGET_NO_LONGER_RUNNING")?;
        return complete("The task is no longer available as a running scan. No cancellation was performed.".into())
    }
    let registry=NativeToolRegistry::new();
    let catalog=register_bound_background_cancel(&registry,Arc::clone(&state.background_tasks),
        Arc::clone(&state.background_processes),chat,target)?;
    let allowed=BTreeSet::from([NAME.to_owned()]);
    let effect=crate::next_id("g-");
    let call=catalog.encode_call(&allowed,NAME,&effect,Some(chat),&json!({"task_id":target}).to_string())
        .map_err(|_|"Exact task cancellation input unavailable")?;
    let fingerprint=lu_core::journal::fingerprint(&call);
    let review=json!({"tool":TOOL,"target":target,"owner":chat,"original_request_sha256":hash(original.as_bytes()),
        "candidate_sha256":hash(candidate.to_string().as_bytes()),"input_sha256":hash(&call.input.bytes),
        "input_type_url":call.input.type_url,"effect_fingerprint":fingerprint});
    let review_sha=hash(review.to_string().as_bytes());
    log(K::HostRouteSelected,&json!({"phase":"native_preview","review":review,"review_sha256":review_sha,"approved":false}).to_string())?;
    checkpoint()?;
    let ok=app.dialog().message(format!("Stop the running background scan?\n\nTask: {target}\n\nLu will stop only this task in this conversation. Any work already completed is not undone."))
        .title("Lu — Confirm background task cancellation")
        .buttons(MessageDialogButtons::OkCancelCustom("Stop this task".into(),"Keep running".into())).blocking_show();
    checkpoint()?;
    if !ok{
        log(K::Rejected,"HUMAN_DECLINED_TASK_CONTROL")?;
        return complete("Cancellation declined. No task was stopped.".into())
    }
    if !owned_running()?{
        log(K::Rejected,"TASK_CONTROL_TARGET_CHANGED")?;
        return complete("The task is no longer available as a running scan. No cancellation was performed.".into())
    }
    let rows=journal.events_for(chat,gid).map_err(|_|"Task control audit unavailable")?;
    let review_row=rows.get(3).filter(|r|r.kind==K::HostRouteSelected).ok_or("Task control review unavailable")?;
    let checked:Value=serde_json::from_str(&review_row.payload).map_err(|_|"Invalid task control review")?;
    if rows.len()!=4||rows[0].kind!=K::UserRequest||rows[0].payload!=original||
       rows[1].kind!=K::ModelOutput||hash(rows[1].payload.as_bytes())!=hash(raw.as_bytes())||
       rows[2].kind!=K::ParsedCandidate||serde_json::from_str::<Value>(&rows[2].payload).ok()!=Some(candidate)||
       checked["approved"]!=false||checked["phase"]!="native_preview"||
       checked["review_sha256"]!=review_sha||hash(checked["review"].to_string().as_bytes())!=review_sha||
       state.effects.get(&effect).map_err(|_|"Effect history unavailable")?.is_some(){
        return Err("Original task control review changed".into())
    }
    struct Fence<'a>{
        policy:&'a PolicyEngine,v:&'a dyn TrustedUserTurnVerifier,c:&'a str,t:&'a str,
        input:&'a ProtoPayload,live:&'a(dyn Fn()->Result<(),String>+Send+Sync),
    }
    impl ToolAuthorizer for Fence<'_>{
        fn authorize(&self,call:&ToolInvocation,d:&ToolDescriptor)->Result<(),AuthorizationError>{
            if (self.live)().is_err()||!self.v.verify_user_turn(self.c,self.t)||
                call.workspace_id.as_deref()!=Some(self.c)||call.tool_id!="lu.bgcancel.run"||
                d.id!="lu.bgcancel.run"||d.effect!=ToolEffect::ExecuteCode||&call.input!=self.input{
                return Err(AuthorizationError::new("Exact task cancellation not authorized"))
            }
            self.policy.authorize(call,d)
        }
    }
    let auth=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task=format!("background-cancel-{effect}");
    let expires=SystemTime::now().checked_add(Duration::from_secs(120)).ok_or("Cannot bound task control")?;
    auth.open_user_task(TaskAuthority{conversation_id:chat.into(),user_turn_id:gid.into(),task_id:task.clone(),
        execution_scope_id:chat.into(),capabilities:vec![AuthorizedCapability{tool_id:TOOL.into(),effect:ToolEffect::ExecuteCode}],
        expires_at:Some(expires)},&verifier).map_err(|_|"Original task control not admitted")?;
    policy.grant(CapabilityGrant{tool_id:TOOL.into(),effect:ToolEffect::ExecuteCode,scope_id:chat.into(),expires_at:Some(expires)})
        .map_err(|_|"Task control grant unavailable")?;
    log(K::HostRouteSelected,&json!({"phase":"native_confirmation","approved":true,"review_sha256":review_sha,
        "effect_call_id":effect,"native_tool_id":TOOL,"authority_task":task,"effect_fingerprint":fingerprint}).to_string())?;
    let exact_input=call.input.clone();
    let fence=Fence{policy:&policy,v:&verifier,c:chat,t:gid,input:&exact_input,live:&checkpoint};
    let origin=BoundCallOrigin{conversation_id:chat.into(),user_turn_id:gid.into(),task_id:task.clone()};
    let observe=progress.dispatch_observer();
    observe(lu_core::agent_tool_loop::ToolDispatchCheckpoint::Entered);
    let result=tauri::async_runtime::block_on(auth.dispatch_bound_durable(&registry,&state.effects,call,origin,&fence));
    observe(lu_core::agent_tool_loop::ToolDispatchCheckpoint::Returned);
    let _=auth.revoke_task(&task);let _=policy.revoke_scope(chat);
    let Ok(output)=&result else{
        log(K::Rejected,"TASK_TERMINATION_NOT_VERIFIED")?;
        return complete("I could not verify cancellation. Check the background tasks view; cancellation was not retried.".into())
    };
    // Re-read the original durable record, independently of the result codec.
    // Windows cancel_owned has already required an empty Job and reaped child.
    let record=state.background_tasks.get(target).map_err(|_|"Task record unavailable")?.ok_or("Task record unavailable")?;
    let row=state.effects.get(&effect).map_err(|_|"Effect receipt unavailable")?.ok_or("Effect receipt unavailable")?;
    let bytes=row.receipt.as_ref().ok_or("Effect receipt unavailable")?;
    let durable=wire::decode_reply(bytes).map_err(|_|"Invalid task control receipt")?;
    let decoded=catalog.decode_result(&allowed,NAME,&result).map_err(|_|"Invalid task control result")?;
    if record.workspace_id.as_deref()!=Some(chat)||record.state.as_str()!="CANCELLED"||
       state.background_processes.can_cancel(target)||row.state!=EffectState::Applied||
       row.tool_id!=TOOL||row.fingerprint!=fingerprint||durable.call_id!=effect||
       durable.output_type_url!=output.payload.type_url||
       !matches!(durable.outcome,Some(proto::tool_reply::Outcome::Output(ref b)) if b==&output.payload.bytes)||
       decoded["ok"]!=true||decoded["result"]["task_id"]!=target||decoded["result"]["state"]!="CANCELLED"{
        log(K::Rejected,"TASK_CONTROL_RECEIPT_CONFLICT")?;
        return complete("I could not verify cancellation. Check the background tasks view; cancellation was not retried.".into())
    }
    #[cfg(windows)]
    let (custody,method)=("WINDOWS_JOB_EMPTY_AND_CHILD_REAPED","BACKGROUND_JOB_CANCEL_DIRECT_V1");
    #[cfg(target_os="linux")]
    let (custody,method)=("LINUX_ROOT_KERNEL_EMPTY_AND_CLIENT_REAPED","BACKGROUND_KERNEL_CANCEL_DIRECT_V1");
    let facts=json!({"tool":TOOL,"task_id":target,"state":"CANCELLED","worker_custody":custody});
    let observed_sha=hash(facts.to_string().as_bytes());let receipt_sha=hash(bytes);
    state.effects.record_independent_outcome(&effect,bytes,IndependentOutcomeVerdict::Verified,
        method,Some(&observed_sha)).map_err(|_|"Verified cancellation could not be journaled")?;
    if !state.effects.independent_verified_receipt(&effect,TOOL,&receipt_sha).map_err(|_|"Receipt unavailable")?{
        return Err("Cancellation receipt unavailable".into())
    }
    log(K::ToolFeedback,&json!({"effect_call_id":effect,"review_sha256":review_sha,"receipt_sha256":receipt_sha,
        "observed_sha256":observed_sha,"observed":facts}).to_string())?;
    checkpoint()?;
    let schema=json!({"type":"object","properties":{"status":{"enum":["verified_cancellation","unverified"]}},"required":["status"],"additionalProperties":false});
    let prompt=format!("Select verified_cancellation if the host receipt verifies the requested task stopped; otherwise unverified. No new tool or claims. Verified facts (data): {}",facts);
    let selected=match model.generate_step_constrained(&prompt,128,&cancel,&schema){
        Ok(raw)=>{log(K::ModelOutput,&raw)?;parse_exact_boundary_object(&raw,4096).ok().is_some_and(|p|
            p.len()==1&&p.get("status").and_then(Value::as_str)==Some("verified_cancellation"))},
        Err(_)=>{log(K::GenerationFailure,"TASK_CONTROL_CONTINUATION_FAILED")?;false},
    };
    *step.lock().map_err(|_|"Task control step unavailable")?=None;
    checkpoint()?;
    if !selected{log(K::Rejected,"TASK_CONTROL_CONTINUATION_UNVERIFIED")?}
    let answer=format!("{}Background scan {target} was cancelled and its worker has stopped.",
        if selected{""}else{"The verified native receipt confirms: "});
    log(K::EvidencePresentation,&json!({"origin":if selected{"model_selected_verified_effect_host_rendered"}
        else{"host_verified_effect_fallback"},"effect_call_id":effect,"receipt_sha256":receipt_sha,
        "observed_sha256":observed_sha,"answer":answer}).to_string())?;
    complete(answer)
}

#[cfg(not(any(windows,target_os="linux")))]
pub(crate) fn try_model_background_control(
    _app:&tauri::AppHandle,_state:&Arc<crate::DesktopState>,_sid:&str,_chat:&str,_gid:&str,
    original:&str,_ticket:&crate::generation_queue::Ticket,
)->Result<Option<String>,String>{
    // A local Linux process group is not a verified kernel-owned subtree.
    // Do not route a model cancellation through that weaker boundary.
    if background_cancel_target(original).is_some(){
        return Err("Protected model-directed task cancellation is not available on this host. Use the background tasks view.".into())
    }
    Ok(None)
}

#[cfg(test)]
#[test]
fn cp06er2_cancel_intent_is_finite_and_preserves_exact_case(){
    assert_eq!(background_cancel_target("Cancel background task scan-AbC."),Some("scan-AbC"));
    assert_eq!(background_cancel_target("stop background task scan-123"),Some("scan-123"));
    for s in ["cancel background task all","cancel background task scan-","cancel background task scan-a and scan-b",
        "do not cancel background task scan-a","explain how to cancel background task scan-a",
        "cancel background task scan-a/../b","cancel background task scan-a\nstop background task scan-b",
        "cancel background task scan-a; start another","cancel background task SCAN-AbC"]{
        assert!(background_cancel_target(s).is_none(),"{s}");
    }
}

/// Finite one-shot scan intent. Resource selection belongs to the native
/// picker, never to model output or an embedded command.
fn background_start_requested(original:&str)->bool{
    if original.len()>256||!original.is_ascii()||original.chars().any(char::is_control)
        ||lu_core::capability_router::user_declines_tools(original){return false}
    let s=original.trim().trim_end_matches(['.','?','!']).to_ascii_lowercase();
    matches!(s.as_str(),"start a background file scan"|"start a background scan"|
        "start a background file integrity scan"|"scan a file in the background"|
        "queue a background file scan")
}
#[cfg(any(windows,target_os="linux"))]
pub(crate) fn try_model_background_start(
    app:&tauri::AppHandle,state:&Arc<crate::DesktopState>,sid:&str,chat:&str,gid:&str,
    original:&str,ticket:&crate::generation_queue::Ticket,
)->Result<Option<String>,String>{
    use std::{collections::BTreeSet,time::{SystemTime,Duration}};
    use sha2::{Digest,Sha256};
    use serde_json::{json,Value};
    use lu_core::{
        contracts::{AuthorizationError,ProtoPayload,ToolAuthorizer,ToolDescriptor,ToolEffect,ToolInvocation},
        conversation_authority::{AuthorizedCapability,BoundCallOrigin,ConversationAuthority,TaskAuthority},
        journal::{EffectState,IndependentOutcomeVerdict},
        model_json_boundary::parse_exact_boundary_object,
        model_turn_journal::ModelAuditKind as K,
        native_background_control::register_reviewed_background_start,
        native_background_scheduler::ReviewedScan,
        policy::{CapabilityGrant,PolicyEngine},
        registry::NativeToolRegistry,
        supervised_tool_generation::ModelStepGenerator,
        wire::{self,proto},
    };
    if !background_start_requested(original){return Ok(None)}
    let target_id=crate::next_id("scan-");let target=target_id.as_str();
    const NAME:&str="lu_background_start";
    const TOOL:&str="lu.bgstart.run";
    let verifier=crate::turn_authority::PersistedHumanTurn::current_chat(&state.chats,chat,gid,original);
    let cancel=Cancellation::default();
    let watermark=state.chats.p0_auth_requested_watermark(chat)?;
    let checkpoint=||->Result<(),String>{
        ticket.verify_active_foreground(sid,chat,gid)?;
        if cancel.is_cancelled()||ticket.cancelled()||state.chats.p0_auth_requested_watermark(chat)?!=watermark
            ||!verifier.verify_user_turn(chat,gid){return Err("Cancelled original task control".into())}
        if state.supervisor.status().map_err(|_|"Model unavailable")?.session_id.as_deref()!=Some(sid){
            return Err("Model changed during task control".into())
        }
        Ok(())
    };
    checkpoint()?;
    let journal=ModelTurnJournal::open(&state.catalog_path.with_file_name("lu-model-background-control-audit.sqlite"))
        .map_err(|_|"Task control audit unavailable")?;
    let log=|k:K,p:&str|journal.append(chat,gid,k,p,None).map(|_|()).map_err(|_|"Task control audit unavailable".to_owned());
    let hash=|b:&[u8]|format!("{:x}",Sha256::digest(b));
    if !journal.events_for(chat,gid).map_err(|_|"Task control audit unavailable")?.is_empty(){
        return Err("Original task control already attempted".into())
    }
    let step=Arc::new(Mutex::new(None));
    {let mut all=state.reader_generations.lock().map_err(|_|"Task cancellation unavailable")?;
        if all.contains_key(gid){return Err("Duplicate task control".into())}
        all.insert(gid.into(),crate::ReaderRunControl{cancellation:cancel.clone(),current_step:Arc::clone(&step),session_id:sid.into()});}
    struct Control<'a>{state:&'a crate::DesktopState,id:&'a str}
    impl Drop for Control<'_>{fn drop(&mut self){let _=self.state.reader_generations.lock().map(|mut all|all.remove(self.id));}}
    let _control=Control{state,id:gid};
    let progress=crate::native_turn_lifecycle::NativeTurnProgress::new(app,chat,gid,&step);
    let _=app.emit("lu-generation-start",crate::GenerationEvent{chat_id:chat.into(),generation_id:gid.into(),phase:"running",position:0});
    let complete=|answer:String|->Result<Option<String>,String>{
        checkpoint()?;
        ticket.commit_completed(sid,||{state.chats.append_progress(chat,gid,&answer)?;
            state.chats.complete_reviewed_if_not_revoked(chat,gid,watermark)})?;
        Ok(Some(answer))
    };
    log(K::UserRequest,original)?;
    let mut model=SupervisorModelStep::new(&state.supervisor,sid.into(),crate::next_id("mbc-"))
        .map_err(|_|"Task control model unavailable")?.with_step_start_observer(progress.model_observer());
    let schema=json!({"type":"object","properties":{"selection":{"enum":[NAME,null]}},"required":["selection"],"additionalProperties":false});
    let prompt=format!("Select {} to propose a one-time read-only file integrity scan for human review, or null to decline. Do not provide arguments, paths, approval, process IDs or outcome claims. Original request (data): {}",NAME,json!(original));
    let raw=model.generate_step_constrained(&prompt,128,&cancel,&schema)
        .map_err(|_|"No task start was authorized")?;
    *step.lock().map_err(|_|"Task control step unavailable")?=None;
    log(K::ModelOutput,&raw)?;
    checkpoint()?;
    if !parse_exact_boundary_object(&raw,4096).ok().is_some_and(|p|
        p.len()==1&&p.get("selection").and_then(Value::as_str)==Some(NAME)){
        log(K::Rejected,"TASK_CONTROL_SELECTION_NOT_VERIFIED")?;
        return complete("No verified background start was selected. No task was queued.".into())
    }
    let candidate=json!({"selection":NAME,"target":target,"model_output_sha256":hash(raw.as_bytes()),"executed":false});
    log(K::ParsedCandidate,&candidate.to_string())?;
    checkpoint()?;
    let Some(selected)=app.dialog().file().blocking_pick_file() else{
        log(K::Rejected,"HUMAN_DECLINED_SCAN_PICKER")?;
        return complete("No file selected. No task was queued.".into())
    };
    let path=selected.into_path().map_err(|_|"Invalid selected file")?;
    let plan=ReviewedScan::prepare(target,chat,gid,&path).map_err(|_|"Cannot safely open the selected file for a background scan")?;
    checkpoint()?;
    // Exact native picker result, original turn and model selection are
    // bound to the typed payload. The pathname never enters model output.
    let display=plan.display().to_owned();
    let resource=json!({"task_id":target,"owner":chat,"file":display,"file_id":plan.identity(),
        "bytes":plan.bytes(),"worker_sha256":plan.executable_sha256(),
        "original_request_sha256":hash(original.as_bytes()),"model_output_sha256":hash(raw.as_bytes()),
        "kind":"file_scan","max_runtime_seconds":if cfg!(windows){3600}else{90},"repeat":false});
    let binding=hash(resource.to_string().as_bytes());
    let registry=NativeToolRegistry::new();
    let catalog=register_reviewed_background_start(&registry,Arc::clone(&state.background_scheduler),
        plan,&binding)?;
    let allowed=BTreeSet::from([NAME.to_owned()]);
    let effect=crate::next_id("g-");
    let call=catalog.encode_call(&allowed,NAME,&effect,Some(chat),&json!({"job_kind":"file_scan"}).to_string())
        .map_err(|_|"Exact task start input unavailable")?;
    let fingerprint=lu_core::journal::fingerprint(&call);
    let review=json!({"tool":TOOL,"target":target,"owner":chat,"original_request_sha256":hash(original.as_bytes()),
        "candidate_sha256":hash(candidate.to_string().as_bytes()),"input_sha256":hash(&call.input.bytes),
        "input_type_url":call.input.type_url,"effect_fingerprint":fingerprint,"resource":resource,"resource_binding":binding});
    let review_sha=hash(review.to_string().as_bytes());
    log(K::HostRouteSelected,&json!({"phase":"native_preview","review":review,"review_sha256":review_sha,"approved":false}).to_string())?;
    checkpoint()?;
    let guard_notice=if cfg!(windows){"The selected file is held open against writes and deletion until the scan stops."}
        else{"Lu checks the selected file identity and change timestamps before and after scanning; a changed file fails the scan."};
    let details=format!("File: {display}\nSize: {} bytes\nTask: {target}\n\nOne read-only integrity scan. {guard_notice} Lu will wait for available resources.",resource["bytes"]);
    let pages=review_pages(&details);let count=pages.len();
    for (n,page) in pages.into_iter().enumerate(){
        checkpoint()?;
        let ok=app.dialog().message(page).title(format!("Lu — Review background scan ({}/{count})",n+1))
            .buttons(MessageDialogButtons::OkCancelCustom("Continue review".into(),"Cancel".into())).blocking_show();
        checkpoint()?;
        if !ok{log(K::Rejected,"HUMAN_DECLINED_TASK_CONTROL")?;
            return complete("Background scan declined. No task was queued.".into())}
    }
    let ok=app.dialog().message(format!("Queue the exact reviewed read-only scan?\n\nTask: {target}\nLu will start it when resources permit."))
        .title("Lu — Confirm background scan")
        .buttons(MessageDialogButtons::OkCancelCustom("Queue this scan".into(),"Cancel".into())).blocking_show();
    checkpoint()?;
    if !ok{log(K::Rejected,"HUMAN_DECLINED_TASK_CONTROL")?;
        return complete("Background scan declined. No task was queued.".into())}
    if state.background_scheduler.state_for(target,chat).map_err(|_|"Schedule unavailable")?.is_some()
       ||state.background_tasks.get(target).map_err(|_|"Task store unavailable")?.is_some(){
        return Err("Task identity already exists".into())
    }
    let rows=journal.events_for(chat,gid).map_err(|_|"Task control audit unavailable")?;
    let review_row=rows.get(3).filter(|r|r.kind==K::HostRouteSelected).ok_or("Task control review unavailable")?;
    let checked:Value=serde_json::from_str(&review_row.payload).map_err(|_|"Invalid task control review")?;
    if rows.len()!=4||rows[0].kind!=K::UserRequest||rows[0].payload!=original||
       rows[1].kind!=K::ModelOutput||hash(rows[1].payload.as_bytes())!=hash(raw.as_bytes())||
       rows[2].kind!=K::ParsedCandidate||serde_json::from_str::<Value>(&rows[2].payload).ok()!=Some(candidate)||
       checked["approved"]!=false||checked["phase"]!="native_preview"||
       checked["review_sha256"]!=review_sha||hash(checked["review"].to_string().as_bytes())!=review_sha||
       state.effects.get(&effect).map_err(|_|"Effect history unavailable")?.is_some(){
        return Err("Original task control review changed".into())
    }
    struct Fence<'a>{
        policy:&'a PolicyEngine,v:&'a dyn TrustedUserTurnVerifier,c:&'a str,t:&'a str,
        input:&'a ProtoPayload,live:&'a(dyn Fn()->Result<(),String>+Send+Sync),
    }
    impl ToolAuthorizer for Fence<'_>{
        fn authorize(&self,call:&ToolInvocation,d:&ToolDescriptor)->Result<(),AuthorizationError>{
            if (self.live)().is_err()||!self.v.verify_user_turn(self.c,self.t)||
                call.workspace_id.as_deref()!=Some(self.c)||call.tool_id!="lu.bgstart.run"||
                d.id!="lu.bgstart.run"||d.effect!=ToolEffect::ExecuteCode||&call.input!=self.input{
                return Err(AuthorizationError::new("Exact task start not authorized"))
            }
            self.policy.authorize(call,d)
        }
    }
    let auth=ConversationAuthority::new();
    let policy=PolicyEngine::new();
    let task=format!("background-start-{effect}");
    let expires=SystemTime::now().checked_add(Duration::from_secs(120)).ok_or("Cannot bound task control")?;
    auth.open_user_task(TaskAuthority{conversation_id:chat.into(),user_turn_id:gid.into(),task_id:task.clone(),
        execution_scope_id:chat.into(),capabilities:vec![AuthorizedCapability{tool_id:TOOL.into(),effect:ToolEffect::ExecuteCode}],
        expires_at:Some(expires)},&verifier).map_err(|_|"Original task control not admitted")?;
    policy.grant(CapabilityGrant{tool_id:TOOL.into(),effect:ToolEffect::ExecuteCode,scope_id:chat.into(),expires_at:Some(expires)})
        .map_err(|_|"Task control grant unavailable")?;
    log(K::HostRouteSelected,&json!({"phase":"native_confirmation","approved":true,"review_sha256":review_sha,
        "effect_call_id":effect,"native_tool_id":TOOL,"authority_task":task,"effect_fingerprint":fingerprint}).to_string())?;
    let exact_input=call.input.clone();
    let fence=Fence{policy:&policy,v:&verifier,c:chat,t:gid,input:&exact_input,live:&checkpoint};
    let origin=BoundCallOrigin{conversation_id:chat.into(),user_turn_id:gid.into(),task_id:task.clone()};
    let observe=progress.dispatch_observer();
    observe(lu_core::agent_tool_loop::ToolDispatchCheckpoint::Entered);
    let result=tauri::async_runtime::block_on(auth.dispatch_bound_durable(&registry,&state.effects,call,origin,&fence));
    observe(lu_core::agent_tool_loop::ToolDispatchCheckpoint::Returned);
    let _=auth.revoke_task(&task);let _=policy.revoke_scope(chat);
    let Ok(output)=&result else{
        log(K::Rejected,"TASK_QUEUE_NOT_VERIFIED")?;
        return complete("I could not verify queue admission. Check the background tasks view; the request was not retried.".into())
    };
    // Re-read the original scoped durable scheduler row. Admission does
    // not mean a process started or that an integrity result exists.
    let queued=state.background_scheduler.state_for(target,chat).map_err(|_|"Schedule unavailable")?;
    let record=state.background_tasks.get(target).map_err(|_|"Task store unavailable")?;
    let row=state.effects.get(&effect).map_err(|_|"Effect receipt unavailable")?.ok_or("Effect receipt unavailable")?;
    let bytes=row.receipt.as_ref().ok_or("Effect receipt unavailable")?;
    let durable=wire::decode_reply(bytes).map_err(|_|"Invalid task control receipt")?;
    let decoded=catalog.decode_result(&allowed,NAME,&result).map_err(|_|"Invalid task control result")?;
    if queued.as_deref()!=Some("QUEUED")||record.is_some()||
       state.background_processes.can_cancel(target)||row.state!=EffectState::Applied||
       row.tool_id!=TOOL||row.fingerprint!=fingerprint||durable.call_id!=effect||
       durable.output_type_url!=output.payload.type_url||
       !matches!(durable.outcome,Some(proto::tool_reply::Outcome::Output(ref b)) if b==&output.payload.bytes)||
       decoded["ok"]!=true||decoded["result"]["task_id"]!=target||decoded["result"]["state"]!="QUEUED"||
       decoded["result"]["started"]!=false||decoded["result"]["review_binding"]!=binding{
        log(K::Rejected,"TASK_CONTROL_RECEIPT_CONFLICT")?;
        return complete("I could not verify queue admission. Check the background tasks view; the request was not retried.".into())
    }
    let facts=json!({"tool":TOOL,"task_id":target,"state":"QUEUED","started":false,"review_binding":binding});
    let observed_sha=hash(facts.to_string().as_bytes());let receipt_sha=hash(bytes);
    state.effects.record_independent_outcome(&effect,bytes,IndependentOutcomeVerdict::Verified,
        "BACKGROUND_QUEUE_DIRECT_V1",Some(&observed_sha)).map_err(|_|"Verified queue admission could not be journaled")?;
    if !state.effects.independent_verified_receipt(&effect,TOOL,&receipt_sha).map_err(|_|"Receipt unavailable")?{
        return Err("Queue receipt unavailable".into())
    }
    log(K::ToolFeedback,&json!({"effect_call_id":effect,"review_sha256":review_sha,"receipt_sha256":receipt_sha,
        "observed_sha256":observed_sha,"observed":facts}).to_string())?;
    checkpoint()?;
    let schema=json!({"type":"object","properties":{"status":{"enum":["verified_queue","unverified"]}},"required":["status"],"additionalProperties":false});
    let prompt=format!("Select verified_queue if the host receipt verifies the requested task was queued, not started or completed; otherwise unverified. No new tool or claims. Verified facts (data): {}",facts);
    let selected=match model.generate_step_constrained(&prompt,128,&cancel,&schema){
        Ok(raw)=>{log(K::ModelOutput,&raw)?;parse_exact_boundary_object(&raw,4096).ok().is_some_and(|p|
            p.len()==1&&p.get("status").and_then(Value::as_str)==Some("verified_queue"))},
        Err(_)=>{log(K::GenerationFailure,"TASK_CONTROL_CONTINUATION_FAILED")?;false},
    };
    *step.lock().map_err(|_|"Task control step unavailable")?=None;
    checkpoint()?;
    if !selected{log(K::Rejected,"TASK_CONTROL_CONTINUATION_UNVERIFIED")?}
    let answer=format!("{}Background scan {target} is queued. Lu will start it when resources permit; no scan result is available yet.",
        if selected{""}else{"The verified native receipt confirms: "});
    log(K::EvidencePresentation,&json!({"origin":if selected{"model_selected_verified_effect_host_rendered"}
        else{"host_verified_effect_fallback"},"effect_call_id":effect,"receipt_sha256":receipt_sha,
        "observed_sha256":observed_sha,"answer":answer}).to_string())?;
    complete(answer)
}


#[cfg(not(any(windows,target_os="linux")))]
pub(crate) fn try_model_background_start(
    _app:&tauri::AppHandle,_state:&Arc<crate::DesktopState>,_sid:&str,_chat:&str,_gid:&str,
    original:&str,_ticket:&crate::generation_queue::Ticket,
)->Result<Option<String>,String>{
    if background_start_requested(original){
        return Err("Protected model-directed background scan initiation is not available on this host.".into())
    }
    Ok(None)
}
#[cfg(test)]
#[test]
fn cp06er2_start_intent_is_one_shot_and_never_accepts_paths(){
    assert!(background_start_requested("Start a background file scan."));
    assert!(background_start_requested("Scan a file in the background"));
    for s in ["start a background scan /tmp/a","start a background scan and delete it",
        "do not start a background scan","start a background scan every hour",
        "start a background scan with no tools","start a background scan\ncancel task scan-a",
        "run a background shell","start a background file scan C:\\private.txt"]{
        assert!(!background_start_requested(s),"{s}");
    }
}
