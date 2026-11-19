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
    let review=if proposal.action()==Action::Archive{
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
    if let Some(manifest)=review.manifest(){state.wave2_recovery.record_archive_intent(chat,&effect,review.display(),review.target(),manifest,"Model-selected, separately approved archive extraction")?;}else{state.wave2_recovery.record_intent(chat,&effect,review.action().tool(),review.display(),review.target(),review.digest(),if review.action().is_revision(){"Model-selected, separately approved non-destructive Office revision"}else{"Model-selected, separately approved document creation"})?;}
    checkpoint()?;let observe=progress.dispatch_observer();observe(lu_core::agent_tool_loop::ToolDispatchCheckpoint::Entered);
    let result=tauri::async_runtime::block_on(review.execute(true,&effect,&verifier,&state.effects,&journal,&checkpoint));observe(lu_core::agent_tool_loop::ToolDispatchCheckpoint::Returned);drop(stop);drop(run);
    let outcome=result?.ok_or("No confirmed publication receipt")?;let answer=outcome.present(&mut model,&cancellation,&journal,&state.effects,&checkpoint);*step.lock().map_err(|_|"Publication step unavailable")?=None;complete(answer?)
}
#[cfg(test)]
mod tests{use super::*;#[test]fn cp06dr2_review_paginates_all_text(){let text="漢字🦊\n".repeat(500);let pages=review_pages(&text);assert!(pages.len()>1);assert_eq!(pages.concat(),text);assert!(pages.iter().all(|p|p.chars().count()<=660&&p.lines().count()<=11));}}
