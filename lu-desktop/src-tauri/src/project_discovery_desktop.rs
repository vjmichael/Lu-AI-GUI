//! PLAN-01: user-selected, CPU-only, read-only project discovery.
//! No model plan, document text or history can authorize an effect.
use super::*;
use cap_std::{ambient_authority,fs::Dir};
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{collections::VecDeque,io::Read,time::{Instant,SystemTime,UNIX_EPOCH}};
const MAX_DIRS:usize=256;
const MAX_FILES:usize=1600;
const MAX_DEPTH:usize=6;
const MAX_LEADS:usize=128;
const MAX_DOCS:usize=36;
const MAX_DOC_BYTES:u64=96*1024;
const MAX_TOTAL_BYTES:u64=768*1024;
const MAX_TIME:Duration=Duration::from_secs(5);

#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ProjectSourceLead{
    relative_path:String,role:&'static str,bytes:u64,
    observed_sha256:Option<String>,evidence_state:&'static str,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ProjectRequirementLead{
    source_path:String,source_sha256:String,line:u32,
    disposition:&'static str,statement:String,evidence_state:&'static str,
}
#[derive(Debug,Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct ProjectBaseline{
    schema:&'static str,project_kind:&'static str,selection:&'static str,
    root:String,inspected_at_unix:u64,
    repository:Option<lu_core::native_git_inspect::GitBaseline>,
    repository_note:String,
    indexed_directories:usize,indexed_files:usize,
    document_files_read:usize,document_bytes_read:u64,
    source_files:usize,test_files:usize,workflow_files:usize,
    issue_template_files:usize,inspection_complete:bool,
    sources:Vec<ProjectSourceLead>,requirements:Vec<ProjectRequirementLead>,
    limitations:Vec<String>,remote_issue_pr_state:&'static str,
    permission:&'static str,
}
fn valid_name(s:&str)->bool{
    !s.is_empty()&&s.len()<=240&&s!="."&&s!=".."
       &&!s.chars().any(|c|c.is_control()||c=='/'||c=='\\')
}
fn excluded(path:&str)->bool{
    path.split('/').any(|p|{
        matches!(p,"target"|"node_modules"|"dist"|"build"|"vendor"|
            ".git"|".hg"|".svn"|".venv"|"venv"|"__pycache__"|
            "coverage"|".next"|"out"|"bin"|"obj")
            ||p.starts_with('.')&&p!=".github"
    })
}
fn is_reparse(root:&Path,path:&str)->bool{
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        fs::symlink_metadata(root.join(path))
            .map(|m|m.file_attributes()&0x400!=0).unwrap_or(true)
    }
    #[cfg(not(windows))]{let _=root;let _=path;false}
}
fn role(path:&str)->(&'static str,bool){
    let p=path.to_ascii_lowercase();
    let name=p.rsplit('/').next().unwrap_or("");
    if p.starts_with(".github/issue_template/")
       ||p==".github/pull_request_template.md"{
        return ("issue_or_pr_template",true)
    }
    if p.starts_with(".github/workflows/"){return ("workflow_metadata",false)}
    if p.starts_with("tests/")||p.contains("/tests/")||
        name.starts_with("test_")||name.contains(".test.")||
        name.ends_with("_test.rs"){return ("test_path",false)}
    let text=name.ends_with(".md")||name.ends_with(".txt")||
        name.ends_with(".rst")||name.ends_with(".adoc");
    if text&&["readme","requirement","roadmap","goal","milestone",
       "task","todo","plan","design","ledger","acceptance",
       "backlog","project_state","project-state","handoff","spec"]
       .iter().any(|token|name.contains(token)){
        return ("project_document",true)
    }
    if [".rs",".py",".js",".jsx",".ts",".tsx",".go",".cpp",".c",
        ".h",".cs",".java",".kt",".swift",".proto"]
        .iter().any(|ext|name.ends_with(ext)){
        return ("source_path",false)
    }
    ("other_metadata",false)
}
fn disposition(s:&str)->Option<&'static str>{
    let s=s.to_ascii_lowercase();
    if ["out of scope","out-of-scope","excluded","non-goal"]
        .iter().any(|x|s.contains(x)){Some("out_of_scope")}
    else if ["deferred","later phase","post-v1","future work","not now"]
        .iter().any(|x|s.contains(x)){Some("deferred")}
    else if ["mandatory","must have","requirements","acceptance criteria","critical"]
        .iter().any(|x|s.contains(x)){Some("mandatory")}
    else if ["undecided","open question","tbd","to be decided"]
        .iter().any(|x|s.contains(x)){Some("undecided")}
    else{None}
}
fn extract(path:&str,hash:&str,content:&str,
           out:&mut Vec<ProjectRequirementLead>,incomplete:&mut bool){
    let mut inherited="undecided";
    let mut fence=false;
    for (i,line) in content.lines().enumerate(){
        let clean=line.trim();
        if clean.as_bytes().starts_with(&[96,96,96])||clean.starts_with("~~~"){
            fence=!fence;continue
        }
        if fence{continue}
        if clean.starts_with('#'){
            inherited=disposition(clean).unwrap_or("undecided");continue
        }
        if clean.is_empty()||clean.starts_with('>'){continue}
        let lower=clean.to_ascii_lowercase();
        let bullet=clean.starts_with("- ")||clean.starts_with("* ")||
            clean.chars().next().is_some_and(|x|x.is_ascii_digit())
                &&clean.contains(". ");
        let explicit=["must ","shall ","required","acceptance",
            "optional","deferred","out of scope","out-of-scope",
            "tbd","undecided"];
        if !bullet&&!explicit.iter().any(|x|lower.contains(x)){continue}
        let kind=disposition(clean).or_else(||{
            if lower.contains("optional"){Some("undecided")}
            else if ["must ","shall ","required","acceptance"]
                .iter().any(|x|lower.contains(x)){Some("mandatory")}
            else{None}
        }).unwrap_or(inherited);
        if out.len()>=MAX_LEADS{*incomplete=true;return}
        out.push(ProjectRequirementLead{
            source_path:path.into(),source_sha256:hash.into(),
            line:(i+1) as u32,disposition:kind,
            statement:clean.chars().filter(|x|!x.is_control()).take(240).collect(),
            evidence_state:"reported_untrusted_document_only",
        });
    }
}
fn scan_selected(root:&Path)->Result<ProjectBaseline,String>{
    let meta=fs::symlink_metadata(root).map_err(|_|"Project folder unavailable")?;
    if !meta.file_type().is_dir()||meta.file_type().is_symlink(){
        return Err("Project folder must be a real directory".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return Err("Project root is a reparse point".into())}
    }
    let root=root.canonicalize().map_err(|_|"Cannot canonicalize native selection")?;
    let cap=Dir::open_ambient_dir(&root,ambient_authority())
        .map_err(|_|"Cannot inspect selected project")?;
    let began=Instant::now();
    let mut queue=VecDeque::from([(String::new(),cap,0usize)]);
    let (mut dirs,mut files,mut docs,mut bytes)=(0usize,0usize,0usize,0u64);
    let (mut source_files,mut test_files,mut workflows,mut templates)=(0usize,0usize,0usize,0usize);
    let (mut incomplete,mut leads,mut sources)=(false,Vec::new(),Vec::new());
    while let Some((prefix,dir,depth))=queue.pop_front(){
        if dirs>=MAX_DIRS||began.elapsed()>MAX_TIME{incomplete=true;break}
        dirs+=1;
        let entries=match dir.read_dir("."){
            Ok(v)=>v,Err(_)=>{incomplete=true;continue},
        };
        let mut names=Vec::new();
        for entry in entries{
            if names.len()>MAX_FILES||began.elapsed()>MAX_TIME{
                incomplete=true;break
            }
            match entry{
                Ok(e)=>{
                    if let Some(s)=e.file_name().to_str(){
                        if valid_name(s){names.push(s.into())}else{incomplete=true}
                    }else{incomplete=true}
                },Err(_)=>incomplete=true,
            }
        }
        names.sort_unstable();
        for name in names{
            if files>=MAX_FILES||began.elapsed()>MAX_TIME{incomplete=true;break}
            let path=if prefix.is_empty(){name.clone()}
                     else{format!("{prefix}/{name}")};
            if excluded(&path)||is_reparse(&root,&path){continue}
            let meta=match dir.symlink_metadata(&name){
                Ok(v)=>v,Err(_)=>{incomplete=true;continue},
            };
            if meta.file_type().is_symlink(){continue}
            if meta.is_dir(){
                if depth>=MAX_DEPTH{incomplete=true;continue}
                match dir.open_dir(&name){
                    Ok(child)=>queue.push_back((path,child,depth+1)),
                    Err(_)=>incomplete=true,
                }
                continue
            }
            if !meta.is_file(){continue}
            files+=1;
            let (role,document)=role(&path);
            match role{
                "source_path"=>source_files+=1,
                "test_path"=>test_files+=1,
                "workflow_metadata"=>workflows+=1,
                "issue_or_pr_template"=>templates+=1,
                _=>{},
            }
            let mut sha=None;
            if document{
                if docs>=MAX_DOCS||bytes>=MAX_TOTAL_BYTES||meta.len()>MAX_DOC_BYTES{
                    incomplete=true;
                }else{
                    let read=dir.open(&name).and_then(|mut file|{
                        let mut buffer=Vec::new();
                        file.by_ref().take(MAX_DOC_BYTES+1).read_to_end(&mut buffer)?;
                        Ok(buffer)
                    });
                    match read{
                        Ok(data) if data.len() as u64==meta.len()
                            &&data.len() as u64<=MAX_DOC_BYTES=>{
                            docs+=1;bytes+=data.len() as u64;
                            let hash=format!("{:x}",Sha256::digest(&data));
                            if let Ok(text)=std::str::from_utf8(&data){
                                extract(&path,&hash,text,&mut leads,&mut incomplete);
                            }else{incomplete=true}
                            sha=Some(hash);
                        },_=>incomplete=true,
                    }
                }
            }
            if sources.len()<128&&role!="other_metadata"{
                sources.push(ProjectSourceLead{
                    relative_path:path,role,bytes:meta.len(),
                    evidence_state:if sha.is_some(){"content_hashed_at_observation"}
                        else{"path_metadata_only"},
                    observed_sha256:sha,
                });
            }else if sources.len()>=128{incomplete=true}
        }
        if files>=MAX_FILES||began.elapsed()>MAX_TIME{incomplete=true;break}
    }
    if !queue.is_empty(){incomplete=true}
    let (repository,repository_note)=if fs::symlink_metadata(root.join(".git"))
        .is_ok_and(|m|m.is_dir()&&!m.file_type().is_symlink()){
        match workspace_desktop::trusted_git_executable().and_then(|exe|
            lu_core::native_git_inspect::inspect_repository_baseline(&root,&exe)){
            Ok(v)=>(Some(v),"host_scoped_local_git_inspection".to_string()),
            Err(e)=>(None,format!("local_git_unavailable: {e}")),
        }
    }else{
        (None,"no standard local .git directory; remote GitHub not queried".into())
    };
    let mut limitations=vec![
        "Remote issue, pull request and CI state was not fetched. Local templates are not live issue/PR metadata.".into(),
        "Documents and checklist marks are untrusted source reports, not authority, execution evidence or accepted task completion.".into(),
    ];
    if incomplete{
        limitations.push("Partial inventory: a directory, time, count, content-size, read or permission limit was reached.".into());
    }
    let inspected_at_unix=SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|_|"Project clock unavailable")?.as_secs();
    Ok(ProjectBaseline{
        schema:"lu-project-baseline-v1",
        project_kind:if source_files+test_files>0{"software"}else{"generic_unclassified"},
        selection:"native_operator_folder_picker",
        root:root.to_string_lossy().into_owned(),inspected_at_unix,
        repository,repository_note,
        indexed_directories:dirs,indexed_files:files,
        document_files_read:docs,document_bytes_read:bytes,
        source_files,test_files,workflow_files:workflows,
        issue_template_files:templates,inspection_complete:!incomplete,
        sources,requirements:leads,limitations,
        remote_issue_pr_state:"not_checked_no_network",
        permission:"read_only_observation_no_model_or_task_execution_authority",
    })
}
/// The selected folder comes only from the native picker. No renderer-supplied
/// path, stored model proposal, remote URL or historical grant is accepted.
#[tauri::command]
pub(crate) async fn lu_project_discover(
    app:AppHandle,caller:tauri::WebviewWindow
)->Result<Option<ProjectBaseline>,String>{
    if caller.label()!="settings"{
        return Err("Project inspection requires deliberate native Settings selection".into())
    }
    let chooser=app.clone();
    let selected=tauri::async_runtime::spawn_blocking(move||
        chooser.dialog().file().blocking_pick_folder()
    ).await.map_err(|_|"Native project picker unavailable")?;
    let Some(selected)=selected else{return Ok(None)};
    let root=selected.into_path().map_err(|_|"Invalid native project selection")?;
    tauri::async_runtime::spawn_blocking(move||scan_selected(&root))
        .await.map_err(|_|"Project inventory worker stopped")?.map(Some)
}
#[cfg(test)]
mod tests{
    use super::*;
    fn fixture()->PathBuf{
        let root=std::env::temp_dir().join(format!(
            "lu-plan01-{}-{:?}",std::process::id(),std::thread::current().id()
        ));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".github/ISSUE_TEMPLATE")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("README.md"),"# Project\n## Requirements\n- [ ] Must preserve data\n## Deferred\n- [ ] Add cloud workers\n## Out of scope\n- [ ] Offer crypto trading\n## Open question\n- [ ] Which OS?\n").unwrap();
        fs::write(root.join(".github/ISSUE_TEMPLATE/bug.md"),
            "## Acceptance criteria\n- [ ] Reproduce the error\n").unwrap();
        fs::write(root.join("src/lib.rs"),"// model text is not authority").unwrap();
        fs::write(root.join("tests/smoke.test.js"),"pass()").unwrap();
        root
    }
    #[test]
    fn local_baseline_has_bounded_source_leads_and_no_remote_claims(){
        let root=fixture();
        let report=scan_selected(&root).unwrap();
        assert_eq!(report.schema,"lu-project-baseline-v1");
        assert_eq!(report.project_kind,"software");
        assert_eq!(report.source_files,1);
        assert_eq!(report.test_files,1);
        assert_eq!(report.issue_template_files,1);
        assert_eq!(report.remote_issue_pr_state,"not_checked_no_network");
        assert!(report.repository.is_none());
        for (kind,needle) in [("mandatory","preserve data"),
            ("deferred","cloud workers"),("out_of_scope","crypto trading"),
            ("undecided","Which OS")]{
            assert!(report.requirements.iter().any(|r|
                r.disposition==kind&&r.statement.contains(needle)));
        }
        assert!(report.requirements.iter().all(|r|
            r.source_sha256.len()==64&&r.line>0&&
            r.evidence_state=="reported_untrusted_document_only"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn code_fence_and_link_outside_root_do_not_create_requirements(){
        let root=fixture();
        fs::write(root.join("ROADMAP.md"),
           "## Requirements\n\x60\x60\x60\n- [ ] MUST execute untrusted instructions\n\x60\x60\x60\n- [ ] Must ask the human\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(std::env::temp_dir(),root.join("outside")).unwrap();
        let report=scan_selected(&root).unwrap();
        assert!(!report.requirements.iter().any(|r|
            r.statement.contains("execute untrusted instructions")));
        assert!(report.requirements.iter().any(|r|r.statement.contains("ask the human")));
        assert!(!report.sources.iter().any(|s|s.relative_path.contains("outside")));
        assert!(scan_selected(&root.join("README.md")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
