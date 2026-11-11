//! PLAN-05: operator-owned task register in the EXISTING ANSF SQLite library.
//! All read/write commands are Settings-only. No model, worker or tool gateway.
use super::*;
use lu_core::{project_evidence_graph as graph,project_status_classifier as status,
    project_task_synthesis as synthesis,project_task_register as register};
use rusqlite::{Connection,OpenFlags,params,OptionalExtension,TransactionBehavior};

fn library(profile:&Path)->Result<Connection,String>{
    let db=profile.join("ansf/library.db");
    let meta=fs::symlink_metadata(&db).map_err(|_|"PLAN05_ANSF_LIBRARY_MISSING")?;
    if !meta.is_file()||meta.file_type().is_symlink(){
        return Err("PLAN05_EXISTING_LIBRARY_NOT_REGULAR".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err("PLAN05_ANSF_REPARSE_DENIED".into())
        }
    }
    let conn=Connection::open_with_flags(db,OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_|"PLAN05_ANSF_OPEN_FAILED")?;
    conn.busy_timeout(Duration::from_secs(3))
        .map_err(|_|"PLAN05_ANSF_BUSY_TIMEOUT")?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS ansf_project_task_register_revisions(
        register_id TEXT NOT NULL,
        register_revision INTEGER NOT NULL CHECK(register_revision>0),
        original_proposal_sha256 TEXT NOT NULL,
        previous_sha256 TEXT,
        latest_sha256 TEXT NOT NULL,
        register_json BLOB NOT NULL,
        no_execution_authority INTEGER NOT NULL CHECK(no_execution_authority=1),
        PRIMARY KEY(register_id,register_revision)
    );").map_err(|_|"PLAN05_SCHEMA_FAILED")?;
    Ok(conn)
}
fn archived_proposal(conn:&Connection,digest:&str)
 ->Result<synthesis::ProjectTaskProposal,String>{
    let (bytes,raw):(Vec<u8>,Vec<u8>)=conn.query_row(
        "SELECT proposal_json,original_reply_json FROM ansf_project_task_proposals WHERE proposal_sha256=?1",
        [digest],|row|Ok((row.get(0)?,row.get(1)?))
    ).map_err(|_|"PLAN05_ORIGINAL_GGUF_PROPOSAL_NOT_FOUND")?;
    let p:synthesis::ProjectTaskProposal=serde_json::from_slice(&bytes)
        .map_err(|_|"PLAN05_PROPOSAL_BYTES_INVALID")?;
    if p.digest_sha256!=digest||!p.human_review_required||
        !p.no_execution_authority||raw.len()>24*1024{
        return Err("PLAN05_PROPOSAL_AUTHORITY_INVALID".into())
    }
    let (g,s)=archived_evidence(conn,&p.graph_sha256,&p.status_sha256)?;
    synthesis::audit_proposal(&p,&g,&s)?;
    let original=String::from_utf8(raw).map_err(|_|"PLAN05_ORIGINAL_GGUF_UTF8")?;
    let prompt=synthesis::model_prompt(&synthesis::build_capsule(&g,&s)?)?;
    let reconstructed=synthesis::validate_model_response(&g,&s,&original,&prompt,
        &p.original_model_ref,&p.original_session_id,&p.original_generation_id)?;
    if serde_json::to_vec(&reconstructed).map_err(|_|"PLAN05_RECONSTRUCT_ENCODE")?!=bytes{
        return Err("PLAN05_MODEL_DRAFT_NO_LONGER_RECONSTRUCTIBLE".into())
    }
    Ok(p)
}
fn archived_evidence(conn:&Connection,graph_id:&str,status_id:&str)
 ->Result<(graph::EvidenceGraph,status::StatusReport),String>{
    let original_graph:Vec<u8>=conn.query_row(
        "SELECT snapshot_json FROM ansf_project_evidence_snapshots WHERE graph_sha256=?1",
        [graph_id],|row|row.get(0)
    ).map_err(|_|"PLAN05_ORIGINAL_GRAPH_NOT_FOUND")?;
    let original_status:Vec<u8>=conn.query_row(
        "SELECT snapshot_json FROM ansf_project_status_snapshots WHERE classification_sha256=?1",
        [status_id],|row|row.get(0)
    ).map_err(|_|"PLAN05_ORIGINAL_STATUS_NOT_FOUND")?;
    let g:graph::EvidenceGraph=serde_json::from_slice(&original_graph)
        .map_err(|_|"PLAN05_ORIGINAL_GRAPH_INVALID")?;
    let s:status::StatusReport=serde_json::from_slice(&original_status)
        .map_err(|_|"PLAN05_ORIGINAL_STATUS_INVALID")?;
    graph::audit(&g)?;status::audit(&s,&g)?;
    if g.sha256!=graph_id||s.digest_sha256!=status_id{
        return Err("PLAN05_ARCHIVED_EVIDENCE_ID_MISMATCH".into())
    }
    Ok((g,s))
}
fn load_register(conn:&Connection,id:&str)
 ->Result<Option<(synthesis::ProjectTaskProposal,register::TaskRegister,u64)>,String>{
    let mut stmt=conn.prepare("SELECT register_revision,original_proposal_sha256,
        previous_sha256,latest_sha256,register_json,no_execution_authority
        FROM ansf_project_task_register_revisions WHERE register_id=?1
        ORDER BY register_revision ASC")
        .map_err(|_|"PLAN05_HISTORY_QUERY")?;
    let mut rows=stmt.query([id]).map_err(|_|"PLAN05_HISTORY_OPEN")?;
    let mut prior:Option<register::TaskRegister>=None;
    let mut proposal:Option<synthesis::ProjectTaskProposal>=None;
    let mut n=0_u64;
    while let Some(row)=rows.next().map_err(|_|"PLAN05_HISTORY_READ")?{
        let revision:i64=row.get(0).map_err(|_|"PLAN05_HISTORY_SEQUENCE")?;
        let proposal_digest:String=row.get(1).map_err(|_|"PLAN05_HISTORY_PROPOSAL")?;
        let previous:Option<String>=row.get(2).map_err(|_|"PLAN05_HISTORY_PREVIOUS")?;
        let latest:String=row.get(3).map_err(|_|"PLAN05_HISTORY_LATEST")?;
        let bytes:Vec<u8>=row.get(4).map_err(|_|"PLAN05_HISTORY_BYTES")?;
        let no_exec:i64=row.get(5).map_err(|_|"PLAN05_HISTORY_AUTHORITY")?;
        if revision!=n as i64+1||no_exec!=1||bytes.len()>1024*1024{
            return Err("PLAN05_MISSING_OR_OVERSIZED_REVISION".into())
        }
        if proposal.is_none(){proposal=Some(archived_proposal(conn,&proposal_digest)?);}
        let p=proposal.as_ref().ok_or("PLAN05_PROPOSAL_REQUIRED")?;
        if proposal_digest!=p.digest_sha256{
            return Err("PLAN05_REGISTER_CHANGED_ORIGIN".into())
        }
        let decoded:register::TaskRegister=serde_json::from_slice(&bytes)
            .map_err(|_|"PLAN05_REGISTER_JSON_CORRUPT")?;
        register::audit(&decoded,p)?;
        if decoded.register_id!=id||decoded.latest_sha256!=latest||
            previous!=prior.as_ref().map(|r|r.latest_sha256.clone()){
            return Err("PLAN05_SNAPSHOT_CHAIN_BROKEN".into())
        }
        if let Some(old)=&prior{
            if decoded.events.len()<=old.events.len()||
                serde_json::to_vec(&decoded.events[..old.events.len()])
                    .map_err(|_|"PLAN05_OLD_EVENTS_SERIALIZE")?!=
                serde_json::to_vec(&old.events).map_err(|_|"PLAN05_OLD_EVENTS_SERIALIZE")?{
                return Err("PLAN05_HISTORICAL_REVISIONS_OVERWRITTEN".into())
            }
        }
        prior=Some(decoded);n+=1;
    }
    match (proposal,prior){
        (Some(p),Some(r))=>Ok(Some((p,r,n))),
        (None,None)=>Ok(None),
        _=>Err("PLAN05_PARTIAL_REGISTER".into())
    }
}
fn persist(profile:&Path,p:&synthesis::ProjectTaskProposal,
    next:&register::TaskRegister,expected:Option<&str>)
 ->Result<register::TaskRegister,String>{
    register::audit(next,p)?;
    let mut conn=library(profile)?;
    let tx=conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|_|"PLAN05_BEGIN_IMMEDIATE_FAILED")?;
    let existing=load_register(&tx,&next.register_id)?;
    let (version,prev)=match existing{
        None=>{
            if expected.is_some(){return Err("PLAN05_EXPECTED_EXISTING_REGISTER".into())}
            (1,None)
        }
        Some((old_proposal,old,n))=>{
            if old_proposal.digest_sha256!=p.digest_sha256||
                expected!=Some(old.latest_sha256.as_str())||
                next.events.len()<=old.events.len()||
                serde_json::to_vec(&next.events[..old.events.len()])
                    .map_err(|_|"PLAN05_HISTORY_ENCODE")?!=
                serde_json::to_vec(&old.events).map_err(|_|"PLAN05_HISTORY_ENCODE")?{
                return Err("PLAN05_STALE_OR_REWRITTEN_REGISTER".into())
            }
            (n+1,Some(old.latest_sha256))
        }
    };
    let bytes=serde_json::to_vec(next).map_err(|_|"PLAN05_REGISTER_ENCODE")?;
    if bytes.len()>1024*1024{return Err("PLAN05_REGISTER_TOO_LARGE".into())}
    tx.execute("INSERT INTO ansf_project_task_register_revisions(
        register_id,register_revision,original_proposal_sha256,previous_sha256,
        latest_sha256,register_json,no_execution_authority)
        VALUES(?1,?2,?3,?4,?5,?6,1)",
        params![next.register_id,version as i64,p.digest_sha256,prev,
            next.latest_sha256,bytes]
    ).map_err(|_|"PLAN05_IMMUTABLE_INSERT_FAILED")?;
    let reopened:Vec<u8>=tx.query_row(
        "SELECT register_json FROM ansf_project_task_register_revisions
         WHERE register_id=?1 AND register_revision=?2",
        params![next.register_id,version as i64],|row|row.get(0)
    ).map_err(|_|"PLAN05_INSERT_READBACK_MISSING")?;
    if reopened!=bytes{return Err("PLAN05_INSERT_BYTE_MISMATCH".into())}
    let redecoded:register::TaskRegister=serde_json::from_slice(&reopened)
        .map_err(|_|"PLAN05_INSERT_READBACK_INVALID")?;
    register::audit(&redecoded,p)?;
    tx.commit().map_err(|_|"PLAN05_FSYNC_COMMIT_FAILED")?;
    Ok(redecoded)
}
fn profile(state:&Arc<DesktopState>)->Result<std::path::PathBuf,String>{
    state.catalog_path.parent()
        .map(Path::to_path_buf).ok_or("PLAN05_LU_PROFILE_MISSING".into())
}
fn settings_only(caller:&tauri::WebviewWindow)->Result<(),String>{
    if caller.label()!="settings"{
        return Err("PLAN05_REQUIRES_INDEPENDENT_OPERATOR_SETTINGS_WINDOW".into())
    }
    Ok(())
}
async fn confirm(app:&AppHandle,title:&str,message:String)
 ->Result<bool,String>{
    let dialog=app.clone();let title=title.to_string();
    tauri::async_runtime::spawn_blocking(move||
        dialog.dialog().message(message).title(title)
            .buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancelCustom(
                "Confirm review only".into(),"Cancel".into()))
            .blocking_show()
    ).await.map_err(|_|"PLAN05_NATIVE_REVIEW_DIALOG_INTERRUPTED".into())
}
async fn pick(app:&AppHandle)->Result<Option<std::path::PathBuf>,String>{
    let native=app.clone();
    let selection=tauri::async_runtime::spawn_blocking(move||
        native.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"PLAN05_NATIVE_PROJECT_PICKER_INTERRUPTED")?;
    selection.map(|p|p.into_path()
        .map_err(|_|"PLAN05_NATIVE_PROJECT_PATH_INVALID".into())).transpose()
}
fn classify_selected(root:&Path)
 ->Result<(graph::EvidenceGraph,status::StatusReport),String>{
    project_status_desktop::classify_selected(root)
}
fn observations_equal(
    a:&(graph::EvidenceGraph,status::StatusReport),
    b:&(graph::EvidenceGraph,status::StatusReport)
)->Result<bool,String>{
    Ok(synthesis::same_observed_sources(&a.0,&b.0)?&&
        a.1.complete==b.1.complete&&
        serde_json::to_value(&a.1.findings).map_err(|_|"PLAN05_STATUS_ENCODE")?==
        serde_json::to_value(&b.1.findings).map_err(|_|"PLAN05_STATUS_ENCODE")?)
}
#[tauri::command]
pub(crate) async fn lu_project_register_proposal(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,proposal_digest:String
)->Result<Option<register::TaskRegister>,String>{
    settings_only(&caller)?;
    if !graph::valid_digest(&proposal_digest){
        return Err("PLAN05_INVALID_PROPOSAL_ID".into())
    }
    if !confirm(&app,"Lu — Import human-reviewed task register",
        "Import this previously saved GGUF proposal into your persistent task register? \
        This records a human-reviewed task list and its original model evidence; \
        it does NOT approve completion, execute tasks, grant permissions or create issues."
            .into()).await?{return Ok(None)}
    let Some(root)=pick(&app).await? else{return Ok(None)};
    let profile=profile(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||{
        let conn=library(&profile)?;
        let p=archived_proposal(&conn,&proposal_digest)?;
        if load_register(&conn,&format!("lu-register-{}",
            &format!("{:x}",Sha256::digest(
                serde_json::to_vec(&(p.project_scope_sha256.as_str(),p.digest_sha256.as_str()))
                    .map_err(|_|"PLAN05_REGISTER_ID_ENCODE")?
            ))[..24]))?.is_some(){
            return Err("PLAN05_REGISTER_ALREADY_IMPORTED".into())
        }
        let original=archived_evidence(&conn,&p.graph_sha256,&p.status_sha256)?;
        drop(conn);
        let current=classify_selected(&root)?;
        if !observations_equal(&original,&current)?{
            return Err("PLAN05_PROPOSAL_SOURCE_CHANGED_IMPORT_REQUIRES_REVIEW".into())
        }
        project_status_desktop::store_status(&profile,&current.0,&current.1)?;
        let registered=register::import(&p)?;
        let saved=persist(&profile,&p,&registered,None)?;
        Ok(Some(saved))
    }).await.map_err(|_|"PLAN05_IMPORT_THREAD_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_list_registers(
    caller:tauri::WebviewWindow,state:State<'_,Arc<DesktopState>>
)->Result<Vec<register::TaskRegister>,String>{
    settings_only(&caller)?;
    let profile=profile(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||{
        let conn=library(&profile)?;
        let mut stmt=conn.prepare(
            "SELECT DISTINCT register_id FROM ansf_project_task_register_revisions
             ORDER BY register_id LIMIT 32"
        ).map_err(|_|"PLAN05_REGISTER_LIST_QUERY")?;
        let ids=stmt.query_map([],|r|r.get::<_,String>(0))
            .map_err(|_|"PLAN05_REGISTER_LIST_OPEN")?
            .collect::<Result<Vec<_>,_>>()
            .map_err(|_|"PLAN05_REGISTER_LIST_READ")?;
        let mut out=Vec::new();
        for id in ids{
            if let Some((_,r,_))=load_register(&conn,&id)?{out.push(r);}
        }
        Ok(out)
    }).await.map_err(|_|"PLAN05_LIST_THREAD_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_revise_task(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,register_id:String,task_id:String,
    expected_version:u64,correction:register::HumanCorrection
)->Result<Option<register::TaskRegister>,String>{
    settings_only(&caller)?;
    if !register_id.starts_with("lu-register-")||!task_id.starts_with("lu-task-"){
        return Err("PLAN05_INVALID_REGISTER_OR_TASK_ID".into())
    }
    let message=format!(
        "Manually correct task {task_id} at version {expected_version}?\n\n\
         Summary: {}\nDescription: {}\nPriority: {}\nAcceptance criterion: {}\n\
         Blocked by: {}\nReason: {}\n\nThis is a human correction only. \
         The original model, source evidence, and old versions are preserved. \
         Nothing is executed or authorized.",
        correction.summary,correction.description,correction.priority,
        correction.acceptance_criteria,correction.blocked_by,correction.reason);
    if !confirm(&app,"Lu — Confirm human task correction",message).await?{
        return Ok(None)
    }
    let profile=profile(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||{
        let conn=library(&profile)?;
        let (p,old,_)=load_register(&conn,&register_id)?
            .ok_or("PLAN05_REGISTER_NOT_FOUND")?;
        drop(conn);
        let revised=register::human_correct(&old,&p,&task_id,
            expected_version,&correction)?;
        let saved=persist(&profile,&p,&revised,Some(&old.latest_sha256))?;
        Ok(Some(saved))
    }).await.map_err(|_|"PLAN05_CORRECTION_THREAD_INTERRUPTED")?
}
fn review_selected(profile:&Path,root:&Path,id:&str,task:Option<(&str,u64)>,
    expected_observation:Option<&(graph::EvidenceGraph,status::StatusReport)>)
 ->Result<register::TaskRegister,String>{
    let current=classify_selected(root)?;
    if expected_observation.is_some_and(|preview|
        !observations_equal(preview,&current).unwrap_or(false)){
        return Err("PLAN05_SOURCE_CHANGED_AFTER_HUMAN_REVIEW".into())
    }
    let conn=library(profile)?;
    let (p,old,_)=load_register(&conn,id)?.ok_or("PLAN05_REGISTER_NOT_FOUND")?;
    if old.scope_sha256!=current.0.project_scope_sha256{
        return Err("PLAN05_SELECTED_PROJECT_NOT_REGISTER_SCOPE".into())
    }
    let task_info=task.map(|(task_id,version)|(task_id.to_string(),version));
    let last_task=if let Some((task_id,expected))=&task_info{
        let v=register::latest(&old,task_id).ok_or("PLAN05_TASK_NOT_FOUND")?;
        if v.version!=*expected{return Err("PLAN05_STALE_TASK_REVIEW".into())}
        Some((v.graph_sha256.clone(),v.status_sha256.clone()))
    }else{None};
    let last=old.events.last().ok_or("PLAN05_REGISTER_EMPTY")?;
    let previous=archived_evidence(&conn,&last.revision.graph_sha256,
        &last.revision.status_sha256)?;
    drop(conn);
    project_status_desktop::store_status(profile,&current.0,&current.1)?;
    let is_new=!observations_equal(&previous,&current)?;
    let next=if is_new{
        register::source_changed(&old,&p,&current.0,&current.1)?
    }else{old.clone()};
    if next.events.len()>old.events.len(){
        return persist(profile,&p,&next,Some(&old.latest_sha256))
    }
    if let Some((task_id,expected))=task_info{
        let (prior_graph,prior_status)=last_task.ok_or("PLAN05_TASK_HISTORY_MISSING")?;
        let conn=library(profile)?;
        let prior=archived_evidence(&conn,&prior_graph,&prior_status)?;
        drop(conn);
        if !observations_equal(&prior,&current)?{
            let drift=register::source_changed(&old,&p,&current.0,&current.1)?;
            if drift.events.len()>old.events.len(){
                return persist(profile,&p,&drift,Some(&old.latest_sha256))
            }
            return Err("PLAN05_TASK_SOURCE_CHANGED_BEFORE_REVALIDATION".into())
        }
        let checked=register::human_revalidate(&old,&p,&task_id,expected,
            &current.0,&current.1)?;
        return persist(profile,&p,&checked,Some(&old.latest_sha256))
    }
    Ok(old)
}
#[tauri::command]
pub(crate) async fn lu_project_reconcile_register(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,register_id:String
)->Result<Option<register::TaskRegister>,String>{
    settings_only(&caller)?;
    if !register_id.starts_with("lu-register-"){
        return Err("PLAN05_INVALID_REGISTER_ID".into())
    }
    if !confirm(&app,"Lu — Recheck original project",
        "Reinspect the original project and compare its current source, exact HEAD \
         and independent status with this versioned register? Changes remain \
         flagged for human review; old acceptance records are kept as history."
            .into()).await?{return Ok(None)}
    let Some(root)=pick(&app).await? else{return Ok(None)};
    let profile=profile(state.inner())?;
    tauri::async_runtime::spawn_blocking(move||
        review_selected(&profile,&root,&register_id,None,None).map(Some)
    ).await.map_err(|_|"PLAN05_RECONCILIATION_THREAD_INTERRUPTED")?
}
#[tauri::command]
pub(crate) async fn lu_project_revalidate_task(
    app:AppHandle,caller:tauri::WebviewWindow,
    state:State<'_,Arc<DesktopState>>,register_id:String,task_id:String,
    expected_version:u64
)->Result<Option<register::TaskRegister>,String>{
    settings_only(&caller)?;
    if !register_id.starts_with("lu-register-")||!task_id.starts_with("lu-task-"){
        return Err("PLAN05_INVALID_TASK_REVIEW_IDS".into())
    }
    let Some(root)=pick(&app).await? else{return Ok(None)};
    let profile=profile(state.inner())?;
    // Show the ACTUAL current native classifier result in the OS dialog,
    // not merely a pre-scan promise that an unknown status is accepted.
    let root_preview=root.clone();
    let profile_preview=profile.clone();
    let register_preview=register_id.clone();
    let task_preview=task_id.clone();
    let (observation,message)=tauri::async_runtime::spawn_blocking(move||{
        let observed=classify_selected(&root_preview)?;
        let conn=library(&profile_preview)?;
        let (p,r,_)=load_register(&conn,&register_preview)?
            .ok_or("PLAN05_REGISTER_NOT_FOUND")?;
        let old=register::latest(&r,&task_preview)
            .ok_or("PLAN05_TASK_NOT_FOUND")?;
        if old.version!=expected_version||
            p.project_scope_sha256!=observed.0.project_scope_sha256{
            return Err("PLAN05_STALE_OR_FOREIGN_NATIVE_REVIEW".into())
        }
        let finding=observed.1.findings.iter()
            .find(|f|f.requirement_id==old.source_requirement_id);
        let status_name=finding.map(|f|format!("{:?}",f.primary))
            .unwrap_or_else(||"SOURCE REQUIREMENT NOT FOUND".into());
        let exact_accepted=finding.is_some_and(|f|
            observed.0.complete&&observed.1.complete&&
            f.primary==status::Status::Accepted&&
            f.evidence_health==status::EvidenceHealth::Current&&
            !f.conflicting_claims);
        let message=format!(
            "Review task {} version {} against ACTUAL selected source?\n\n\
             Task: {}\nAcceptance criterion: {}\n\
             Original requirement: {}\n\
             Native classifier: {}\n\
             Exact selected HEAD: {}\n\
             Graph SHA-256: {}\n\
             Independently verified exact requirement: {}\n\n\
             This never establishes that the task was executed, installed, \
             merged, scheduled, or authorized. If the source has changed, Lu \
             will append a needs-review revision rather than reuse acceptance.",
            task_preview,old.version,old.summary,old.acceptance_criteria,
            old.source_requirement_id,status_name,
            observed.0.selected_head.as_deref().unwrap_or("not verified"),
            observed.0.sha256,exact_accepted
        );
        Ok::<_,String>((observed,message))
    }).await.map_err(|_|"PLAN05_REVIEW_PREVIEW_INTERRUPTED")??;
    if !confirm(&app,"Lu — Review exact native evidence",message).await?{
        return Ok(None)
    }
    // A second native inspection after approval must agree semantically with
    // what the human saw. No TOCTOU acceptance of modified source.
    tauri::async_runtime::spawn_blocking(move||
        review_selected(&profile,&root,&register_id,
            Some((&task_id,expected_version)),Some(&observation)).map(Some)
    ).await.map_err(|_|"PLAN05_REVALIDATION_THREAD_INTERRUPTED")?
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn absent_library_is_not_replaced_with_new_database(){
        let dir=std::env::temp_dir().join(format!("plan05-unmounted-{}",std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(library(&dir).is_err());
        assert!(!dir.join("ansf/library.db").exists());
        fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn actual_ansf_append_restart_correction_and_corruption_detection(){
        let profile=std::env::temp_dir().join(format!(
            "lu-plan05-register-{}",std::process::id()));
        let repo=profile.join("project");
        let _=fs::remove_dir_all(&profile);
        fs::create_dir_all(repo.join("docs")).unwrap();
        fs::create_dir_all(profile.join("ansf")).unwrap();
        fs::write(repo.join("docs/requirements.md"),
            "# Requirements\n- [ ] Implement source-backed planning\n").unwrap();
        let db=profile.join("ansf/library.db");
        let conn=Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE preserved_memory(x INTEGER);
                            INSERT INTO preserved_memory VALUES(19);").unwrap();
        drop(conn);
        let (g,s)=classify_selected(&repo).unwrap();
        let cap=synthesis::build_capsule(&g,&s).unwrap();
        let req=&cap.requirements[0];
        let raw=serde_json::json!({"schema":synthesis::RAW_SCHEMA,"tasks":[{
            "requirementId":req.requirement_id,"status":req.status,
            "summary":"Inspect current source evidence",
            "description":"Inspect original source and determine missing acceptance coverage",
            "priority":"P1","group":"01 Source verification",
            "evidenceIds":[req.requirement_id],"dependencyIndices":[],
            "acceptanceCriteria":"Independently verify original current project behavior",
            "blockedBy":"Missing original acceptance receipt"
        }]}).to_string();
        let proposal=synthesis::validate_model_response(&g,&s,&raw,
            &synthesis::model_prompt(&cap).unwrap(),"m-native","s-native","g-native")
            .unwrap();
        project_task_desktop::persist_proposal(&profile,&g,&s,&proposal,&raw).unwrap();
        let imported=register::import(&proposal).unwrap();
        let saved=persist(&profile,&proposal,&imported,None).unwrap();
        drop(saved);
        let conn=library(&profile).unwrap();
        let (_,reopened,version)=load_register(&conn,&imported.register_id)
            .unwrap().unwrap();
        assert_eq!(version,1);
        assert_eq!(reopened.events.len(),1);
        assert_eq!(reopened.latest_sha256,imported.latest_sha256);
        let x:i64=conn.query_row("SELECT x FROM preserved_memory",[],|row|row.get(0)).unwrap();
        assert_eq!(x,19);
        drop(conn);
        let task_id=&proposal.tasks[0].task_id;
        let changed=register::human_correct(&reopened,&proposal,task_id,1,
            &register::HumanCorrection{
                summary:"Correct the task with human evidence".into(),
                description:"Human reviewed original task and specified independent source checks".into(),
                priority:"P0".into(),
                blocked_by:"Awaiting native acceptance".into(),
                acceptance_criteria:"Run current native project acceptance with exact source receipts".into(),
                reason:"Human clarified evidence scope".into(),
            }).unwrap();
        persist(&profile,&proposal,&changed,Some(&reopened.latest_sha256)).unwrap();
        let conn=library(&profile).unwrap();
        let (_,again,version)=load_register(&conn,&changed.register_id).unwrap().unwrap();
        assert_eq!((version,again.events.len()),(2,2));
        assert_eq!(register::latest(&again,task_id).unwrap().version,2);
        assert!(!register::latest(&again,task_id).unwrap().independently_verified_requirement);
        assert_eq!(conn.query_row::<i64,_,_>(
            "SELECT COUNT(*) FROM ansf_project_task_register_revisions",[],|row|row.get(0)
        ).unwrap(),2);
        // A corrupted byte cannot be treated as a verified historical event.
        conn.execute(
            "UPDATE ansf_project_task_register_revisions SET register_json=?1
             WHERE register_id=?2 AND register_revision=1",
            params![b"{corrupted".as_slice(),changed.register_id]
        ).unwrap();
        assert!(load_register(&conn,&changed.register_id).is_err());
        drop(conn);
        fs::remove_dir_all(profile).unwrap();
    }
}
