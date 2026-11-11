//! PLAN-02: bounded, typed, host-assembled evidence, never execution authority.
//! Project files and CI reports are data, not instructions or verified claims.
use serde::{Serialize,Deserialize};
use sha2::{Sha256,Digest};
use std::collections::{BTreeMap,BTreeSet};

pub const SCHEMA:&str="lu-project-evidence-v1";
const MAX_NODES:usize=448;
const MAX_EDGES:usize=768;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum EvidenceState{Reported,Inspected,IndependentlyVerified,Stale,Unknown}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum NodeKind{Requirement,Document,Component,IntegrationPath,Test,Run,Acceptance,Artifact,Snapshot}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Relation{OriginatesFrom,ExplicitMention,ChecksSource,ReportedBy,ClaimsOutcome,ClaimsArtifact,ContainedInHead}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct EvidenceNode{
    pub id:String,pub kind:NodeKind,pub label:String,pub locator:String,
    pub digest:Option<String>,pub commit:Option<String>,pub line:Option<u32>,
    pub evidence:EvidenceState,pub reason:String,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct EvidenceEdge{
    pub from:String,pub to:String,pub relation:Relation,
    pub evidence:EvidenceState,pub reason:String,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct EvidenceGraph{
    pub schema:String,pub project_scope_sha256:String,
    pub selected_head:Option<String>,pub observed_at_unix:u64,
    pub complete:bool,pub nodes:Vec<EvidenceNode>,pub edges:Vec<EvidenceEdge>,
    pub limitations:Vec<String>,pub no_effect_authority:bool,pub sha256:String,
}
#[derive(Clone,Debug)]
pub struct FileInput{pub path:String,pub role:String,pub sha256:Option<String>}
#[derive(Clone,Debug)]
pub struct RequirementInput{
    pub path:String,pub sha256:String,pub line:u32,pub statement:String,
}
#[derive(Clone,Debug)]
pub struct RunInput{
    pub report_path:String,pub report_sha256:String,pub claimed_head:String,
    pub test_path:String,pub source_path:Option<String>,pub run_id:String,
    pub outcome:String,pub artifact_path:Option<String>,pub artifact_sha256:Option<String>,
}
#[derive(Clone,Debug)]
pub struct BranchInput{
    pub ref_name:String,pub tip:String,
    /// Set by the trusted host's native Git merge-base check, never a report.
    pub contained_in_head:bool,
}
pub struct GraphInput{
    pub project_scope_sha256:String,
    pub selected_head:Option<String>,pub observed_at_unix:u64,
    pub complete:bool,pub host_git_checked:bool,
    pub files:Vec<FileInput>,pub requirements:Vec<RequirementInput>,
    pub runs:Vec<RunInput>,pub branches:Vec<BranchInput>,
    pub extra_limitations:Vec<String>,
}
pub fn valid_sha(s:&str)->bool{
    (s.len()==40||s.len()==64)&&s.bytes().all(|c|c.is_ascii_hexdigit())
}
pub fn valid_digest(s:&str)->bool{s.len()==64&&s.bytes().all(|c|c.is_ascii_hexdigit())}
pub fn valid_path(s:&str)->bool{
    !s.is_empty()&&s.len()<=3200&&!s.starts_with('/')&&
    !s.chars().any(|c|c.is_control()||c==':'||c=='\\')&&
    s.split('/').all(|p|!p.is_empty()&&p!="."&&p!=".."&&p.len()<=240)
}
fn hash(parts:&[&str])->String{
    let mut h=Sha256::new();
    for p in parts{h.update((p.len() as u64).to_be_bytes());h.update(p.as_bytes())}
    format!("{:x}",h.finalize())
}
fn node(map:&mut BTreeMap<String,EvidenceNode>,kind:NodeKind,
    locator:&str,version:&str,label:&str,digest:Option<String>,
    commit:Option<String>,line:Option<u32>,evidence:EvidenceState,reason:&str
)->Result<String,String>{
    if map.len()>=MAX_NODES{return Err("PLAN02_NODES_OVERFLOW".into())}
    let id=hash(&[&format!("{kind:?}"),locator,version]);
    map.entry(id.clone()).or_insert_with(||EvidenceNode{
        id:id.clone(),kind,locator:locator.into(),
        label:label.chars().take(300).collect(),digest,commit,line,evidence,
        reason:reason.into(),
    });
    Ok(id)
}
fn link(out:&mut Vec<EvidenceEdge>,nodes:&BTreeMap<String,EvidenceNode>,
    from:&str,to:&str,relation:Relation,evidence:EvidenceState,reason:&str
)->Result<(),String>{
    if out.len()>=MAX_EDGES{return Err("PLAN02_EDGES_OVERFLOW".into())}
    if !nodes.contains_key(from)||!nodes.contains_key(to){
        return Err("PLAN02_DANGLING_LINK".into())
    }
    if evidence==EvidenceState::IndependentlyVerified&&relation!=Relation::ContainedInHead{
        return Err("PLAN02_FALSE_VERIFICATION".into())
    }
    if !out.iter().any(|e|e.from==from&&e.to==to&&e.relation==relation){
        out.push(EvidenceEdge{from:from.into(),to:to.into(),relation,evidence,reason:reason.into()})
    }
    Ok(())
}
fn explicit_path(statement:&str,path:&str)->bool{
    for (at,_) in statement.match_indices(path){
        let before=statement[..at].chars().last();
        let after=statement[at+path.len()..].chars().next();
        let delim=|ch:Option<char>|ch.is_none_or(|c|c.is_whitespace()||
            c=='"'||c==39 as char||c==96 as char||"()[],;".contains(c));
        if delim(before)&&delim(after){return true}
    }
    false
}
pub fn assemble(input:GraphInput)->Result<EvidenceGraph,String>{
    if !valid_digest(&input.project_scope_sha256)||
        input.files.len()>160||input.requirements.len()>128||
        input.runs.len()>24||input.branches.len()>12||
        input.selected_head.as_deref().is_some_and(|s|!valid_sha(s)){
        return Err("PLAN02_INPUT_INVALID".into())
    }
    let mut nodes=BTreeMap::new();
    let mut edges=Vec::new();
    let mut limitations=input.extra_limitations.clone();
    let mut paths=BTreeMap::<String,(String,Option<String>)>::new();
    let head=input.selected_head.clone();
    for f in &input.files{
        if !valid_path(&f.path)||f.sha256.as_deref().is_some_and(|s|!valid_digest(s))
            ||paths.contains_key(&f.path){return Err("PLAN02_FILE_INVALID".into())}
        let kind=match f.role.as_str(){
            "project_document"|"issue_or_pr_template"=>NodeKind::Document,
            "source_path"|"workflow_metadata"=>NodeKind::Component,
            "test_path"=>NodeKind::Test,
            "test_report"=>NodeKind::Artifact,
            _=>continue,
        };
        let id=node(&mut nodes,kind,&f.path,
            f.sha256.as_deref().unwrap_or("unhashed"),&f.path,
            f.sha256.clone(),head.clone(),None,
            if f.sha256.is_some(){EvidenceState::Inspected}else{EvidenceState::Unknown},
            "Path and optional local bytes observed, not behavior or acceptance.")?;
        paths.insert(f.path.clone(),(id,f.sha256.clone()));
    }
    for r in &input.requirements{
        if !valid_path(&r.path)||!valid_digest(&r.sha256)||r.line==0||
            r.statement.is_empty()||r.statement.len()>2048{
            return Err("PLAN02_REQUIREMENT_INVALID".into())
        }
        let rid=node(&mut nodes,NodeKind::Requirement,
            &format!("{}:{}",r.path,r.line),&r.sha256,&r.statement,
            Some(r.sha256.clone()),head.clone(),Some(r.line),
            EvidenceState::Reported,"Untrusted project requirement, not an execution grant.")?;
        if let Some((target,sha))=paths.get(&r.path){
            if sha.as_deref()==Some(r.sha256.as_str()){
                link(&mut edges,&nodes,&rid,target,Relation::OriginatesFrom,
                    EvidenceState::Inspected,"Original document hash agrees.")?;
            }else{limitations.push(format!("Requirement digest changed: {}",r.path))}
        }else{limitations.push(format!("Requirement document not enumerated: {}",r.path))}
        for (path,(target,_)) in &paths{
            if path!=&r.path&&explicit_path(&r.statement,path){
                link(&mut edges,&nodes,&rid,target,Relation::ExplicitMention,
                    EvidenceState::Reported,"Exact path cited by document; no integration proven.")?;
            }
        }
    }
    if let Some(ref selected)=head{
        let sid=node(&mut nodes,NodeKind::Snapshot,"selected-local-head",selected,selected,
            None,Some(selected.clone()),None,
            if input.host_git_checked{EvidenceState::Inspected}else{EvidenceState::Unknown},
            "Local HEAD identity, not remote CI or final acceptance.")?;
        for branch in &input.branches{
            if branch.ref_name.is_empty()||branch.ref_name.len()>240||
                branch.ref_name.split('/').any(|s|s.is_empty()||s=="."||s=="..")||
                branch.ref_name.bytes().any(|c|c<32||c==127||c==b':'||c==b'\\')||
                !valid_sha(&branch.tip){return Err("PLAN02_BRANCH_INVALID".into())}
            let bid=node(&mut nodes,NodeKind::IntegrationPath,
                &format!("refs/heads/{}",branch.ref_name),&branch.tip,
                &branch.ref_name,None,Some(branch.tip.clone()),None,
                if input.host_git_checked{EvidenceState::Inspected}else{EvidenceState::Unknown},
                "Actual local branch ref; only ancestry is independently checked.")?;
            if input.host_git_checked&&branch.contained_in_head{
                link(&mut edges,&nodes,&bid,&sid,Relation::ContainedInHead,
                    EvidenceState::IndependentlyVerified,
                    "Native host Git observed branch tip in the ancestry of selected HEAD.")?;
            }
        }
    }else{limitations.push("No independently inspected local Git HEAD.".into())}
    for r in &input.runs{
        if !valid_path(&r.report_path)||!valid_digest(&r.report_sha256)||
            !valid_path(&r.test_path)||r.source_path.as_deref().is_some_and(|s|!valid_path(s))||
            r.run_id.is_empty()||r.run_id.len()>100||
            (r.claimed_head!="unknown"&&!valid_sha(&r.claimed_head))||
            !matches!(r.outcome.as_str(),"passed"|"failed"|"skipped"|"cancelled"|"unknown")||
            r.artifact_path.as_deref().is_some_and(|s|!valid_path(s))||
            r.artifact_sha256.as_deref().is_some_and(|s|!valid_digest(s)){
            return Err("PLAN02_REPORT_INVALID".into())
        }
        let stale=head.as_deref()!=Some(r.claimed_head.as_str());
        let state=if stale{EvidenceState::Stale}else{EvidenceState::Reported};
        let report=node(&mut nodes,NodeKind::Artifact,&r.report_path,&r.report_sha256,
            &r.report_path,Some(r.report_sha256.clone()),None,None,EvidenceState::Inspected,
            "Host hashed local report bytes; its contents remain untrusted.")?;
        let test=if let Some((id,_))=paths.get(&r.test_path){id.clone()}else{
            node(&mut nodes,NodeKind::Test,&r.test_path,"unobserved",&r.test_path,
                None,None,None,EvidenceState::Unknown,
                "Report names a test not independently inspected.")?
        };
        let run=node(&mut nodes,NodeKind::Run,
            &format!("{}:{}",r.report_path,r.run_id),&r.claimed_head,&r.run_id,
            None,None,None,state,"Reported run, not Lu's independent test observation.")?;
        let acceptance=node(&mut nodes,NodeKind::Acceptance,
            &format!("{}:{}:acceptance",r.report_path,r.run_id),&r.claimed_head,
            &r.outcome,None,None,None,
            if stale{EvidenceState::Stale}else{EvidenceState::Unknown},
            "Reported outcome does not prove native, installed, or human acceptance.")?;
        link(&mut edges,&nodes,&test,&run,Relation::ReportedBy,state,
            "Test participation is a report claim.")?;
        link(&mut edges,&nodes,&run,&report,Relation::ReportedBy,
            EvidenceState::Reported,"Run record linked to inspected bytes.")?;
        link(&mut edges,&nodes,&run,&acceptance,Relation::ClaimsOutcome,state,
            "Historical or claimed pass never automatically verifies acceptance.")?;
        if let Some(path)=r.source_path.as_deref(){
            if let Some((source,_))=paths.get(path){
                link(&mut edges,&nodes,&test,source,Relation::ChecksSource,
                    EvidenceState::Reported,"Test-source link was reported, not independently run.")?;
            }
        }
        if let Some(path)=r.artifact_path.as_deref(){
            let artifact=if let Some((id,_))=paths.get(path){id.clone()}else{
                node(&mut nodes,NodeKind::Artifact,path,
                    r.artifact_sha256.as_deref().unwrap_or("unknown"),path,
                    None,None,None,EvidenceState::Unknown,
                    "Artifact path claimed but bytes not independently inspected.")?
            };
            link(&mut edges,&nodes,&acceptance,&artifact,Relation::ClaimsArtifact,
                EvidenceState::Reported,"Artifact association is a report claim.")?;
        }
    }
    if input.runs.is_empty(){
        limitations.push("No scoped structured run reports; no test outcomes established.".into());
    }
    if !input.complete{
        limitations.push("Partial inventory; absent paths do not prove missing features.".into());
    }
    limitations.push("Remote GitHub/CI, independent test outcomes and human acceptance not verified.".into());
    limitations.push("Graph has no tool grants, task classification or GGUF synthesis.".into());
    let nodes=nodes.into_values().collect::<Vec<_>>();
    edges.sort_by(|a,b|a.from.cmp(&b.from).then(a.to.cmp(&b.to))
        .then(format!("{:?}",a.relation).cmp(&format!("{:?}",b.relation))));
    let bytes=serde_json::to_vec(&(&nodes,&edges,&input.project_scope_sha256,&head,input.observed_at_unix,
        input.complete,&limitations)).map_err(|_|"PLAN02_SERIALIZE".to_string())?;
    let sha256=format!("{:x}",Sha256::digest(&bytes));
    Ok(EvidenceGraph{
        schema:SCHEMA.into(),project_scope_sha256:input.project_scope_sha256,
        selected_head:head,observed_at_unix:input.observed_at_unix,
        complete:input.complete,nodes,edges,limitations,
        no_effect_authority:true,sha256,
    })
}
pub fn audit(g:&EvidenceGraph)->Result<(),String>{
    if g.schema!=SCHEMA||!valid_digest(&g.project_scope_sha256)||!g.no_effect_authority||
        !valid_digest(&g.sha256)||g.nodes.len()>MAX_NODES||g.edges.len()>MAX_EDGES{
        return Err("PLAN02_SNAPSHOT_INVALID".into())
    }
    let ids=g.nodes.iter().map(|n|n.id.as_str()).collect::<BTreeSet<_>>();
    if ids.len()!=g.nodes.len(){return Err("PLAN02_NODE_DUPLICATE".into())}
    if g.edges.iter().any(|e|!ids.contains(e.from.as_str())||
        !ids.contains(e.to.as_str())||
        (e.evidence==EvidenceState::IndependentlyVerified&&
            e.relation!=Relation::ContainedInHead)){
        return Err("PLAN02_EDGE_UNTRUSTED_VERIFIER".into())
    }
    let bytes=serde_json::to_vec(&(&g.nodes,&g.edges,&g.project_scope_sha256,&g.selected_head,
        g.observed_at_unix,g.complete,&g.limitations))
        .map_err(|_|"PLAN02_SERIALIZE".to_string())?;
    if format!("{:x}",Sha256::digest(&bytes))!=g.sha256{
        return Err("PLAN02_SNAPSHOT_CHANGED".into())
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    fn sample(head:Option<String>,report_sha:String)->GraphInput{
        GraphInput{
            project_scope_sha256:"f".repeat(64),
            selected_head:head,host_git_checked:true,observed_at_unix:8,complete:true,
            files:vec![
                FileInput{path:"docs/REQUIREMENTS.md".into(),role:"project_document".into(),sha256:Some("b".repeat(64))},
                FileInput{path:"src/feature.rs".into(),role:"source_path".into(),sha256:Some("c".repeat(64))},
                FileInput{path:"tests/feature.test.js".into(),role:"test_path".into(),sha256:Some("d".repeat(64))},
            ],
            requirements:vec![RequirementInput{path:"docs/REQUIREMENTS.md".into(),
                sha256:"b".repeat(64),line:3,statement:"Must implement src/feature.rs".into()}],
            runs:vec![RunInput{report_path:"test-results/check.json".into(),
                report_sha256:"e".repeat(64),claimed_head:report_sha,
                test_path:"tests/feature.test.js".into(),
                source_path:Some("src/feature.rs".into()),run_id:"linux-17".into(),
                outcome:"passed".into(),artifact_path:Some("artifacts/run.zip".into()),
                artifact_sha256:None}],
            extra_limitations:Vec::new(),
            branches:vec![BranchInput{ref_name:"candidate".into(),
                tip:"a".repeat(40),contained_in_head:false}],
        }
    }
    #[test]
    fn graph_links_full_chain_without_accepting_reported_pass(){
        let h="a".repeat(40);
        let g=assemble(sample(Some(h.clone()),h)).unwrap();
        audit(&g).unwrap();
        assert!(g.nodes.iter().any(|n|n.kind==NodeKind::Requirement));
        assert!(g.nodes.iter().any(|n|n.kind==NodeKind::Component));
        assert!(g.nodes.iter().any(|n|n.kind==NodeKind::Run));
        assert!(g.nodes.iter().any(|n|n.kind==NodeKind::Acceptance&&n.evidence==EvidenceState::Unknown));
        assert!(g.edges.iter().any(|e|e.relation==Relation::ExplicitMention));
        assert!(g.edges.iter().any(|e|e.relation==Relation::ClaimsArtifact));
        assert!(!g.edges.iter().any(|e|e.evidence==EvidenceState::IndependentlyVerified));
        let mut changed=g.clone();changed.nodes[0].label="forged".into();
        assert!(audit(&changed).is_err());
    }
    #[test]
    fn earlier_pass_and_skipped_are_not_current_acceptance(){
        let mut data=sample(Some("a".repeat(40)),"c".repeat(40));
        data.runs[0].outcome="skipped".into();
        let g=assemble(data).unwrap();
        assert!(g.nodes.iter().any(|n|n.kind==NodeKind::Run&&n.evidence==EvidenceState::Stale));
        assert!(g.nodes.iter().filter(|n|n.kind==NodeKind::Acceptance)
            .all(|n|n.evidence==EvidenceState::Stale));
    }
    #[test]
    fn no_git_or_run_is_partial_unknown_not_missing(){
        let mut data=sample(None,"unknown".into());
        data.runs.clear();data.branches.clear();data.complete=false;
        let g=assemble(data).unwrap();audit(&g).unwrap();
        assert!(g.limitations.iter().any(|l|l.contains("No scoped structured")));
        assert!(!g.complete);
    }
    #[test]
    fn hostile_paths_and_forged_verification_never_admitted(){
        let mut data=sample(Some("a".repeat(40)),"a".repeat(40));
        data.files[0].path="../outside".into();assert!(assemble(data).is_err());
        let mut data=sample(Some("a".repeat(40)),"a".repeat(40));
        data.branches[0].contained_in_head=true;
        let g=assemble(data).unwrap();
        assert!(g.edges.iter().any(|e|e.relation==Relation::ContainedInHead&&
            e.evidence==EvidenceState::IndependentlyVerified));
        assert!(g.nodes.iter().filter(|n|n.kind==NodeKind::Acceptance)
            .all(|n|n.evidence!=EvidenceState::IndependentlyVerified));
    }
}
