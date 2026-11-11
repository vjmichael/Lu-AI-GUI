//! PLAN-05: human-revised, evidence-bound project task history.
//! This module NEVER produces tool grants, execution plans or acceptance from model text.
use crate::{project_evidence_graph as graph,project_status_classifier as status,
    project_task_synthesis as synthesis};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::collections::{BTreeMap,BTreeSet};

pub const SCHEMA:&str="lu-project-task-register-v1";
const MAX_EVENTS:usize=1024;

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub struct HumanCorrection{
    pub summary:String,pub description:String,pub priority:String,
    pub blocked_by:String,pub acceptance_criteria:String,pub reason:String,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum RevisionKind{Imported,HumanCorrection,SourceChanged,HumanRevalidated}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub struct TaskRevision{
    pub task_id:String,pub version:u64,pub source_requirement_id:String,
    pub summary:String,pub description:String,pub group:String,pub priority:String,
    pub blocked_by:String,pub acceptance_criteria:String,
    pub depends_on:Vec<String>,pub evidence_ids:Vec<String>,
    pub original_model_proposal_sha256:String,
    pub graph_sha256:String,pub status_sha256:String,
    pub selected_head:Option<String>,pub observed_status:status::Status,
    pub observed_at_unix:u64,
    pub needs_human_reconciliation:bool,
    pub independently_verified_requirement:bool,
    pub execution_authorized:bool,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub struct TaskEvent{
    pub sequence:u64,pub kind:RevisionKind,
    pub previous_sha256:Option<String>,pub reason:String,
    pub revision:TaskRevision,pub sha256:String,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields,rename_all="camelCase")]
pub struct TaskRegister{
    pub schema:String,pub register_id:String,pub scope_sha256:String,
    pub original_proposal_sha256:String,pub events:Vec<TaskEvent>,
    pub latest_sha256:String,pub no_execution_authority:bool,
}
fn hash<T:Serialize>(value:&T)->Result<String,String>{
    let bytes=serde_json::to_vec(value).map_err(|_|"PLAN05_HASH_SERIALIZATION")?;
    Ok(format!("{:x}",Sha256::digest(bytes)))
}
fn bounded(s:&str,lo:usize,hi:usize)->bool{
    s.len()>=lo&&s.len()<=hi&&
        !s.chars().any(|c|c.is_control()&&c!='\n')
}
fn validate_revision(v:&TaskRevision)->Result<(),String>{
    if !v.task_id.starts_with("lu-task-")||
        !bounded(&v.source_requirement_id,3,100)||
        !bounded(&v.summary,3,100)||!bounded(&v.description,5,300)||
        !bounded(&v.group,3,80)||!bounded(&v.blocked_by,2,200)||
        !bounded(&v.acceptance_criteria,10,300)||
        !matches!(v.priority.as_str(),"P0"|"P1"|"P2")||
        v.execution_authorized||v.version==0||
        v.evidence_ids.is_empty()||v.evidence_ids.len()>12||
        !v.evidence_ids.contains(&v.source_requirement_id)||
        v.depends_on.len()>8||!graph::valid_digest(&v.graph_sha256)||
        !graph::valid_digest(&v.status_sha256)||
        !graph::valid_digest(&v.original_model_proposal_sha256)||
        (v.independently_verified_requirement&&
            (v.needs_human_reconciliation||
                v.observed_status!=status::Status::Accepted)){
        return Err("PLAN05_INVALID_REVISION_OR_AUTHORITY".into())
    }
    Ok(())
}
fn seal(e:&mut TaskEvent)->Result<(),String>{
    e.sha256.clear();e.sha256=hash(e)?;Ok(())
}
fn append(r:&mut TaskRegister,mut event:TaskEvent)->Result<(),String>{
    if r.events.len()>=MAX_EVENTS{return Err("PLAN05_HISTORY_BUDGET".into())}
    event.sequence=r.events.len() as u64+1;
    event.previous_sha256=r.events.last().map(|e|e.sha256.clone());
    seal(&mut event)?;
    r.latest_sha256=event.sha256.clone();
    r.events.push(event);
    Ok(())
}
pub fn latest<'a>(r:&'a TaskRegister,task_id:&str)->Option<&'a TaskRevision>{
    r.events.iter().rev().find(|e|e.revision.task_id==task_id)
        .map(|e|&e.revision)
}
pub fn audit(r:&TaskRegister,p:&synthesis::ProjectTaskProposal)
 ->Result<(),String>{
    if r.schema!=SCHEMA||!r.no_execution_authority||
        !p.no_execution_authority||!p.human_review_required||
        r.scope_sha256!=p.project_scope_sha256||
        r.original_proposal_sha256!=p.digest_sha256||
        r.register_id!=format!("lu-register-{}",
            &hash(&(p.project_scope_sha256.as_str(),p.digest_sha256.as_str()))?[..24])||
        r.events.len()<p.tasks.len()||r.events.len()>MAX_EVENTS||
        p.tasks.is_empty()||p.tasks.len()>16{
        return Err("PLAN05_REGISTER_PROVENANCE_INVALID".into())
    }
    let ids=p.tasks.iter().map(|t|t.task_id.as_str())
        .collect::<BTreeSet<_>>();
    if ids.len()!=p.tasks.len(){return Err("PLAN05_DUPLICATE_ORIGINAL_TASK".into())}
    let mut previous=None::<String>;
    let mut versions=BTreeMap::<String,TaskRevision>::new();
    for (i,e) in r.events.iter().enumerate(){
        let v=&e.revision;
        validate_revision(v)?;
        if e.sequence!=i as u64+1||e.previous_sha256!=previous||
            !bounded(&e.reason,2,250)||
            e.sha256.len()!=64{
            return Err("PLAN05_REVISION_CHAIN_BROKEN".into())
        }
        let mut unsealed=e.clone();unsealed.sha256.clear();
        if hash(&unsealed)?!=e.sha256{
            return Err("PLAN05_REVISION_BYTES_TAMPERED".into())
        }
        if !ids.contains(v.task_id.as_str())||
            v.original_model_proposal_sha256!=p.digest_sha256{
            return Err("PLAN05_FOREIGN_TASK".into())
        }
        if i<p.tasks.len(){
            let original=&p.tasks[i];
            if e.kind!=RevisionKind::Imported||v.version!=1||
                v.task_id!=original.task_id||
                v.source_requirement_id!=original.source_requirement_id||
                v.summary!=original.summary||v.description!=original.description||
                v.priority!=original.priority||v.group!=original.group||
                v.blocked_by!=original.blocked_by||
                v.acceptance_criteria!=original.acceptance_criteria||
                v.depends_on!=original.depends_on||
                v.evidence_ids!=original.evidence_ids||
                v.graph_sha256!=p.graph_sha256||
                v.status_sha256!=p.status_sha256||
                v.selected_head!=p.selected_head||
                v.observed_status!=original.status||
                v.independently_verified_requirement||
                v.needs_human_reconciliation{
                return Err("PLAN05_IMPORT_NOT_ORIGINAL_MODEL_DRAFT".into())
            }
        }else{
            let old=versions.get(&v.task_id)
                .ok_or("PLAN05_REVISION_WITHOUT_ORIGINAL")?;
            if v.version!=old.version+1||
                v.task_id!=old.task_id||
                v.source_requirement_id!=old.source_requirement_id||
                v.group!=old.group||
                v.depends_on!=old.depends_on||
                v.evidence_ids!=old.evidence_ids{
                return Err("PLAN05_REVISIONS_CANNOT_REWRITE_PROVENANCE".into())
            }
            match e.kind{
                RevisionKind::Imported=>return Err("PLAN05_DUPLICATE_IMPORT".into()),
                RevisionKind::HumanCorrection=>{
                    if !v.needs_human_reconciliation||
                        v.independently_verified_requirement||
                        v.graph_sha256!=old.graph_sha256||
                        v.status_sha256!=old.status_sha256||
                        v.selected_head!=old.selected_head||
                        v.observed_status!=old.observed_status{
                        return Err("PLAN05_CORRECTION_CANNOT_CHANGE_EVIDENCE".into())
                    }
                }
                RevisionKind::SourceChanged=>{
                    if !v.needs_human_reconciliation||
                        v.independently_verified_requirement||
                        v.summary!=old.summary||
                        v.description!=old.description||
                        v.priority!=old.priority||
                        v.blocked_by!=old.blocked_by||
                        v.acceptance_criteria!=old.acceptance_criteria{
                        return Err("PLAN05_SOURCE_CANNOT_EDIT_HUMAN_TEXT".into())
                    }
                }
                RevisionKind::HumanRevalidated=>{
                    if v.needs_human_reconciliation||
                        v.summary!=old.summary||v.description!=old.description||
                        v.priority!=old.priority||
                        v.blocked_by!=old.blocked_by||
                        v.acceptance_criteria!=old.acceptance_criteria{
                        return Err("PLAN05_REVALIDATION_CANNOT_EDIT_TASK".into())
                    }
                }
            }
        }
        versions.insert(v.task_id.clone(),v.clone());
        previous=Some(e.sha256.clone());
    }
    if previous.as_deref()!=Some(r.latest_sha256.as_str())||
        versions.len()!=p.tasks.len(){
        return Err("PLAN05_INCOMPLETE_REGISTER".into())
    }
    Ok(())
}
pub fn import(p:&synthesis::ProjectTaskProposal)
 ->Result<TaskRegister,String>{
    if !p.human_review_required||!p.no_execution_authority||
        p.tasks.is_empty()||p.tasks.len()>16{
        return Err("PLAN05_NOT_REVIEWABLE_PROPOSAL".into())
    }
    let mut r=TaskRegister{
        schema:SCHEMA.into(),register_id:format!("lu-register-{}",
            &hash(&(p.project_scope_sha256.as_str(),p.digest_sha256.as_str()))?[..24]),
        scope_sha256:p.project_scope_sha256.clone(),
        original_proposal_sha256:p.digest_sha256.clone(),
        events:vec![],latest_sha256:String::new(),no_execution_authority:true,
    };
    for t in &p.tasks{
        append(&mut r,TaskEvent{
            sequence:0,kind:RevisionKind::Imported,previous_sha256:None,
            reason:"Reviewed model proposal imported; not executed or accepted".into(),
            revision:TaskRevision{
                task_id:t.task_id.clone(),version:1,
                source_requirement_id:t.source_requirement_id.clone(),
                summary:t.summary.clone(),description:t.description.clone(),
                group:t.group.clone(),priority:t.priority.clone(),
                blocked_by:t.blocked_by.clone(),
                acceptance_criteria:t.acceptance_criteria.clone(),
                depends_on:t.depends_on.clone(),evidence_ids:t.evidence_ids.clone(),
                original_model_proposal_sha256:p.digest_sha256.clone(),
                graph_sha256:p.graph_sha256.clone(),status_sha256:p.status_sha256.clone(),
                selected_head:p.selected_head.clone(),observed_status:t.status,
                observed_at_unix:0,needs_human_reconciliation:false,
                independently_verified_requirement:false,execution_authorized:false,
            },sha256:String::new(),
        })?;
    }
    audit(&r,p)?;Ok(r)
}
pub fn human_correct(r:&TaskRegister,p:&synthesis::ProjectTaskProposal,
    task_id:&str,expected_version:u64,c:&HumanCorrection)
 ->Result<TaskRegister,String>{
    audit(r,p)?;
    if !bounded(&c.summary,3,100)||!bounded(&c.description,5,300)||
        !matches!(c.priority.as_str(),"P0"|"P1"|"P2")||
        !bounded(&c.blocked_by,2,200)||
        !bounded(&c.acceptance_criteria,10,300)||
        !bounded(&c.reason,5,250){
        return Err("PLAN05_HUMAN_CORRECTION_FIELDS_INVALID".into())
    }
    let old=latest(r,task_id).ok_or("PLAN05_TASK_NOT_FOUND")?;
    if old.version!=expected_version{return Err("PLAN05_STALE_EDIT_EXPECTED_VERSION".into())}
    let mut new=r.clone();let mut v=old.clone();v.version+=1;
    v.summary=c.summary.clone();v.description=c.description.clone();
    v.priority=c.priority.clone();v.blocked_by=c.blocked_by.clone();
    v.acceptance_criteria=c.acceptance_criteria.clone();
    v.needs_human_reconciliation=true;
    v.independently_verified_requirement=false;
    append(&mut new,TaskEvent{sequence:0,kind:RevisionKind::HumanCorrection,
        previous_sha256:None,reason:c.reason.clone(),revision:v,
        sha256:String::new()})?;
    audit(&new,p)?;Ok(new)
}
fn observation<'a>(r:&TaskRegister,g:&graph::EvidenceGraph,
    s:&'a status::StatusReport,task_id:&str)
 ->Result<&'a status::Finding,String>{
    graph::audit(g)?;status::audit(s,g)?;
    if g.project_scope_sha256!=r.scope_sha256{
        return Err("PLAN05_FOREIGN_SELECTED_PROJECT".into())
    }
    let current=latest(r,task_id).ok_or("PLAN05_UNKNOWN_TASK")?;
    s.findings.iter().find(|f|f.requirement_id==current.source_requirement_id)
        .ok_or("PLAN05_SOURCE_REQUIREMENT_REMOVED_REVIEW_REQUIRED".into())
}
pub fn source_changed(r:&TaskRegister,p:&synthesis::ProjectTaskProposal,
    g:&graph::EvidenceGraph,s:&status::StatusReport)
 ->Result<TaskRegister,String>{
    audit(r,p)?;graph::audit(g)?;status::audit(s,g)?;
    if r.scope_sha256!=g.project_scope_sha256{
        return Err("PLAN05_FOREIGN_PROJECT_RECONCILIATION".into())
    }
    let mut new=r.clone();
    for task in &p.tasks{
        let old=latest(&new,&task.task_id).ok_or("PLAN05_TASK_NOT_FOUND")?;
        let current_status=s.findings.iter()
            .find(|f|f.requirement_id==old.source_requirement_id);
        let next=current_status.map(|f|f.primary).unwrap_or(status::Status::Unknown);
        let changed=old.graph_sha256!=g.sha256||
            old.status_sha256!=s.digest_sha256||
            old.selected_head!=g.selected_head||
            old.observed_status!=next;
        if !changed{continue}
        let mut v=old.clone();v.version+=1;
        v.graph_sha256=g.sha256.clone();v.status_sha256=s.digest_sha256.clone();
        v.selected_head=g.selected_head.clone();v.observed_status=next;
        v.observed_at_unix=s.observed_at_unix;
        v.needs_human_reconciliation=true;
        v.independently_verified_requirement=false;
        append(&mut new,TaskEvent{sequence:0,kind:RevisionKind::SourceChanged,
            previous_sha256:None,
            reason:if current_status.is_none(){
                "Original source requirement missing; mapping requires human review".into()
            }else{
                "Original source, exact HEAD or native classification changed; previous acceptance is historical".into()
            },revision:v,sha256:String::new()})?;
    }
    audit(&new,p)?;Ok(new)
}
pub fn human_revalidate(r:&TaskRegister,p:&synthesis::ProjectTaskProposal,
    task_id:&str,expected_version:u64,g:&graph::EvidenceGraph,
    s:&status::StatusReport)
 ->Result<TaskRegister,String>{
    audit(r,p)?;
    let f=observation(r,g,s,task_id)?;
    let old=latest(r,task_id).ok_or("PLAN05_TASK_NOT_FOUND")?;
    if old.version!=expected_version{return Err("PLAN05_STALE_REVALIDATION".into())}
    let accepted=s.complete&&g.complete&&
        f.primary==status::Status::Accepted&&
        f.evidence_health==status::EvidenceHealth::Current&&
        !f.conflicting_claims;
    let mut v=old.clone();v.version+=1;
    v.graph_sha256=g.sha256.clone();v.status_sha256=s.digest_sha256.clone();
    v.selected_head=g.selected_head.clone();v.observed_status=f.primary;
    v.observed_at_unix=s.observed_at_unix;
    v.needs_human_reconciliation=false;
    v.independently_verified_requirement=accepted;
    let mut new=r.clone();
    append(&mut new,TaskEvent{sequence:0,kind:RevisionKind::HumanRevalidated,
        previous_sha256:None,
        reason:if accepted{
            "Human reviewed current native evidence; exact requirement independently accepted, not execution".into()
        }else{
            "Human reviewed current native evidence; acceptance remains unverified or incomplete".into()
        },revision:v,sha256:String::new()})?;
    audit(&new,p)?;Ok(new)
}
#[cfg(test)]
mod tests{
    use super::*;
    fn fixture()->(graph::EvidenceGraph,status::StatusReport,synthesis::ProjectTaskProposal){
        let g=graph::assemble(graph::GraphInput{
            project_scope_sha256:"f".repeat(64),
            selected_head:Some("a".repeat(40)),observed_at_unix:42,
            complete:true,host_git_checked:true,
            files:vec![graph::FileInput{
                path:"docs/REQUIREMENTS.md".into(),role:"project_document".into(),
                sha256:Some("b".repeat(64))}],
            requirements:vec![graph::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:"b".repeat(64),
                line:2,statement:"- [ ] Must include src/required.rs".into()}],
            runs:vec![],branches:vec![],extra_limitations:vec![],
        }).unwrap();
        let r=status::classify(&g,&[]).unwrap();
        let cap=synthesis::build_capsule(&g,&r).unwrap();
        let f=&cap.requirements[0];
        let raw=serde_json::json!({"schema":synthesis::RAW_SCHEMA,"tasks":[{
            "requirementId":f.requirement_id,"status":f.status,
            "summary":"Add required source",
            "description":"Inspect and add the independently required source file",
            "priority":"P1","group":"01 Source work",
            "evidenceIds":[f.requirement_id],"dependencyIndices":[],
            "acceptanceCriteria":"Verify the exact required file with native inspection",
            "blockedBy":"Missing native evidence"
        }]}).to_string();
        let p=synthesis::validate_model_response(&g,&r,&raw,
            &synthesis::model_prompt(&cap).unwrap(),"m-qwen","s-live","g-real").unwrap();
        (g,r,p)
    }
    #[test]
    fn human_correction_is_versioned_and_never_grants_execution(){
        let (_g,_s,p)=fixture();let r=import(&p).unwrap();
        let id=p.tasks[0].task_id.as_str();
        let revised=human_correct(&r,&p,id,1,&HumanCorrection{
            summary:"Clarify exact source location".into(),
            description:"Manually inspect and correct the required source target".into(),
            priority:"P0".into(),blocked_by:"No original test run".into(),
            acceptance_criteria:"Independent current source presence check".into(),
            reason:"Human corrected original task".into(),
        }).unwrap();
        assert_eq!(latest(&revised,id).unwrap().version,2);
        assert_eq!(revised.events.len(),2);
        assert!(latest(&revised,id).unwrap().needs_human_reconciliation);
        assert!(!latest(&revised,id).unwrap().execution_authorized);
        assert!(human_correct(&revised,&p,id,1,&HumanCorrection{
            summary:"Try".into(),description:"Invalid stale expected edit".into(),
            priority:"P0".into(),blocked_by:"No original test run".into(),
            acceptance_criteria:"Independent current source presence check".into(),
            reason:"Conflicting user edit".into(),
        }).is_err());
        let mut tampered=revised.clone();
        tampered.events[0].revision.summary="Injected approval".into();
        assert!(audit(&tampered,&p).is_err());
    }
    #[test]
    fn only_fresh_native_presence_criterion_can_be_marked_accepted(){
        let root=std::env::temp_dir().join(format!(
            "lu-plan05-presence-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        let requirement=b"# Requirements\n- [ ] Must include src/required.rs\n";
        std::fs::write(root.join("docs/REQUIREMENTS.md"),requirement).unwrap();
        std::fs::write(root.join("src/required.rs"),b"fn required() {}").unwrap();
        let canonical=root.canonicalize().unwrap();
        let scope=format!("{:x}",Sha256::digest(
            canonical.to_string_lossy().as_bytes()));
        let doc_sha=format!("{:x}",Sha256::digest(requirement));
        let g=graph::assemble(graph::GraphInput{
            project_scope_sha256:scope,selected_head:None,
            observed_at_unix:42,complete:true,host_git_checked:false,
            files:vec![graph::FileInput{
                path:"docs/REQUIREMENTS.md".into(),role:"project_document".into(),
                sha256:Some(doc_sha.clone())}],
            requirements:vec![graph::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:doc_sha,
                line:2,statement:"- [ ] Must include src/required.rs".into()}],
            runs:vec![],branches:vec![],extra_limitations:vec![],
        }).unwrap();
        let original_status=status::classify(&g,&[]).unwrap();
        let cap=synthesis::build_capsule(&g,&original_status).unwrap();
        let req=&cap.requirements[0];
        let raw=serde_json::json!({"schema":synthesis::RAW_SCHEMA,"tasks":[{
            "requirementId":req.requirement_id,"status":req.status,
            "summary":"Verify required original source",
            "description":"Check the required file against native source evidence",
            "priority":"P1","group":"01 Source work",
            "evidenceIds":[req.requirement_id],"dependencyIndices":[],
            "acceptanceCriteria":"Independently verify the exact required source file",
            "blockedBy":"Native source check missing"
        }]}).to_string();
        let p=synthesis::validate_model_response(&g,&original_status,&raw,
            &synthesis::model_prompt(&cap).unwrap(),
            "m-qwen","s-live","g-real").unwrap();
        let facts=status::observe_project_paths(&root,&g).unwrap();
        let fresh=status::classify(&g,&facts).unwrap();
        status::audit(&fresh,&g).unwrap();
        assert_eq!(fresh.findings[0].primary,status::Status::Accepted);
        let original=import(&p).unwrap();
        let confirmed=human_revalidate(&original,&p,&p.tasks[0].task_id,
            1,&g,&fresh).unwrap();
        let event=confirmed.events.last().unwrap();
        assert_eq!(event.kind,RevisionKind::HumanRevalidated);
        assert!(event.revision.independently_verified_requirement);
        assert!(!event.revision.execution_authorized);
        let old=latest(&confirmed,&p.tasks[0].task_id).unwrap();
        assert!(!old.needs_human_reconciliation);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn source_change_never_inherits_old_acceptance(){
        let (g,s,p)=fixture();let r=import(&p).unwrap();
        let mut newer=g.clone();
        newer.selected_head=Some("c".repeat(40));
        // Reassemble rather than forging a valid graph digest.
        let newer=graph::assemble(graph::GraphInput{
            project_scope_sha256:g.project_scope_sha256.clone(),
            selected_head:newer.selected_head.clone(),
            observed_at_unix:50,complete:true,host_git_checked:true,
            files:vec![graph::FileInput{
                path:"docs/REQUIREMENTS.md".into(),role:"project_document".into(),
                sha256:Some("b".repeat(64))}],
            requirements:vec![graph::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:"b".repeat(64),
                line:2,statement:"- [ ] Must include src/required.rs".into()}],
            runs:vec![],branches:vec![],extra_limitations:vec![],
        }).unwrap();
        let changed_status=status::classify(&newer,&[]).unwrap();
        let drift=source_changed(&r,&p,&newer,&changed_status).unwrap();
        let version=latest(&drift,&p.tasks[0].task_id).unwrap();
        assert!(version.needs_human_reconciliation);
        assert!(!version.independently_verified_requirement);
        assert_ne!(version.graph_sha256,s.graph_sha256);
        assert!(human_revalidate(&drift,&p,&p.tasks[0].task_id,1,
            &newer,&changed_status).is_err());
        let review=human_revalidate(&drift,&p,&p.tasks[0].task_id,version.version,
            &newer,&changed_status).unwrap();
        assert!(!latest(&review,&p.tasks[0].task_id).unwrap().independently_verified_requirement);
    }
}
