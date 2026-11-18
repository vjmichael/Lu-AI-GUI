//! PLAN-04: native Settings -> original bounded source -> real loaded GGUF
//! -> independently checked task proposal. Never create a worker grant.
use super::*;
use lu_core::{
    project_evidence_graph as graph,
    project_status_classifier as classifier,
    project_task_synthesis as synthesis,
};
use rusqlite::{Connection,OpenFlags,params};

fn scan_and_classify(root:&Path)
 ->Result<(graph::EvidenceGraph,classifier::StatusReport),String>{
    let g=project_evidence_desktop::assemble_from_selected(root)?;
    let facts=classifier::observe_project_paths(root,&g)?;
    let r=classifier::classify(&g,&facts)?;
    classifier::audit(&r,&g)?;
    Ok((g,r))
}
pub(crate) fn persist_proposal(profile:&Path,g:&graph::EvidenceGraph,
    status:&classifier::StatusReport,p:&synthesis::ProjectTaskProposal,
    raw_reply:&str)->Result<(),String>{
    synthesis::audit_proposal(p,g,status)?;
    let observed=format!("{:x}",Sha256::digest(raw_reply.as_bytes()));
    if raw_reply.len()>24*1024||observed!=p.original_reply_sha256{
        return Err("PLAN04_ORIGINAL_MODEL_REPLY_NOT_VERIFIED".into())
    }
    project_status_desktop::store_status(profile,g,status)?;
    let db=profile.join("ansf/library.db");
    let meta=fs::symlink_metadata(&db)
        .map_err(|_|"PLAN04_EXISTING_ANSF_LIBRARY_MISSING")?;
    if !meta.is_file()||meta.file_type().is_symlink(){
        return Err("PLAN04_ANSF_LIBRARY_UNSAFE".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return Err("PLAN04_ANSF_REPARSE_DENIED".into())}
    }
    let mut conn=Connection::open_with_flags(&db,OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_|"PLAN04_ANSF_OPEN_DENIED")?;
    conn.busy_timeout(Duration::from_secs(3))
        .map_err(|_|"PLAN04_ANSF_LOCK_FAILURE")?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS ansf_project_task_proposals(
        proposal_sha256 TEXT PRIMARY KEY,
        graph_sha256 TEXT NOT NULL,
        classification_sha256 TEXT NOT NULL,
        model_ref TEXT NOT NULL,
        session_id TEXT NOT NULL,
        generation_id TEXT NOT NULL,
        original_reply_sha256 TEXT NOT NULL,
        original_reply_json BLOB NOT NULL,
        proposal_json BLOB NOT NULL,
        human_review_required INTEGER NOT NULL CHECK(human_review_required=1),
        execution_authorized INTEGER NOT NULL CHECK(execution_authorized=0)
    );").map_err(|_|"PLAN04_ANSF_SCHEMA_DENIED")?;
    let prompt=synthesis::model_prompt(&synthesis::build_capsule(g,status)?)?;
    let reconstructed=synthesis::validate_model_response(
        g,status,raw_reply,&prompt,&p.original_model_ref,
        &p.original_session_id,&p.original_generation_id
    )?;
    let bytes=serde_json::to_vec(p).map_err(|_|"PLAN04_SERIALIZATION")?;
    if bytes.len()>512*1024||
        serde_json::to_vec(&reconstructed).map_err(|_|"PLAN04_RECONSTRUCTION_ENCODE")?!=bytes{
        return Err("PLAN04_PROPOSAL_NOT_RECONSTRUCTIBLE_FROM_ORIGINAL_GGUF_REPLY".into())
    }
    let tx=conn.transaction().map_err(|_|"PLAN04_TRANSACTION")?;
    tx.execute(
        "INSERT OR IGNORE INTO ansf_project_task_proposals
        (proposal_sha256,graph_sha256,classification_sha256,model_ref,session_id,
         generation_id,original_reply_sha256,original_reply_json,proposal_json,human_review_required,
         execution_authorized) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,1,0)",
        params![
            p.digest_sha256,p.graph_sha256,p.status_sha256,p.original_model_ref,
            p.original_session_id,p.original_generation_id,p.original_reply_sha256,
            raw_reply.as_bytes(),bytes,
        ]
    ).map_err(|_|"PLAN04_DURABLE_APPEND")?;
    let (stored,reply):(Vec<u8>,Vec<u8>)=tx.query_row(
        "SELECT proposal_json,original_reply_json FROM ansf_project_task_proposals WHERE proposal_sha256=?1",
        [&p.digest_sha256],|r|Ok((r.get(0)?,r.get(1)?))
    ).map_err(|_|"PLAN04_READBACK_MISSING")?;
    if reply!=raw_reply.as_bytes()||
        format!("{:x}",Sha256::digest(&reply))!=p.original_reply_sha256{
        return Err("PLAN04_DURABLE_ORIGINAL_GGUF_REPLY_MISMATCH".into())
    }
    let reopened:synthesis::ProjectTaskProposal=serde_json::from_slice(&stored)
        .map_err(|_|"PLAN04_READBACK_INVALID")?;
    synthesis::audit_proposal(&reopened,g,status)?;
    if stored!=bytes{return Err("PLAN04_IMMUTABLE_ORIGINAL_CONFLICT".into())}
    tx.commit().map_err(|_|"PLAN04_DURABLE_COMMIT")?;
    Ok(())
}
#[tauri::command]
pub(crate) async fn lu_project_generate_tasks(
    app:AppHandle,caller:tauri::WebviewWindow,state:State<'_,Arc<DesktopState>>
)->Result<Option<synthesis::ProjectTaskProposal>,String>{
    if caller.label()!="settings"{
        return Err("PLAN04_OPERATOR_MUST_SELECT_FROM_NATIVE_SETTINGS".into())
    }
    let owner=Arc::clone(state.inner());
    let initial=owner.supervisor.status()
        .map_err(|e|format!("PLAN04_LOCAL_MODEL_UNAVAILABLE: {e:?}"))?;
    if initial.phase!=lu_core::inference_host::EnginePhase::Ready||
       initial.session_id.is_none()||initial.model_ref.is_none(){
        return Err("PLAN04_GGUF_NOT_LOADED: load an actual GGUF before generating tasks".into())
    }
    let confirmed=tauri::async_runtime::spawn_blocking({
        let native=app.clone();
        move||native.dialog().message(
            "Use the currently loaded local GGUF to propose evidence-backed \
             engineering tasks from a read-only project inspection? \
             Lu will select a folder next, keep original source and model \
             receipts in your existing ANSF library, and independently reject \
             invented citations, stale branches, cycles and invalid status. \
             This generates a proposal for your review only: no tasks run, \
             GitHub issues are created or permissions granted."
        ).title("Lu — Review local GGUF project analysis")
        .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
            "Generate read-only proposal".into(),"Cancel".into()))
        .blocking_show()
    }).await.map_err(|_|"PLAN04_OS_REVIEW_DIALOG_UNAVAILABLE")?;
    if !confirmed{return Ok(None)}
    let selected=tauri::async_runtime::spawn_blocking({
        let chooser=app.clone();
        move||chooser.dialog().file().blocking_pick_folder()
    }).await.map_err(|_|"PLAN04_NATIVE_FOLDER_PICKER_FAILED")?;
    let Some(selected)=selected else{return Ok(None)};
    let root=selected.into_path().map_err(|_|"PLAN04_INVALID_FOLDER_SELECTION")?;
    let profile=owner.catalog_path.parent()
        .ok_or("PLAN04_EXISTING_LU_PROFILE_MISSING")?.to_path_buf();
    let sid=initial.session_id.ok_or("PLAN04_MODEL_SESSION_LOST")?;
    let model=initial.model_ref.ok_or("PLAN04_MODEL_REF_LOST")?;
    let result=tauri::async_runtime::spawn_blocking(move||{
        let (g,status)=scan_and_classify(&root)?;
        let capsule=synthesis::build_capsule(&g,&status)?;
        if capsule.requirements.is_empty(){
            return Err("PLAN04_INCOMPLETE_NO_UNRESOLVED_REQUIREMENT_EVIDENCE".into())
        }
        let prompt=synthesis::model_prompt(&capsule)?;
        let current=owner.supervisor.status()
            .map_err(|_|"PLAN04_GGUF_SESSION_UNAVAILABLE")?;
        if current.phase!=lu_core::inference_host::EnginePhase::Ready||
            current.session_id.as_deref()!=Some(sid.as_str())||
            current.model_ref.as_deref()!=Some(model.as_str()){
            return Err("PLAN04_GGUF_SESSION_CHANGED_BEFORE_GENERATION".into())
        }
        // The same FIFO scheduler used by normal chat is mandatory.
        // This is a synthetic host-only queue label, NEVER a human chat turn.
        let generation=next_id("g-");
        let request=next_id("req-");
        let (ticket,_)=owner.generation_queue.enqueue(
            &sid,"lu-plan04-host-analysis",&generation
        )?;
        ticket.wait_for_slot()?;
        if ticket.cancelled(){return Err("PLAN04_INCOMPLETE_CANCELLED".into())}
        let history=vec![]; // no web, tools, old chat or mutable permissions
        owner.supervisor.reserve_generation_with_history(
            &sid,&generation,&request,&prompt,3072,&history
        ).map_err(|e|format!("PLAN04_REAL_GGUF_RESERVATION_FAILED: {e:?}"))?;
        if ticket.cancelled(){
            let _=owner.supervisor.cancel(&sid,&generation,&next_id("req-"));
            return Err("PLAN04_INCOMPLETE_CANCELLED".into())
        }
        let mut raw=String::new();
        let emit=|wire:Vec<u8>|->Result<(),BridgeError>{
            let frame=decode_frame(&wire).map_err(BridgeError::Wire)?;
            if let Some(proto::inference_frame::Body::Event(event))=frame.body{
                if let Some(proto::inference_event::Kind::TextChunk(chunk))=event.kind{
                    if raw.len().saturating_add(chunk.text.len())>24*1024{
                        return Err(BridgeError::WrongOperation)
                    }
                    raw.push_str(&chunk.text);
                }
            }
            Ok(())
        };
        let exact_prompt=prompt.clone();
        owner.supervisor.run_reserved_with_history(
            &sid,&generation,&request,prompt,3072,history,emit
        ).map_err(|e|format!("PLAN04_INCOMPLETE_ACTUAL_GGUF_INFERENCE_FAILED: {e:?}"))?;
        if ticket.cancelled(){return Err("PLAN04_INCOMPLETE_CANCELLED".into())}
        let original=synthesis::validate_model_response(
            &g,&status,&raw,&exact_prompt,&model,&sid,&generation
        ).map_err(|e|format!("PLAN04_INCOMPLETE_REJECTED_MODEL_OUTPUT: {e}"))?;
        // A genuine local completion is not enough. Re-read source/branch
        // AFTER inference, then reject outdated observations.
        let (latest,after)=scan_and_classify(&root)?;
        if !synthesis::same_observed_sources(&g,&latest)?||
            status.findings.len()!=after.findings.len()||
            status.findings.iter().zip(after.findings.iter()).any(|(x,y)|
                x.requirement_id!=y.requirement_id||
                x.primary!=y.primary||
                x.evidence_health!=y.evidence_health||
                x.conflicting_claims!=y.conflicting_claims){
            return Err("PLAN04_SOURCE_CHANGED_DURING_GGUF_GENERATION".into())
        }
        let current=owner.supervisor.status()
            .map_err(|_|"PLAN04_GGUF_SESSION_UNAVAILABLE_AFTER_GENERATION")?;
        if current.session_id.as_deref()!=Some(sid.as_str())||
            current.model_ref.as_deref()!=Some(model.as_str()){
            return Err("PLAN04_GGUF_SESSION_CHANGED_DURING_GENERATION".into())
        }
        persist_proposal(&profile,&g,&status,&original,&raw)?;
        Ok(Some(original))
    }).await.map_err(|_|"PLAN04_HOST_ANALYSIS_THREAD_INTERRUPTED")?;
    result
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn actual_ansf_database_is_append_only_and_draft_is_never_authorized(){
        let profile=std::env::temp_dir().join(format!(
            "lu-plan04-test-{}",std::process::id()));
        let root=profile.join("repo");
        let _=fs::remove_dir_all(&profile);
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(profile.join("ansf")).unwrap();
        fs::write(root.join("docs/requirements.md"),
            "# Requirements\n- [ ] Implement source-backed planning\n").unwrap();
        let db=profile.join("ansf/library.db");
        let conn=Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE original_memory(x INTEGER);
                            INSERT INTO original_memory VALUES (7);").unwrap();
        drop(conn);
        let (g,r)=scan_and_classify(&root).unwrap();
        let cap=synthesis::build_capsule(&g,&r).unwrap();
        assert_eq!(cap.requirements.len(),1);
        let req=&cap.requirements[0];
        let raw=serde_json::json!({"schema":synthesis::RAW_SCHEMA,"tasks":[{
            "requirementId":req.requirement_id,"status":"UNKNOWN",
            "summary":"Inspect source-bound planning gaps",
            "description":"Inspect project source and record missing acceptance evidence",
            "priority":"P0","group":"01 Evidence verification",
            "evidenceIds":[req.requirement_id],
            "dependencyIndices":[],
            "acceptanceCriteria":"Verify original source and native end-to-end acceptance",
            "blockedBy":"No trusted native outcome yet"
        }]}).to_string();
        let prompt=synthesis::model_prompt(&cap).unwrap();
        let proposal=synthesis::validate_model_response(&g,&r,&raw,&prompt,
            "m-test","s-real","g-original").unwrap();
        persist_proposal(&profile,&g,&r,&proposal,&raw).unwrap();
        persist_proposal(&profile,&g,&r,&proposal,&raw).unwrap();
        let c=Connection::open(&db).unwrap();
        let count:i64=c.query_row(
            "SELECT COUNT(*) FROM ansf_project_task_proposals",[],|r|r.get(0)
        ).unwrap();
        let original:i64=c.query_row(
            "SELECT x FROM original_memory",[],|r|r.get(0)
        ).unwrap();
        let stored:Vec<u8>=c.query_row(
            "SELECT proposal_json FROM ansf_project_task_proposals",[],|r|r.get(0)
        ).unwrap();
        assert_eq!((count,original),(1,7));
        assert!(String::from_utf8_lossy(&stored).contains("humanReviewRequired"));
        drop(c);
        fs::remove_dir_all(profile).unwrap();
    }
}
