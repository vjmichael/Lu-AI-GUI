//! PLAN-07A host: a read-only native source scan -> audited delta -> human
//! reviewed append-only PLAN05 SourceChanged revision, never task completion.
//! This module cannot route to GGUF, Git mutation, shell, or worker grants.
use super::*;
use lu_core::{project_task_updates as updates,project_evidence_graph as graph,
    project_status_classifier as status,project_task_register as register};
use rusqlite::{Connection,params,TransactionBehavior};
use std::path::PathBuf;

fn task_window_only(caller:&tauri::WebviewWindow)->Result<(),String>{
    if caller.label()!="task-list"{
        return Err("PLAN07_REQUIRES_INDEPENDENT_HUMAN_TASK_WINDOW".into())
    }
    Ok(())
}
fn profile(state:&Arc<DesktopState>)->Result<PathBuf,String>{
    project_register_desktop::profile(state)
}
fn create_table(conn:&Connection)->Result<(),String>{
    conn.execute_batch("CREATE TABLE IF NOT EXISTS ansf_project_task_refresh_observations(
        preview_sha256 TEXT PRIMARY KEY,
        register_id TEXT NOT NULL,
        expected_register_sha256 TEXT NOT NULL,
        previous_graph_sha256 TEXT NOT NULL,
        current_graph_sha256 TEXT NOT NULL,
        current_status_sha256 TEXT NOT NULL,
        preview_json BLOB NOT NULL,
        no_execution_authority INTEGER NOT NULL CHECK(no_execution_authority=1)
    );").map_err(|_|"PLAN07_ANSF_REFRESH_SCHEMA_FAILED")
}
fn saved_preview(profile:&Path,preview_sha256:&str)
 ->Result<updates::Preview,String>{
    if !graph::valid_digest(preview_sha256){
        return Err("PLAN07_INVALID_PREVIEW_DIGEST".into())
    }
    let conn=project_register_desktop::library(profile)?;
    create_table(&conn)?;
    let bytes:Vec<u8>=conn.query_row(
        "SELECT preview_json FROM ansf_project_task_refresh_observations
         WHERE preview_sha256=?1 AND no_execution_authority=1",
        [preview_sha256],|row|row.get(0)
    ).map_err(|_|"PLAN07_UNARCHIVED_PREVIEW")?;
    if bytes.len()>512*1024{
        return Err("PLAN07_OVERSIZED_PREVIEW".into())
    }
    let preview:updates::Preview=serde_json::from_slice(&bytes)
        .map_err(|_|"PLAN07_PREVIEW_BYTES_INVALID")?;
    if preview.digest_sha256!=preview_sha256||
       !preview.no_execution_authority||preview.model_text_is_evidence{
        return Err("PLAN07_PREVIEW_PROVENANCE_INVALID".into())
    }
    Ok(preview)
}
fn analyze_selected(profile:&Path,root:&Path,register_id:&str)
 ->Result<updates::Preview,String>{
    if !register_id.starts_with("lu-register-"){
        return Err("PLAN07_REGISTER_ID_INVALID".into())
    }
    let current=project_status_desktop::classify_selected(root)?;
    let conn=project_register_desktop::library(profile)?;
    let (proposal,register,_)=project_register_desktop::load_register(&conn,register_id)?
        .ok_or("PLAN07_ORIGINAL_REGISTER_NOT_FOUND")?;
    if register.scope_sha256!=current.0.project_scope_sha256{
        return Err("PLAN07_SELECTED_PROJECT_SCOPE_MISMATCH".into())
    }
    let last=register.events.last().ok_or("PLAN07_EMPTY_REGISTER")?;
    let (previous_g,previous_s)=project_register_desktop::archived_evidence(
        &conn,&last.revision.graph_sha256,&last.revision.status_sha256)?;
    let preview=updates::analyze(&register,&proposal,&previous_g,&previous_s,
        &current.0,&current.1)?;
    drop(conn);
    // Persist verified full current source snapshot before linking the
    // proposed differences to it. No task status is updated by previewing.
    project_status_desktop::store_status(profile,&current.0,&current.1)?;
    let mut conn=project_register_desktop::library(profile)?;
    create_table(&conn)?;
    let bytes=serde_json::to_vec(&preview)
        .map_err(|_|"PLAN07_PREVIEW_ENCODE_FAILED")?;
    if bytes.len()>512*1024{return Err("PLAN07_PREVIEW_BUDGET_EXCEEDED".into())}
    let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_|"PLAN07_PREVIEW_BEGIN_IMMEDIATE")?;
    tx.execute(
        "INSERT OR IGNORE INTO ansf_project_task_refresh_observations(
        preview_sha256,register_id,expected_register_sha256,previous_graph_sha256,
        current_graph_sha256,current_status_sha256,preview_json,no_execution_authority)
        VALUES(?1,?2,?3,?4,?5,?6,?7,1)",
        params![preview.digest_sha256,preview.register_id,
            preview.expected_register_sha256,preview.previous_graph_sha256,
            preview.current_graph_sha256,preview.current_status_sha256,bytes]
    ).map_err(|_|"PLAN07_PREVIEW_APPEND_FAILED")?;
    let original:Vec<u8>=tx.query_row(
        "SELECT preview_json FROM ansf_project_task_refresh_observations
         WHERE preview_sha256=?1",[&preview.digest_sha256],|row|row.get(0)
    ).map_err(|_|"PLAN07_PREVIEW_READBACK_MISSING")?;
    if original!=bytes{return Err("PLAN07_PREVIEW_IMMUTABLE_COLLISION".into())}
    tx.commit().map_err(|_|"PLAN07_PREVIEW_COMMIT_FAILED")?;
    Ok(preview)
}
fn reload_audited(profile:&Path,preview_id:&str)
 ->Result<(updates::Preview,register::TaskRegister,
    lu_core::project_task_synthesis::ProjectTaskProposal,
    graph::EvidenceGraph,status::StatusReport),String>{
    let preview=saved_preview(profile,preview_id)?;
    let conn=project_register_desktop::library(profile)?;
    let (proposal,register,_)=project_register_desktop::load_register(
        &conn,&preview.register_id)?.ok_or("PLAN07_REGISTER_NOT_FOUND")?;
    if register.latest_sha256!=preview.expected_register_sha256{
        return Err("PLAN07_REGISTER_CHANGED_AFTER_PREVIEW".into())
    }
    let (old_g,old_s)=project_register_desktop::archived_evidence(
        &conn,&preview.previous_graph_sha256,&preview.previous_status_sha256)?;
    let (current_g,current_s)=project_register_desktop::archived_evidence(
        &conn,&preview.current_graph_sha256,&preview.current_status_sha256)?;
    updates::audit(&preview,&register,&proposal,&old_g,&old_s,
        &current_g,&current_s)?;
    Ok((preview,register,proposal,current_g,current_s))
}
fn same_native_observation(
    archived:&(graph::EvidenceGraph,status::StatusReport),
    observed:&(graph::EvidenceGraph,status::StatusReport)
)->Result<bool,String>{
    Ok(lu_core::project_task_synthesis::same_observed_sources(
        &archived.0,&observed.0)?&&
        archived.1.complete==observed.1.complete&&
        serde_json::to_value(&archived.1.findings)
            .map_err(|_|"PLAN07_ARCHIVED_FINDINGS_ENCODE")?==
        serde_json::to_value(&observed.1.findings)
            .map_err(|_|"PLAN07_OBSERVED_FINDINGS_ENCODE")?)
}
fn recheck_and_commit(profile:&Path,root:&Path,preview_id:&str)
 ->Result<register::TaskRegister,String>{
    // OS approval alone is not source evidence: independently recheck every
    // native condition *after* the dialog and new OS folder selection.
    let (preview,register,proposal,current_g,current_s)=
        reload_audited(profile,preview_id)?;
    if !preview.semantic_change{
        return Err("PLAN07_NO_SEMANTIC_DELTA_TO_RECONCILE".into())
    }
    let observed=project_status_desktop::classify_selected(root)?;
    if observed.0.project_scope_sha256!=register.scope_sha256||
       !same_native_observation(&(current_g.clone(),current_s.clone()),&observed)?{
        return Err("PLAN07_SOURCE_CHANGED_AFTER_OPERATOR_REVIEW".into())
    }
    // Old source remains a separate original historical fact. Never copy a
    // new ACCEPTED classification directly into "task completed".
    let next=register::source_changed(&register,&proposal,&current_g,&current_s)?;
    if next.events.len()<=register.events.len(){
        return Err("PLAN07_CHANGED_SOURCE_DID_NOT_APPEND_REVIEW".into())
    }
    project_register_desktop::persist(profile,&proposal,&next,
        Some(&register.latest_sha256))
}
#[tauri::command]
pub(crate) async fn lu_project_update_inspect(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,register_id:String
)->Result<Option<updates::Preview>,String>{
    task_window_only(&caller)?;
    if !register_id.starts_with("lu-register-"){
        return Err("PLAN07_INVALID_HUMAN_SELECTED_REGISTER".into())
    }
    let chooser=app.clone();
    let selected=tauri::async_runtime::spawn_blocking(move||
        chooser.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"PLAN07_NATIVE_PROJECT_PICKER_INTERRUPTED")?;
    let Some(selected)=selected else{return Ok(None)};
    let root=selected.into_path().map_err(|_|"PLAN07_NATIVE_PATH_INVALID")?;
    let profile=profile(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||
        analyze_selected(&profile,&root,&register_id).map(Some)
    ).await.map_err(|_|"PLAN07_EVIDENCE_REFRESH_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_update_apply(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,preview_sha256:String
)->Result<Option<register::TaskRegister>,String>{
    task_window_only(&caller)?;
    let profile=profile(state.inner())?;
    let preview_id=preview_sha256.clone();
    let profile_preview=profile.clone();
    let preview=tauri::async_runtime::spawn_blocking(move||
        reload_audited(&profile_preview,&preview_id).map(|tuple|tuple.0)
    ).await.map_err(|_|"PLAN07_PREVIEW_READ_INTERRUPTED")??;
    if !preview.semantic_change{
        return Err("PLAN07_NO_SOURCE_CHANGE_TO_APPLY".into())
    }
    let details=preview.candidates.iter().filter(|c|c.requires_human_review)
        .take(16).map(|c|format!(
            "{} v{}: {:?}, original requirement {}, current status {:?}",
            c.task_id,c.expected_version,c.disposition,
            c.original_requirement_id,c.current_status
        )).collect::<Vec<_>>().join("\n");
    if !project_register_desktop::confirm(&app,
        "Lu — Review changed native task evidence",
        format!("Reconcile these NATIVE source differences with your original \
            durable task register?\n\nOriginal HEAD: {}\nCurrent HEAD: {}\n\
            Changed source nodes: {}\n{}\
            \n\nEvery affected task remains NEEDS HUMAN REVIEW. \
            An ACCEPTED exact path is not proof of completed work. \
            No GGUF claim, task completion, worker, Git or file effect \
            is authorized. Select the SAME original project again next.",
            preview.previous_head.as_deref().unwrap_or("not verified"),
            preview.current_head.as_deref().unwrap_or("not verified"),
            preview.changed_evidence_nodes,details)
    ).await?{return Ok(None)}
    let chooser=app.clone();
    let selected=tauri::async_runtime::spawn_blocking(move||
        chooser.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"PLAN07_RECHECK_FOLDER_PICKER_INTERRUPTED")?;
    let Some(selected)=selected else{return Ok(None)};
    let root=selected.into_path().map_err(|_|"PLAN07_RECHECK_PATH_INVALID")?;
    tauri::async_runtime::spawn_blocking(move||
        recheck_and_commit(&profile,&root,&preview_sha256).map(Some)
    ).await.map_err(|_|"PLAN07_RECONCILIATION_INTERRUPTED")?
}
