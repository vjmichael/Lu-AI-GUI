//! PLAN-07B: real local GGUF-only generation from audited PLAN07A source
//! snapshot; prompt, original response and identity are immutable ANSF data.
//! No model tool route, no task authorizer, no acceptance or Git mutation.
use super::*;
use lu_core::{project_task_update_model as suggestions,
    project_task_updates as updates,project_evidence_graph as graph,
    project_status_classifier as status,project_task_synthesis as synthesis};
use rusqlite::{Connection,params,OptionalExtension,TransactionBehavior};

fn task_window_only(caller:&tauri::WebviewWindow)->Result<(),String>{
    if caller.label()!="task-list"{
        return Err("PLAN07B_REQUIRES_INDEPENDENT_OPERATOR_TASK_WINDOW".into())
    }
    Ok(())
}
fn init(conn:&Connection)->Result<(),String>{
    conn.execute_batch("CREATE TABLE IF NOT EXISTS ansf_project_task_update_model_suggestions(
        suggestion_sha256 TEXT PRIMARY KEY,
        preview_sha256 TEXT NOT NULL,
        register_id TEXT NOT NULL,
        expected_register_sha256 TEXT NOT NULL,
        graph_sha256 TEXT NOT NULL,
        status_sha256 TEXT NOT NULL,
        model_ref TEXT NOT NULL,
        session_id TEXT NOT NULL,
        generation_id TEXT NOT NULL,
        original_prompt_sha256 TEXT NOT NULL,
        original_reply_sha256 TEXT NOT NULL,
        original_prompt_text BLOB NOT NULL,
        original_reply_text BLOB NOT NULL,
        suggestion_json BLOB NOT NULL,
        no_completion_authority INTEGER NOT NULL CHECK(no_completion_authority=1),
        no_execution_authority INTEGER NOT NULL CHECK(no_execution_authority=1)
    );").map_err(|_|"PLAN07B_ANSF_SCHEMA_FAILED".to_string())
}
fn equal_native(a:&(graph::EvidenceGraph,status::StatusReport),
    b:&(graph::EvidenceGraph,status::StatusReport))->Result<bool,String>{
    Ok(synthesis::same_observed_sources(&a.0,&b.0)?&&
        a.1.complete==b.1.complete&&
        serde_json::to_value(&a.1.findings).map_err(|_|"PLAN07B_OLD_STATUS_ENCODE")?==
        serde_json::to_value(&b.1.findings).map_err(|_|"PLAN07B_NEW_STATUS_ENCODE")?)
}
fn persist(profile:&Path,proof:&suggestions::ModelSuggestion,
    exact_prompt:&str,raw:&str)->Result<(),String>{
    let (preview,register,_,g,s)=project_updates_desktop::reload_audited(
        profile,&proof.preview_sha256)?;
    if proof.expected_register_sha256!=register.latest_sha256{
        return Err("PLAN07B_REGISTER_CHANGED_BEFORE_PERSIST".into())
    }
    if exact_prompt!=suggestions::prompt(&preview,&g,&s)?{
        return Err("PLAN07B_PROMPT_MISMATCH_BEFORE_PERSIST".into())
    }
    suggestions::audit(proof,&preview,&g,&s,raw)?;
    let bytes=serde_json::to_vec(proof).map_err(|_|"PLAN07B_RECEIPT_ENCODE")?;
    if bytes.len()>80*1024||raw.len()>suggestions::MAX_REPLY||exact_prompt.len()>22000{
        return Err("PLAN07B_IMMUTABLE_RECEIPT_BUDGET".into())
    }
    let mut conn=project_register_desktop::library(profile)?;
    init(&conn)?;
    let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_|"PLAN07B_SQLITE_IMMEDIATE_BEGIN")?;
    // Recheck latest register identity INSIDE the write transaction. A
    // concurrent correction cannot be bypassed by a stale model run.
    let latest:Option<String>=tx.query_row(
        "SELECT latest_sha256 FROM ansf_project_task_register_revisions
         WHERE register_id=?1 ORDER BY register_revision DESC LIMIT 1",
        [&proof.register_id],|row|row.get(0)
    ).optional().map_err(|_|"PLAN07B_SQLITE_LATEST_LOOKUP")?;
    if latest.as_deref()!=Some(proof.expected_register_sha256.as_str()){
        return Err("PLAN07B_REGISTER_EDITED_DURING_INFERENCE".into())
    }
    tx.execute("INSERT OR IGNORE INTO ansf_project_task_update_model_suggestions(
        suggestion_sha256,preview_sha256,register_id,expected_register_sha256,
        graph_sha256,status_sha256,model_ref,session_id,generation_id,
        original_prompt_sha256,original_reply_sha256,original_prompt_text,
        original_reply_text,suggestion_json,no_completion_authority,no_execution_authority)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,1,1)",
        params![
            proof.digest_sha256,proof.preview_sha256,proof.register_id,
            proof.expected_register_sha256,proof.graph_sha256,proof.status_sha256,
            proof.original_model_ref,proof.original_session_id,proof.original_generation_id,
            proof.original_prompt_sha256,proof.original_reply_sha256,
            exact_prompt.as_bytes(),raw.as_bytes(),bytes
        ]
    ).map_err(|_|"PLAN07B_IMMUTABLE_INSERT_FAILED")?;
    let (saved_prompt,saved_reply,saved): (Vec<u8>,Vec<u8>,Vec<u8>)=tx.query_row(
        "SELECT original_prompt_text,original_reply_text,suggestion_json
         FROM ansf_project_task_update_model_suggestions
         WHERE suggestion_sha256=?1",
        [&proof.digest_sha256],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))
    ).map_err(|_|"PLAN07B_IMMUTABLE_READBACK_MISSING")?;
    if saved_prompt!=exact_prompt.as_bytes()||saved_reply!=raw.as_bytes()||
        saved!=bytes{
        return Err("PLAN07B_ORIGINAL_PROMPT_REPLY_COLLISION".into())
    }
    tx.commit().map_err(|_|"PLAN07B_RECEIPT_TRANSACTION_FAILED")?;
    Ok(())
}
fn read_latest(profile:&Path,preview_id:&str)
 ->Result<Option<suggestions::ModelSuggestion>,String>{
    let (preview,register,_,g,s)=project_updates_desktop::reload_audited(
        profile,preview_id)?;
    let conn=project_register_desktop::library(profile)?;
    init(&conn)?;
    let row:Option<(Vec<u8>,Vec<u8>,Vec<u8>)>=conn.query_row(
        "SELECT original_prompt_text,original_reply_text,suggestion_json
         FROM ansf_project_task_update_model_suggestions WHERE preview_sha256=?1
         AND expected_register_sha256=?2
         ORDER BY rowid DESC LIMIT 1",
        params![preview_id,register.latest_sha256],
        |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))
    ).optional().map_err(|_|"PLAN07B_LATEST_SUGGESTION_QUERY")?;
    let Some((old_prompt,reply,bytes))=row else{return Ok(None)};
    if old_prompt.len()>22000||reply.len()>suggestions::MAX_REPLY||
        bytes.len()>80*1024{
        return Err("PLAN07B_OVERSIZED_SAVED_MODEL_RECEIPT".into())
    }
    let p=String::from_utf8(old_prompt).map_err(|_|"PLAN07B_PROMPT_NOT_UTF8")?;
    let raw=String::from_utf8(reply).map_err(|_|"PLAN07B_REPLY_NOT_UTF8")?;
    let proof:suggestions::ModelSuggestion=serde_json::from_slice(&bytes)
        .map_err(|_|"PLAN07B_SAVED_SUGGESTION_NOT_JSON")?;
    if p!=suggestions::prompt(&preview,&g,&s)?||
        proof.preview_sha256!=preview_id||
        proof.expected_register_sha256!=register.latest_sha256{
        return Err("PLAN07B_SAVED_SUGGESTION_STALE_SOURCE".into())
    }
    suggestions::audit(&proof,&preview,&g,&s,&raw)?;
    Ok(Some(proof))
}
#[tauri::command]
pub(crate) async fn lu_project_update_model_latest(
    caller:tauri::WebviewWindow,state:State<'_,Arc<DesktopState>>,
    preview_sha256:String
)->Result<Option<suggestions::ModelSuggestion>,String>{
    task_window_only(&caller)?;
    let profile=project_register_desktop::profile(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||
        read_latest(&profile,&preview_sha256))
        .await.map_err(|_|"PLAN07B_REOPEN_RECEIPT_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_update_model_suggest(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,preview_sha256:String
)->Result<Option<suggestions::ModelSuggestion>,String>{
    task_window_only(&caller)?;
    if !graph::valid_digest(&preview_sha256){
        return Err("PLAN07B_INVALID_OPERATOR_PREVIEW_ID".into())
    }
    let owner=Arc::clone(state.inner());
    let ready=owner.supervisor.status()
        .map_err(|e|format!("PLAN07B_REAL_GGUF_UNAVAILABLE: {e:?}"))?;
    if ready.phase!=lu_core::inference_host::EnginePhase::Ready||
       ready.session_id.is_none()||ready.model_ref.is_none(){
        return Err("PLAN07B_LOCAL_GGUF_NOT_READY".into())
    }
    let profile=project_register_desktop::profile(&owner)?;
    let check_preview=preview_sha256.clone();
    let original_profile=profile.clone();
    let (native_preview,_,_,native_graph,native_status)=
        tauri::async_runtime::spawn_blocking(move||
            project_updates_desktop::reload_audited(&original_profile,&check_preview)
        ).await.map_err(|_|"PLAN07B_ORIGINAL_PREVIEW_INTERRUPTED")??;
    let planned_prompt=suggestions::prompt(
        &native_preview,&native_graph,&native_status)?;
    let prompt_hash=format!("{:x}",Sha256::digest(planned_prompt.as_bytes()));
    if !project_register_desktop::confirm(&app,
        "Lu — Local GGUF evidence suggestions",
        format!("Ask the currently loaded LOCAL GGUF to propose HUMAN evidence \
        review steps for this original project?\n\nNative source HEAD: {}\n\
        Original review SHA: {}\nPrompt SHA: {}\nCandidates: {}\n\n\
        Model text is UNTRUSTED; it cannot certify completion, authorize execution, \
        edit task histories or create issues. Choose the SAME original project \
        next, and Lu will recheck the actual source before and after inference.",
        native_preview.current_head.as_deref().unwrap_or("not verified"),
        native_preview.digest_sha256,prompt_hash,native_preview.candidates.len())
    ).await?{return Ok(None)}
    let picker=app.clone();
    let chosen=tauri::async_runtime::spawn_blocking(move||
        picker.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"PLAN07B_NATIVE_FOLDER_PICKER_INTERRUPTED")?;
    let Some(chosen)=chosen else{return Ok(None)};
    let root=chosen.into_path().map_err(|_|"PLAN07B_NATIVE_PROJECT_PATH_INVALID")?;
    let sid=ready.session_id.ok_or("PLAN07B_GGUF_SESSION_MISSING")?;
    let model=ready.model_ref.ok_or("PLAN07B_GGUF_MODEL_MISSING")?;
    tauri::async_runtime::spawn_blocking(move||{
        // One complete native source read before generation. No renderer
        // supplied paths, web, historical chat or mutable tool privileges.
        let original=project_status_desktop::classify_selected(&root)?;
        if !equal_native(&(native_graph.clone(),native_status.clone()),&original)?{
            return Err("PLAN07B_SOURCE_CHANGED_BEFORE_MODEL".into())
        }
        let (preview,register,_,g,s)=project_updates_desktop::reload_audited(
            &profile,&preview_sha256)?;
        if preview.expected_register_sha256!=register.latest_sha256||
            !equal_native(&(g.clone(),s.clone()),&original)?{
            return Err("PLAN07B_STALE_PREVIEW_BEFORE_GGUF".into())
        }
        let prompt=suggestions::prompt(&preview,&g,&s)?;
        let before=owner.supervisor.status()
            .map_err(|_|"PLAN07B_GGUF_STATUS_UNAVAILABLE")?;
        if before.phase!=lu_core::inference_host::EnginePhase::Ready||
           before.session_id.as_deref()!=Some(sid.as_str())||
           before.model_ref.as_deref()!=Some(model.as_str()){
            return Err("PLAN07B_LOCAL_MODEL_CHANGED".into())
        }
        let generation=next_id("g-");let request=next_id("req-");
        let (ticket,_)=owner.generation_queue.enqueue(
            &sid,"lu-plan07b-host-suggestions",&generation)?;
        ticket.wait_for_slot()?;
        if ticket.cancelled(){return Err("PLAN07B_OPERATOR_GENERATION_CANCELLED".into())}
        let history=vec![];
        owner.supervisor.reserve_generation_with_history(
            &sid,&generation,&request,&prompt,2560,&history)
            .map_err(|e|format!("PLAN07B_REAL_GGUF_RESERVATION_FAILED: {e:?}"))?;
        if ticket.cancelled(){
            let _=owner.supervisor.cancel(&sid,&generation,&next_id("req-"));
            return Err("PLAN07B_OPERATOR_GENERATION_CANCELLED".into())
        }
        let mut raw=String::new();
        let emit=|wire:Vec<u8>|->Result<(),BridgeError>{
            let frame=decode_frame(&wire).map_err(BridgeError::Wire)?;
            if let Some(proto::inference_frame::Body::Event(event))=frame.body{
                if let Some(proto::inference_event::Kind::TextChunk(chunk))=event.kind{
                    if raw.len().saturating_add(chunk.text.len())>suggestions::MAX_REPLY{
                        return Err(BridgeError::WrongOperation)
                    }
                    raw.push_str(&chunk.text);
                }
            }
            Ok(())
        };
        owner.supervisor.run_reserved_with_history(
            &sid,&generation,&request,prompt.clone(),2560,history,emit)
            .map_err(|e|format!("PLAN07B_ACTUAL_GGUF_INFERENCE_FAILED: {e:?}"))?;
        if ticket.cancelled(){return Err("PLAN07B_GENERATION_CANCELLED".into())}
        let validated=suggestions::validate(&preview,&g,&s,&raw,&prompt,
            &model,&sid,&generation)
            .map_err(|e|format!("PLAN07B_GGUF_OUTPUT_REJECTED: {e}"))?;
        let after=project_status_desktop::classify_selected(&root)?;
        if !equal_native(&(g,s),&after)?{
            return Err("PLAN07B_SOURCE_CHANGED_DURING_GGUF".into())
        }
        let current=owner.supervisor.status()
            .map_err(|_|"PLAN07B_GGUF_STATUS_LOST")?;
        if current.phase!=lu_core::inference_host::EnginePhase::Ready||
           current.session_id.as_deref()!=Some(sid.as_str())||
           current.model_ref.as_deref()!=Some(model.as_str()){
            return Err("PLAN07B_GGUF_SESSION_CHANGED_AFTER_GENERATION".into())
        }
        persist(&profile,&validated,&prompt,&raw)?;
        Ok(Some(validated))
    }).await.map_err(|_|"PLAN07B_GGUF_WORKER_THREAD_INTERRUPTED")?
}
