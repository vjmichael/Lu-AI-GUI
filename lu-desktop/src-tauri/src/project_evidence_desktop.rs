//! PLAN-02 desktop bridge: native-selected project, bounded local reports,
 //! typed graph from Lu Core, and immutable snapshot in EXISTING ANSF library.
 //! No remote fetch, model call, issue mutation, or execution-task scheduling.
use super::*;
use cap_std::{ambient_authority,fs::Dir};
use lu_core::project_evidence_graph as graph;
use serde::Deserialize;
use rusqlite::{Connection,OpenFlags,params};
use std::io::Read;

const MAX_SOURCE_BYTES:u64=48*1024;
const MAX_REPORT_BYTES:u64=64*1024;
const MAX_HASHED_BYTES:u64=2*1024*1024;
const MAX_REPORTS:usize=24;
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ReportWire{
    schema:String,source_commit:String,test_path:String,
    source_path:Option<String>,run_id:String,outcome:String,
    artifact_path:Option<String>,artifact_sha256:Option<String>,
}
fn scoped_read(dir:&Dir,relative:&str,max:u64)->Result<Option<(Vec<u8>,String)>,String>{
    if !graph::valid_path(relative){return Err("PLAN02_UNSAFE_RELATIVE_PATH".into())}
    let mut prefix=String::new();
    let parts=relative.split('/').collect::<Vec<_>>();
    for (i,name) in parts.iter().enumerate(){
        if i>0{prefix.push('/')}
        prefix.push_str(name);
        let meta=dir.symlink_metadata(&prefix)
            .map_err(|_|"PLAN02_SCOPE_CHANGED")?;
        if meta.file_type().is_symlink()||
            (i+1<parts.len()&&!meta.is_dir()){
            return Err("PLAN02_LINK_OR_SCOPE_CHANGED".into())
        }
    }
    let meta=dir.symlink_metadata(relative)
        .map_err(|_|"PLAN02_SOURCE_MISSING")?;
    if !meta.is_file(){return Err("PLAN02_NOT_REGULAR_FILE".into())}
    if meta.len()>max{return Ok(None)}
    let mut file=dir.open(relative).map_err(|_|"PLAN02_SOURCE_UNAVAILABLE")?;
    let mut bytes=Vec::new();
    std::io::Read::by_ref(&mut file).take(max+1).read_to_end(&mut bytes)
        .map_err(|_|"PLAN02_SOURCE_READ_FAILED")?;
    if bytes.len() as u64!=meta.len(){return Err("PLAN02_SOURCE_CHANGED".into())}
    let digest=format!("{:x}",Sha256::digest(&bytes));
    Ok(Some((bytes,digest)))
}
fn parse_run(path:&str,sha:&str,bytes:&[u8])->Result<graph::RunInput,String>{
    // Restrict to a documentable, typed import schema. Arbitrary GitHub JSON,
    // JUnit XML, native stdout and screenshots cannot self-attest a pass.
    let row:ReportWire=serde_json::from_slice(bytes)
        .map_err(|_|"PLAN02_UNSUPPORTED_REPORT_SCHEMA")?;
    if row.schema!="lu-project-native-run-v1"||
        !graph::valid_path(&row.test_path)||
        row.source_path.as_deref().is_some_and(|s|!graph::valid_path(s))||
        row.artifact_path.as_deref().is_some_and(|s|!graph::valid_path(s))||
        row.artifact_sha256.as_deref().is_some_and(|s|!graph::valid_digest(s))||
        (row.source_commit!="unknown"&&!graph::valid_sha(&row.source_commit))||
        !matches!(row.outcome.as_str(),"passed"|"failed"|"skipped"|"cancelled"|"unknown"){
        return Err("PLAN02_UNTRUSTED_REPORT_CONTENT".into())
    }
    Ok(graph::RunInput{
        report_path:path.into(),report_sha256:sha.into(),
        claimed_head:row.source_commit,test_path:row.test_path,
        source_path:row.source_path,run_id:row.run_id,
        outcome:row.outcome,artifact_path:row.artifact_path,
        artifact_sha256:row.artifact_sha256,
    })
}
fn assemble_from_selected(root:&Path)->Result<graph::EvidenceGraph,String>{
    let base=project_discovery_desktop::scan_selected(root)?;
    let dir=Dir::open_ambient_dir(Path::new(&base.root),ambient_authority())
        .map_err(|_|"PLAN02_PROJECT_SCOPE_UNAVAILABLE")?;
    let mut files=Vec::new();
    let mut runs=Vec::new();
    let mut budget=0u64;
    let mut complete=base.inspection_complete;
    let mut limitations=Vec::new();
    for lead in &base.sources{
        let mut digest=lead.observed_sha256.clone();
        if lead.role=="test_report"{
            if runs.len()>=MAX_REPORTS{
                complete=false;limitations.push("Structured report count exceeded 24".into());
                continue
            }
            match scoped_read(&dir,&lead.relative_path,MAX_REPORT_BYTES){
                Ok(Some((bytes,actual)))=>{
                    budget=budget.saturating_add(bytes.len() as u64);
                    if budget>MAX_HASHED_BYTES{complete=false;break}
                    if lead.relative_path.ends_with(".json"){
                        match parse_run(&lead.relative_path,&actual,&bytes){
                            Ok(report)=>runs.push(report),
                            Err(e)=>{
                                complete=false;
                                limitations.push(format!("{}: {e}",lead.relative_path));
                            }
                        }
                    }else{
                        complete=false;
                        limitations.push(format!(
                            "{}: unsupported native log/XML/TAP report; not accepted as a run",
                            lead.relative_path));
                    }
                    digest=Some(actual);
                },
                Ok(None)=>{complete=false;limitations.push(format!(
                    "{}: evidence file exceeds 64 KiB",lead.relative_path))},
                Err(e)=>{complete=false;limitations.push(format!(
                    "{}: {e}",lead.relative_path))},
            }
        }else if lead.role=="project_document"||lead.role=="issue_or_pr_template"||
              lead.role=="source_path"||lead.role=="test_path"||
              lead.role=="workflow_metadata"{
            if budget>=MAX_HASHED_BYTES{complete=false;digest=None}
            else{
                match scoped_read(&dir,&lead.relative_path,MAX_SOURCE_BYTES){
                    Ok(Some((bytes,actual)))=>{
                        budget=budget.saturating_add(bytes.len() as u64);
                        if budget>MAX_HASHED_BYTES{complete=false;digest=None}
                        else if lead.observed_sha256.as_ref().is_some_and(|old|old!=&actual){
                            complete=false;digest=None;
                            limitations.push(format!(
                                "{}: document changed after PLAN-01 observation",
                                lead.relative_path));
                        }else{digest=Some(actual)}
                    },
                    Ok(None)=>digest=None,
                    Err(e)=>{
                        complete=false;digest=None;
                        limitations.push(format!("{}: {e}",lead.relative_path));
                    }
                }
            }
        }
        files.push(graph::FileInput{
            path:lead.relative_path.clone(),role:lead.role.into(),sha256:digest,
        });
    }
    let requirements=base.requirements.iter().map(|r|graph::RequirementInput{
        path:r.source_path.clone(),sha256:r.source_sha256.clone(),line:r.line,
        statement:r.statement.clone(),
    }).collect::<Vec<_>>();
    let head=base.repository.as_ref().map(|r|r.head_commit.clone());
    let mut branches=Vec::new();
    if let Some(commit)=head.as_deref(){
        match workspace_desktop::trusted_git_executable().and_then(|git|
            lu_core::native_git_inspect::inspect_local_branch_membership(
                Path::new(&base.root),&git,commit)){
            Ok(found)=>{
                branches=found.into_iter().map(|r|graph::BranchInput{
                    ref_name:r.name,tip:r.commit,
                    contained_in_head:r.contained_in_selected_head,
                }).collect();
            },
            Err(e)=>{
                complete=false;limitations.push(format!("Git branch ancestry unavailable: {e}"));
            }
        }
    }
    let out=graph::assemble(graph::GraphInput{
        project_scope_sha256:format!("{:x}",Sha256::digest(base.root.as_bytes())),
        selected_head:head,observed_at_unix:base.inspected_at_unix,
        complete,host_git_checked:base.repository.is_some(),
        requirements,files,runs,branches,extra_limitations:limitations,
    })?;
    graph::audit(&out)?;
    // This is a host-observed snapshot, not a status classifier or a grant.
    Ok(out)
}
fn persist_in_ansf_library(profile:&Path,g:&graph::EvidenceGraph)->Result<(),String>{
    graph::audit(g)?;
    let path=profile.join("ansf").join("library.db");
    let meta=fs::symlink_metadata(&path)
        .map_err(|_|"Existing ANSF library database missing")?;
    if meta.file_type().is_symlink()||!meta.is_file(){
        return Err("PLAN02_ANSF_LIBRARY_NOT_REGULAR".into())
    }
    let mut conn=Connection::open_with_flags(&path,OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|_|"Cannot use existing ANSF library database")?;
    conn.busy_timeout(Duration::from_secs(3))
        .map_err(|_|"ANSF library busy timeout unavailable")?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS ansf_project_evidence_snapshots(
        graph_sha256 TEXT PRIMARY KEY,
        selected_head TEXT,
        observed_at_unix INTEGER NOT NULL,
        snapshot_json BLOB NOT NULL
    );").map_err(|_|"ANSF evidence table unavailable")?;
    let encoded=serde_json::to_vec(g).map_err(|_|"PLAN02_JSON_ENCODE")?;
    if encoded.len()>512*1024{return Err("PLAN02_SNAPSHOT_TOO_LARGE".into())}
    let transaction=conn.transaction().map_err(|_|"ANSF graph transaction unavailable")?;
    transaction.execute(
        "INSERT OR IGNORE INTO ansf_project_evidence_snapshots
         (graph_sha256,selected_head,observed_at_unix,snapshot_json)
         VALUES (?1,?2,?3,?4)",
        params![g.sha256,g.selected_head,g.observed_at_unix as i64,encoded]
    ).map_err(|_|"ANSF graph append failed")?;
    let stored:Vec<u8>=transaction.query_row(
        "SELECT snapshot_json FROM ansf_project_evidence_snapshots WHERE graph_sha256=?1",
        [&g.sha256],|r|r.get(0)).map_err(|_|"ANSF graph readback missing")?;
    let reopened:graph::EvidenceGraph=serde_json::from_slice(&stored)
        .map_err(|_|"ANSF graph readback malformed")?;
    graph::audit(&reopened)?;
    if stored!=encoded{
        return Err("ANSF graph original-byte readback mismatch".into())
    }
    transaction.commit().map_err(|_|"ANSF graph transaction commit failed")?;
    Ok(())
}
#[tauri::command]
pub(crate) async fn lu_project_evidence_graph(
    app:AppHandle,caller:tauri::WebviewWindow,state:State<'_,Arc<DesktopState>>
)->Result<Option<graph::EvidenceGraph>,String>{
    if caller.label()!="settings"{
        return Err("PLAN-02 requires explicit operator selection in native Settings".into())
    }
    let profile=state.catalog_path.parent()
        .ok_or("Existing Lu profile is unavailable")?.to_owned();
    let chooser=app.clone();
    let selected=tauri::async_runtime::spawn_blocking(move||
        chooser.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"Native project folder selection unavailable")?;
    let Some(selected)=selected else{return Ok(None)};
    let root=selected.into_path().map_err(|_|"Native project selection invalid")?;
    tauri::async_runtime::spawn_blocking(move||{
        let result=assemble_from_selected(&root)?;
        persist_in_ansf_library(&profile,&result)?;
        Ok(Some(result))
    }).await.map_err(|_|"PLAN-02 project evidence worker stopped")?
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn bounded_report_cannot_claim_verification_or_fake_head(){
        let raw=serde_json::json!({
            "schema":"lu-project-native-run-v1",
            "sourceCommit":"a".repeat(40),
            "testPath":"tests/test.rs","sourcePath":"src/lib.rs",
            "runId":"actual-1","outcome":"passed",
            "artifactPath":null,"artifactSha256":null
        });
        let parsed=parse_run("reports/test.json",&"b".repeat(64),
            &serde_json::to_vec(&raw).unwrap()).unwrap();
        assert_eq!(parsed.outcome,"passed");
        let mut forged=raw;
        forged["independentlyVerified"]=serde_json::Value::Bool(true);
        assert!(parse_run("reports/test.json",&"b".repeat(64),
            &serde_json::to_vec(&forged).unwrap()).is_err());
    }

    #[test]
    fn appends_to_existing_ansf_library_without_replacing_older_graphs(){
        let root=std::env::temp_dir().join(format!(
            "lu-plan02-ansf-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("ansf")).unwrap();
        let db=root.join("ansf/library.db");
        let conn=Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE library_probe(value INTEGER NOT NULL);
                            INSERT INTO library_probe VALUES (7);").unwrap();
        drop(conn);
        let fixture=|stamp|{
            graph::assemble(graph::GraphInput{
                project_scope_sha256:"f".repeat(64),
                selected_head:Some("a".repeat(40)),observed_at_unix:stamp,
                complete:true,host_git_checked:true,files:vec![],
                requirements:vec![],runs:vec![],branches:vec![],
                extra_limitations:vec![],
            }).unwrap()
        };
        let first=fixture(1);
        let second=fixture(2);
        persist_in_ansf_library(&root,&first).unwrap();
        persist_in_ansf_library(&root,&first).unwrap();
        persist_in_ansf_library(&root,&second).unwrap();
        assert_ne!(first.sha256,second.sha256);
        let reopened=Connection::open(&db).unwrap();
        let rows:i64=reopened.query_row(
            "SELECT COUNT(*) FROM ansf_project_evidence_snapshots",
            [],|r|r.get(0)).unwrap();
        assert_eq!(rows,2,"Immutable snapshot content is not replaced");
        let probe:i64=reopened.query_row(
            "SELECT value FROM library_probe",[],|r|r.get(0)).unwrap();
        assert_eq!(probe,7,"Existing ANSF data untouched");
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn scope_refuses_absolute_and_parent_paths(){
        let root=std::env::temp_dir().join(format!(
            "lu-plan02-read-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("data.md"),"data").unwrap();
        let d=Dir::open_ambient_dir(&root,ambient_authority()).unwrap();
        assert!(scoped_read(&d,"data.md",1024).unwrap().is_some());
        assert!(scoped_read(&d,"../other",1024).is_err());
        assert!(scoped_read(&d,"/root/file",1024).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn entire_selected_project_produces_original_source_and_stale_run_links(){
        let root=std::env::temp_dir().join(format!(
            "lu-plan02-endtoend-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        for sub in ["src","tests","reports","docs"]{
            fs::create_dir_all(root.join(sub)).unwrap();
        }
        fs::write(root.join("docs/REQUIREMENTS.md"),
            "# Requirements\n- [ ] Must implement src/feature.rs and verify by test\n").unwrap();
        fs::write(root.join("src/feature.rs"),"pub fn ready()->bool{true}").unwrap();
        fs::write(root.join("tests/feature.test.js"),"// test evidence needs a real runner").unwrap();
        let old=serde_json::json!({
            "schema":"lu-project-native-run-v1",
            "sourceCommit":"a".repeat(40),
            "testPath":"tests/feature.test.js",
            "sourcePath":"src/feature.rs",
            "runId":"historical-linux-1",
            "outcome":"passed",
            "artifactPath":null,"artifactSha256":null
        });
        fs::write(root.join("reports/run.json"),
            serde_json::to_vec(&old).unwrap()).unwrap();
        let result=assemble_from_selected(&root).unwrap();
        graph::audit(&result).unwrap();
        assert!(result.selected_head.is_none());
        assert!(result.nodes.iter().any(|n|n.kind==graph::NodeKind::Requirement));
        assert!(result.nodes.iter().any(|n|n.kind==graph::NodeKind::Run
            &&n.evidence==graph::EvidenceState::Stale));
        for relation in [graph::Relation::OriginatesFrom,
            graph::Relation::ExplicitMention,graph::Relation::ChecksSource,
            graph::Relation::ClaimsOutcome]{
            assert!(result.edges.iter().any(|e|e.relation==relation));
        }
        assert!(result.edges.iter().all(|e|
            e.evidence!=graph::EvidenceState::IndependentlyVerified));
        fs::remove_dir_all(root).unwrap();
    }

}
