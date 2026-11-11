//! PLAN-03: one native selection, read-only scoped facts, immutable ANSF status.
use super::*;
use lu_core::{project_evidence_graph as graph,project_status_classifier as classifier};
use rusqlite::{Connection,OpenFlags,params};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ClassifiedProject{
    schema:&'static str,
    graph_sha256:String,project_scope_sha256:String,
    status:classifier::StatusReport,
}
fn classify_selected(root:&Path)->Result<(graph::EvidenceGraph,classifier::StatusReport),String>{
    // PLAN-02 owns the exact native project scan and report provenance.
    let evidence=project_evidence_desktop::assemble_from_selected(root)?;
    let facts=classifier::observe_project_paths(root,&evidence)?;
    // Re-check original local HEAD after the separate evidence/path passes.
    // Never reuse a project verdict if a Git checkout changed underneath.
    if let Some(original)=evidence.selected_head.as_deref(){
        let git=workspace_desktop::trusted_git_executable()?;
        let fresh=lu_core::native_git_inspect::inspect_repository_baseline(root,&git)?;
        if fresh.head_commit!=original{
            return Err("PLAN03_HEAD_CHANGED_DURING_INSPECTION".into())
        }
    }
    let report=classifier::classify(&evidence,&facts)?;
    classifier::audit(&report,&evidence)?;
    Ok((evidence,report))
}
pub(crate) fn store_status(profile:&Path,g:&graph::EvidenceGraph,
    r:&classifier::StatusReport)->Result<(),String>{
    graph::audit(g)?;
    classifier::audit(r,g)?;
    // Preserve canonical graph snapshots in existing ANSF before linking any
    // classification. No independent database, no task or model state.
    project_evidence_desktop::persist_in_ansf_library(profile,g)?;
    let db=profile.join("ansf/library.db");
    let meta=fs::symlink_metadata(&db)
        .map_err(|_|"PLAN03_ANSF_LIBRARY_MISSING")?;
    if !meta.is_file()||meta.file_type().is_symlink(){
        return Err("PLAN03_ANSF_LIBRARY_UNSAFE".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err("PLAN03_ANSF_REPARSE_REFUSED".into())
        }
    }
    let mut conn=Connection::open_with_flags(&db,OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_|"PLAN03_ANSF_OPEN_FAILED")?;
    conn.busy_timeout(Duration::from_secs(3))
        .map_err(|_|"PLAN03_ANSF_LOCK_FAILED")?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS ansf_project_status_snapshots(
        classification_sha256 TEXT PRIMARY KEY,
        graph_sha256 TEXT NOT NULL,
        scope_sha256 TEXT NOT NULL,
        exact_head TEXT,
        created_unix INTEGER NOT NULL,
        snapshot_json BLOB NOT NULL
    );").map_err(|_|"PLAN03_ANSF_SCHEMA_FAILED")?;
    let bytes=serde_json::to_vec(r).map_err(|_|"PLAN03_STATUS_ENCODE")?;
    if bytes.len()>512*1024{return Err("PLAN03_STATUS_TOO_LARGE".into())}
    let tx=conn.transaction().map_err(|_|"PLAN03_TRANSACTION_FAILED")?;
    tx.execute(
        "INSERT OR IGNORE INTO ansf_project_status_snapshots
         (classification_sha256,graph_sha256,scope_sha256,exact_head,
          created_unix,snapshot_json) VALUES (?1,?2,?3,?4,?5,?6)",
        params![r.digest_sha256,r.graph_sha256,r.project_scope_sha256,
            r.selected_head,r.observed_at_unix as i64,bytes]
    ).map_err(|_|"PLAN03_APPEND_FAILED")?;
    let readback:Vec<u8>=tx.query_row(
        "SELECT snapshot_json FROM ansf_project_status_snapshots
         WHERE classification_sha256=?1",
        [&r.digest_sha256],|row|row.get(0)
    ).map_err(|_|"PLAN03_READBACK_MISSING")?;
    let reopened:classifier::StatusReport=serde_json::from_slice(&readback)
        .map_err(|_|"PLAN03_READBACK_INVALID")?;
    classifier::audit(&reopened,g)?;
    if readback!=bytes{return Err("PLAN03_IMMUTABLE_SNAPSHOT_CONFLICT".into())}
    tx.commit().map_err(|_|"PLAN03_COMMIT_FAILED")?;
    Ok(())
}
#[tauri::command]
pub(crate) async fn lu_project_classify(
    app:AppHandle,caller:tauri::WebviewWindow,state:State<'_,Arc<DesktopState>>
)->Result<Option<ClassifiedProject>,String>{
    if caller.label()!="settings"{
        return Err("PLAN03_REQUIRES_NATIVE_SETTINGS_OPERATOR_SELECTION".into())
    }
    let profile=state.catalog_path.parent()
        .ok_or("PLAN03_LU_PROFILE_UNAVAILABLE")?.to_owned();
    let owner=app.clone();
    let selected=tauri::async_runtime::spawn_blocking(move||
        owner.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"PLAN03_NATIVE_PICKER_UNAVAILABLE")?;
    let Some(selected)=selected else{return Ok(None)};
    let root=selected.into_path().map_err(|_|"PLAN03_NATIVE_SELECTION_INVALID")?;
    tauri::async_runtime::spawn_blocking(move||{
        let (g,r)=classify_selected(&root)?;
        store_status(&profile,&g,&r)?;
        Ok(Some(ClassifiedProject{
            schema:classifier::SCHEMA,
            graph_sha256:g.sha256.clone(),
            project_scope_sha256:g.project_scope_sha256.clone(),status:r,
        }))
    }).await.map_err(|_|"PLAN03_NATIVE_CLASSIFIER_INTERRUPTED")?
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn selected_folder_absence_is_narrow_and_existing_file_unverified(){
        let root=std::env::temp_dir().join(format!(
            "lu-plan03-native-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/exists.rs"),"fn present(){}").unwrap();
        fs::write(root.join("docs/REQUIREMENTS.md"),
            "# Requirements\n- Must implement src/exists.rs\n- Must implement src/missing.rs\n").unwrap();
        let (g,r)=classify_selected(&root).unwrap();
        classifier::audit(&r,&g).unwrap();
        assert!(r.findings.iter().any(|f|f.requirement.contains("exists.rs")&&
            f.primary==classifier::Status::Unverified));
        assert!(r.findings.iter().any(|f|f.requirement.contains("missing.rs")&&
            f.primary==classifier::Status::Missing));
        assert!(!r.findings.iter().any(|f|f.primary==classifier::Status::Accepted));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn original_native_json_syntax_failure_and_repair_change_only_exact_gate(){
        let root=std::env::temp_dir().join(format!(
            "lu-plan03-json-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("docs/REQUIREMENTS.md"),
            "# Requirements\n- Must validate JSON syntax of config/settings.json\n").unwrap();
        fs::write(root.join("config/settings.json"),b"{invalid").unwrap();
        let (graph,broken)=classify_selected(&root).unwrap();
        classifier::audit(&broken,&graph).unwrap();
        assert_eq!(broken.findings[0].primary,classifier::Status::Broken);
        fs::write(root.join("config/settings.json"),b"{\"valid\":true}").unwrap();
        let (graph,accepted)=classify_selected(&root).unwrap();
        classifier::audit(&accepted,&graph).unwrap();
        assert_eq!(accepted.findings[0].primary,classifier::Status::Accepted);
        assert!(accepted.findings[0].explanation.contains("JSON parser"));
        assert_ne!(broken.digest_sha256,accepted.digest_sha256,
            "Changing original file content must not silently reuse an old verdict");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn existing_ansf_status_is_append_only_across_restarts(){
        let profile=std::env::temp_dir().join(format!(
            "lu-plan03-ansf-{}",std::process::id()));
        let root=profile.join("project");
        let _=fs::remove_dir_all(&profile);
        fs::create_dir_all(root.join("docs")).unwrap();
        fs::create_dir_all(profile.join("ansf")).unwrap();
        let db=profile.join("ansf/library.db");
        let conn=Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE untouched_probe(v INTEGER);
                            INSERT INTO untouched_probe VALUES (7);").unwrap();
        drop(conn);
        fs::write(root.join("docs/REQUIREMENTS.md"),
            "# Requirements\n- Must implement src/expected.rs\n").unwrap();
        let (g,r)=classify_selected(&root).unwrap();
        store_status(&profile,&g,&r).unwrap();
        store_status(&profile,&g,&r).unwrap();
        let check=Connection::open(&db).unwrap();
        let count:i64=check.query_row(
            "SELECT COUNT(*) FROM ansf_project_status_snapshots",
            [],|row|row.get(0)).unwrap();
        let probe:i64=check.query_row("SELECT v FROM untouched_probe",
            [],|row|row.get(0)).unwrap();
        assert_eq!((count,probe),(1,7));
        drop(check);
        fs::remove_dir_all(profile).unwrap();
    }
}
