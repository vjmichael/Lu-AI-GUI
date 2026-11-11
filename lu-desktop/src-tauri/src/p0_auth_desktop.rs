//! P0-AUTH: one operator-facing, exact-chat native status and stop sweep.
//! Does NOT replace domain-specific execution. Each original OS supervisor
//! owns its child; state/receipt checks never invent old grants or retry work.
use super::*;
use crate::p0_auth_custody::{
    HostCancelEvent,HostCancelHistory,DomainResult,SCHEMA
};
use lu_core::tasks::TaskState;
use serde::Serialize;
use std::sync::atomic::Ordering;

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct AuthorityPosture{
    pub chat_id:String,
    pub model_generation:String,
    pub browser_host_grant:bool,
    pub observe_mode:String,
    pub offline_ocr_cancellable:bool,
    pub scheduled_cancellable:usize,
    pub scheduled_window_truncated:bool,
    pub native_workers_cancellable:usize,
    pub interrupted_or_uncertain_native_tasks:usize,
    pub original_reviewed_recovery_status:String,
    pub original_host_cancel_events:Vec<HostCancelHistory>,
    pub execution_authorized:bool,
    pub previous_grant_reusable:bool,
    pub previous_os_custody_independently_proven:bool,
    pub automatic_retry_allowed:bool,
}
fn posture(owner:&DesktopState,chat:&str)->Result<AuthorityPosture,String>{
    owner.chats.require(chat)?;
    let model=match owner.generation_queue.scoped_job_active(chat)?{
        Some(true)=>"ACTIVE_UNPROVEN_OS_STOP",
        Some(false)=>"QUEUED_NOT_RUNNING",
        None=>"NOT_PRESENT",
    };
    let browser=owner.browser_grant.lock()
        .map_err(|_|"Browser grant ownership unavailable")?
        .as_ref().is_some_and(|g|g.chat_id==chat);
    let observe=owner.observe.owned_status(chat)?
        .map(|v|v.mode.to_string()).unwrap_or_else(||"off".into());
    let ocr=owner.ocr_cancellations.lock()
        .map_err(|_|"OCR cancellation registry unavailable")?
        .contains_key(chat);
    let scheduled=owner.background_scheduler.list_owned(chat,100)
        .map_err(|_|"Background schedule original records unavailable")?;
    let dashboard=owner.background_tasks.dashboard_for_workspace(chat,100)
        .map_err(|_|"Original background tasks unavailable")?;
    let interrupted=dashboard.iter()
        .filter(|r|r.state==TaskState::Interrupted).count();
    let native=dashboard.iter()
        .filter(|r|owner.background_processes.can_cancel(&r.task_id)).count();
    let host_recovery=owner.catalog_path.parent().and_then(|profile|{
        agent_recovery_provenance::read_original_recovery(
            profile,chat,&owner.chats
        ).ok().map(|r|r.status.to_owned())
    }).unwrap_or_else(||
        "ORIGINAL_REVIEWED_RECOVERY_UNAVAILABLE_RECONCILE".into());
    Ok(AuthorityPosture{
        chat_id:chat.into(),model_generation:model.into(),
        browser_host_grant:browser,observe_mode:observe,
        offline_ocr_cancellable:ocr,
        scheduled_cancellable:scheduled.iter().filter(|v|v.can_cancel).count(),
        scheduled_window_truncated:scheduled.len()==100,
        native_workers_cancellable:native,
        interrupted_or_uncertain_native_tasks:interrupted,
        original_reviewed_recovery_status:host_recovery,
        original_host_cancel_events:owner.chats.p0_auth_cancel_history(chat)?,
        execution_authorized:false,previous_grant_reusable:false,
        previous_os_custody_independently_proven:false,
        automatic_retry_allowed:false,
    })
}
#[tauri::command]
pub(crate) async fn lu_p0_authority_posture(
    state:State<'_,Arc<DesktopState>>,chat_id:String
)->Result<AuthorityPosture,String>{
    let owner=Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move||
        posture(&owner,&chat_id)
    ).await.map_err(|_|"P0-AUTH native posture observer unavailable")?
}

fn result(name:&str,state:&str)->DomainResult{
    DomainResult{name:name.into(),state:state.into()}
}
/// Only the original owner of a chat may stop its resource family. A
/// previously saved journal, browser grant ID, model tool, screen pixel or
/// inherited task plan cannot request this sweep.
fn stop_scoped(owner:&DesktopState,chat:&str)->Vec<DomainResult>{
    let mut records=Vec::with_capacity(7);
    // Revoke browser authority BEFORE attempting the OS stop. Only touch
    // the browser if the HOST grant is owned by this exact conversation.
    let browser=owner.browser_grant.lock()
        .ok().and_then(|mut slot|{
            if slot.as_ref().is_some_and(|g|g.chat_id==chat){
                slot.take()
            }else{None}
        });
    if let Some(grant)=browser{
        let revoke=owner.browser_authority.revoke_task(&grant.task_id);
        if let Some((session,generation))=grant.active_read{
            let _=owner.generation_queue.cancel(&session,&generation);
            let _=owner.supervisor.cancel(
                &session,&generation,&next_id("req-")
            );
        }
        let stopped=owner.browser_runtime.stop();
        records.push(result("browser",if revoke.is_ok()&&stopped.is_ok(){
            "NATIVE_STOP_RETURNED_CUSTODY_RECHECK_REQUIRED"
        }else{"STOP_UNCERTAIN_RECONCILE"}));
    }else{
        // A poisoned browser lock is uncertainty, not evidence of absence.
        records.push(result("browser",if owner.browser_grant.is_poisoned(){
            "STOP_UNCERTAIN_RECONCILE"
        }else{"NOT_PRESENT"}));
    }
    // Observe is a separate double-consent lane. Do not even call its
    // status/stop for a different conversation.
    match owner.observe.owned_status(chat){
        Ok(Some(view)) if view.mode!="off"=>{
            let stopped=owner.observe.stop(chat);
            records.push(result("observe",if stopped.is_ok(){
                "OBSERVE_PERMISSION_REVOKED"
            }else{"STOP_UNCERTAIN_RECONCILE"}))
        },
        Ok(_)=>records.push(result("observe","NOT_PRESENT")),
        Err(_)=>records.push(result("observe","STOP_UNCERTAIN_RECONCILE")),
    }
    // The offline OCR worker checks its own atomic cancellation. Setting
    // it is NOT a child process reap receipt or a verified unmodified file.
    match owner.ocr_cancellations.lock(){
        Ok(registry)=>{
            if let Some(flag)=registry.get(chat){
                flag.store(true,Ordering::SeqCst);
                records.push(result("ocr","CANCEL_REQUESTED_NOT_REAPED"));
            }else{records.push(result("ocr","NOT_PRESENT"))}
        },
        Err(_)=>records.push(result("ocr","STOP_UNCERTAIN_RECONCILE"))
    }
    // The unique model generation is resolved through the original
    // chat-bound queue, never from the UI's generation id. Reader uses the
    // same original generation but owns its nested tool step separately.
    match owner.generation_queue.cancel_for_chat(chat){
        Ok(Some((session,generation,phase)))=>{
            let reader=owner.reader_generations.lock().ok()
                .and_then(|r|r.get(&generation).cloned());
            if let Some(ref controller)=reader{
                controller.cancellation.cancel();
            }
            let step=reader.as_ref().and_then(|r|
                r.current_step.lock().ok().and_then(|x|x.clone())
            ).unwrap_or_else(||generation.clone());
            let cancel=if phase==CancelState::Active{
                owner.supervisor.cancel(
                    &session,&step,&next_id("req-")
                ).is_ok()
            }else{true};
            let state=if !cancel{"STOP_UNCERTAIN_RECONCILE"}
                else if phase==CancelState::Queued{"MODEL_CANCELLED_QUEUE_ONLY"}
                else{"CANCEL_REQUESTED_NOT_REAPED"};
            records.push(result("model",state));
            records.push(result("reader",if reader.is_some(){state}else{"NOT_PRESENT"}));
        },
        Ok(None)=>{
            records.push(result("model","NOT_PRESENT"));
            records.push(result("reader","NOT_PRESENT"));
        },
        Err(_)=>{
            records.push(result("model","STOP_UNCERTAIN_RECONCILE"));
            records.push(result("reader","STOP_UNCERTAIN_RECONCILE"));
        }
    }
    // A scheduled job is cancelled only using the existing host-owned
    // scheduler exact chat+task row. A missing worker receipt cannot
    // authorize a second execution or a fabricated cancellation success.
    match owner.background_scheduler.list_owned(chat,100){
        Ok(rows)=>{
            let capped=rows.len()==100;
            let current=rows.into_iter().filter(|row|row.can_cancel)
                .collect::<Vec<_>>();
            // Attempt EVERY owned task even if an earlier cancel fails.
            // Otherwise the first failure would silently skip later workers.
            let mut uncertain=false;
            for row in &current{
                if owner.background_scheduler.cancel_owned(
                    &row.task_id,chat
                ).is_err(){uncertain=true}
            }
            records.push(result("scheduled",if current.is_empty()&&!capped{
                "NOT_PRESENT"
            }else if uncertain||capped{
                "SCHEDULE_CANCEL_UNCERTAIN"
            }else{"SCHEDULE_CANCEL_RECORDED"}))
        },
        Err(_)=>records.push(result("scheduled","SCHEDULE_CANCEL_UNCERTAIN"))
    }
    // Catch actual OS-owned workers that are not scheduled. Dashboard is
    // filtered by exact authenticated chat BEFORE applying its bound.
    match owner.background_tasks.dashboard_for_workspace(chat,100){
        Ok(rows)=>{
            let capped=rows.len()==100;
            let owned=rows.into_iter().filter(|row|
                owner.background_processes.can_cancel(&row.task_id)
            ).collect::<Vec<_>>();
            let mut failed=false;
            for row in &owned{
                if owner.background_processes.cancel_owned(
                    &row.task_id,chat
                ).is_err(){failed=true}
            }
            records.push(result("background",if owned.is_empty()&&!capped{
                "NOT_PRESENT"
            }else if failed||capped{"STOP_UNCERTAIN_RECONCILE"}
            else{"NATIVE_TERMINATION_REAP_RETURNED"}))
        },
        Err(_)=>records.push(result("background","STOP_UNCERTAIN_RECONCILE"))
    }
    records
}
#[tauri::command]
pub(crate) async fn lu_p0_cancel_chat_operations(
    app:AppHandle,state:State<'_,Arc<DesktopState>>,chat_id:String
)->Result<Option<AuthorityPosture>,String>{
    state.chats.require(&chat_id)?;
    // Requiring a fresh native confirmation is independent of any
    // previously remembered capability, model plan or webview click.
    let dialog=app.clone();
    let approved=tauri::async_runtime::spawn_blocking(move||
        dialog.dialog().message(
            "Revoke the selected conversation's CURRENT Browser, Observe, \
             model, Reader and OCR operations, and stop its native scheduled \
             and unscheduled background workers?\n\n\
             Other conversations are not authorized targets. A stop request \
             does NOT prove that every OS descendant was reaped, that \
             interrupted file effects are safe to retry, or that old \
             permission can be reused.\n\n\
             Lu first writes a durable cancellation intent to the original \
             ANSF audit. Any incomplete/uncertain stop will require review."
        ).title("Lu — Stop only this conversation's native work")
        .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
            "Revoke and stop work".into(),"Keep running".into()
        )).blocking_show()
    ).await.map_err(|_|"Native conversation stop approval unavailable")?;
    if !approved{return Ok(None)}
    let owner=Arc::clone(state.inner());
    tauri::async_runtime::spawn_blocking(move||{
        owner.chats.require(&chat_id)?;
        let attempt=next_id("au-");
        owner.chats.append_p0_auth_cancel(&HostCancelEvent{
            schema:SCHEMA.into(),attempt_id:attempt.clone(),
            chat_id:chat_id.clone(),phase:"REQUESTED".into(),
            domains:Vec::new(),authority_restored:false,
            automatic_retry_allowed:false,
            os_custody_independently_verified:false
        })?;
        // No replay on interruption. If the process crashes here or a
        // subsystem panics, absence of RESULT is a durable UNKNOWN.
        let outcomes=stop_scoped(&owner,&chat_id);
        owner.chats.append_p0_auth_cancel(&HostCancelEvent{
            schema:SCHEMA.into(),attempt_id:attempt,chat_id:chat_id.clone(),
            phase:"RESULT".into(),domains:outcomes,
            authority_restored:false,automatic_retry_allowed:false,
            os_custody_independently_verified:false
        })?;
        posture(&owner,&chat_id).map(Some)
    }).await.map_err(|_|"Native conversation stop interrupted; reconcile original intent")?
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn stop_return_status_alone_is_never_an_independent_os_reap_proof(){
        let result=result("browser","NATIVE_STOP_RETURNED_CUSTODY_RECHECK_REQUIRED");
        assert_ne!(result.state,"OS_CUSTODY_INDEPENDENTLY_VERIFIED");
        let uncertain=result("ocr","CANCEL_REQUESTED_NOT_REAPED");
        assert_ne!(uncertain.state,"NATIVE_TERMINATION_REAP_RETURNED");
    }
}
