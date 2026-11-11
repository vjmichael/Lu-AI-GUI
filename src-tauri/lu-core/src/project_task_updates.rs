//! PLAN-07A: host-reconciled evidence DELTA for durable project tasks.
//! The native host still performs a full bounded read-only source scan;
//! 'incremental' describes the audited difference between snapshots, not a
//! claimed faster scanner or completed implementation.
use crate::{project_evidence_graph as graph,project_status_classifier as status,
    project_task_register as register,project_task_synthesis as synthesis};
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::collections::{BTreeMap,BTreeSet};

pub const SCHEMA:&str="lu-project-task-evidence-refresh-v1";
const MAX_CHANGED:usize=64;
#[derive(Clone,Copy,Debug,Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Disposition{
    Unchanged,ReviewSourceChange,RequirementMissing,
    ExactCriterionReviewCandidate,NeedsIndependentEvidence,
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Candidate{
    pub task_id:String,pub expected_version:u64,
    pub original_requirement_id:String,
    pub previous_status:status::Status,pub current_status:Option<status::Status>,
    pub evidence_health:Option<status::EvidenceHealth>,
    pub disposition:Disposition,pub requires_human_review:bool,
    pub exact_criterion_candidate:bool,
    pub task_complete:bool,pub execution_authorized:bool,
    pub explanation:String,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Preview{
    pub schema:String,pub register_id:String,pub project_scope_sha256:String,
    pub expected_register_sha256:String,
    pub previous_graph_sha256:String,pub previous_status_sha256:String,
    pub current_graph_sha256:String,pub current_status_sha256:String,
    pub previous_head:Option<String>,pub current_head:Option<String>,
    pub head_changed:bool,pub semantic_change:bool,pub scan_complete:bool,
    pub changed_evidence_nodes:usize,pub changed_evidence_locators:Vec<String>,
    pub changed_list_truncated:bool,
    pub candidates:Vec<Candidate>,pub exact_criterion_review_candidates:usize,
    pub human_review_required:bool,pub no_execution_authority:bool,
    pub model_text_is_evidence:bool,pub digest_sha256:String,
}
fn digest(preview:&Preview)->Result<String,String>{
    let mut unsigned=preview.clone();
    unsigned.digest_sha256.clear();
    let bytes=serde_json::to_vec(&unsigned)
        .map_err(|_|"PLAN07_PREVIEW_SERIALIZE_FAILED")?;
    Ok(format!("{:x}",Sha256::digest(bytes)))
}
fn source_diff(old:&graph::EvidenceGraph,new:&graph::EvidenceGraph)
 ->Result<(usize,Vec<String>,bool),String>{
    // Multiple historical versions may legitimately use the same locator.
    // Node IDs incorporate original source versions, so use their unique
    // audited identity rather than treating repeated paths as malformed.
    let index=|g:&graph::EvidenceGraph|
     ->Result<BTreeMap<String,(String,String)>,String>{
        let mut values=BTreeMap::new();
        for n in &g.nodes{
            let value=serde_json::to_string(n)
                .map_err(|_|"PLAN07_NODE_SERIALIZATION_FAILED")?;
            if values.insert(n.id.clone(),
                (format!("{:?}:{}",n.kind,n.locator),value)).is_some(){
                return Err("PLAN07_DUPLICATE_EVIDENCE_NODE_ID".into())
            }
        }
        Ok(values)
    };
    let old=index(old)?;let new=index(new)?;
    let keys=old.keys().chain(new.keys()).cloned().collect::<BTreeSet<_>>();
    let mut changed=Vec::new();
    for key in keys{
        if old.get(&key)!=new.get(&key){
            let label=new.get(&key).or_else(||old.get(&key))
                .ok_or("PLAN07_NODE_DIFF_DISAPPEARED")?;
            changed.push(label.0.clone());
        }
    }
    let total=changed.len();changed.truncate(MAX_CHANGED);
    Ok((total,changed,total>MAX_CHANGED))
}
pub fn analyze(r:&register::TaskRegister,p:&synthesis::ProjectTaskProposal,
    previous_g:&graph::EvidenceGraph,previous_s:&status::StatusReport,
    current_g:&graph::EvidenceGraph,current_s:&status::StatusReport)
 ->Result<Preview,String>{
    register::audit(r,p)?;
    graph::audit(previous_g)?;status::audit(previous_s,previous_g)?;
    graph::audit(current_g)?;status::audit(current_s,current_g)?;
    if previous_g.project_scope_sha256!=r.scope_sha256||
       current_g.project_scope_sha256!=r.scope_sha256||
       current_s.project_scope_sha256!=r.scope_sha256{
        return Err("PLAN07_WRONG_SELECTED_PROJECT_SCOPE".into())
    }
    let last=r.events.last().ok_or("PLAN07_NO_ORIGINAL_TASK")?;
    if last.revision.graph_sha256!=previous_g.sha256||
       last.revision.status_sha256!=previous_s.digest_sha256{
        return Err("PLAN07_STALE_REGISTER_EVIDENCE_ANCHOR".into())
    }
    let semantically_same=synthesis::same_observed_sources(previous_g,current_g)?&&
        previous_s.complete==current_s.complete&&
        serde_json::to_value(&previous_s.findings)
            .map_err(|_|"PLAN07_PREVIOUS_STATUS_COMPARE")?==
        serde_json::to_value(&current_s.findings)
            .map_err(|_|"PLAN07_CURRENT_STATUS_COMPARE")?;
    let semantic_change=!semantically_same;
    let (total,locators,truncated)=source_diff(previous_g,current_g)?;
    let mut candidates=Vec::new();
    let mut exact=0;
    for task in &p.tasks{
        let v=register::latest(r,&task.task_id)
            .ok_or("PLAN07_NO_ORIGINAL_TASK_REVISION")?;
        if v.source_requirement_id!=task.source_requirement_id{
            return Err("PLAN07_TASK_PROVENANCE_CHANGED".into())
        }
        let finding=current_s.findings.iter().find(|f|
            f.requirement_id==v.source_requirement_id);
        let exact_native=finding.is_some_and(|f|
            f.primary==status::Status::Accepted&&
            f.evidence_health==status::EvidenceHealth::Current&&
            f.confidence==status::Confidence::Grounded&&
            !f.conflicting_claims&&current_g.complete&&current_s.complete);
        let drift=semantic_change||v.graph_sha256!=previous_g.sha256||
            v.status_sha256!=previous_s.digest_sha256;
        let possible=exact_native&&(drift||
            !v.independently_verified_requirement||v.needs_human_reconciliation);
        if possible{exact+=1}
        let disposition=if finding.is_none(){
            Disposition::RequirementMissing
        }else if possible{
            Disposition::ExactCriterionReviewCandidate
        }else if drift{
            Disposition::ReviewSourceChange
        }else if v.needs_human_reconciliation{
            Disposition::NeedsIndependentEvidence
        }else{Disposition::Unchanged};
        let explanation=match disposition{
            Disposition::RequirementMissing=>
                "Original requirement identity absent. No remapping by filename, old model claim or prose similarity.",
            Disposition::ExactCriterionReviewCandidate=>
                "Fresh native evidence supports reviewing only the exact criterion. Not whole-task completion, runtime behavior, Git integration or authorization.",
            Disposition::ReviewSourceChange=>
                "Source HEAD, original evidence, or classification changed. Prior acceptance remains historical until human reconciliation.",
            Disposition::NeedsIndependentEvidence=>
                "Human correction or uncertain evidence requires separate independent review.",
            Disposition::Unchanged=>
                "No semantic evidence change; unchanged prior status does not prove execution or full task completion.",
        };
        candidates.push(Candidate{
            task_id:task.task_id.clone(),expected_version:v.version,
            original_requirement_id:v.source_requirement_id.clone(),
            previous_status:v.observed_status,
            current_status:finding.map(|f|f.primary),
            evidence_health:finding.map(|f|f.evidence_health),
            disposition,requires_human_review:drift||
                finding.is_none()||v.needs_human_reconciliation||possible,
            exact_criterion_candidate:possible,task_complete:false,
            execution_authorized:false,explanation:explanation.into(),
        });
    }
    let mut preview=Preview{
        schema:SCHEMA.into(),register_id:r.register_id.clone(),
        project_scope_sha256:r.scope_sha256.clone(),
        expected_register_sha256:r.latest_sha256.clone(),
        previous_graph_sha256:previous_g.sha256.clone(),
        previous_status_sha256:previous_s.digest_sha256.clone(),
        current_graph_sha256:current_g.sha256.clone(),
        current_status_sha256:current_s.digest_sha256.clone(),
        previous_head:previous_g.selected_head.clone(),
        current_head:current_g.selected_head.clone(),
        head_changed:previous_g.selected_head!=current_g.selected_head,
        semantic_change,scan_complete:current_g.complete&&current_s.complete,
        changed_evidence_nodes:total,changed_evidence_locators:locators,
        changed_list_truncated:truncated,candidates,
        exact_criterion_review_candidates:exact,human_review_required:true,
        no_execution_authority:true,model_text_is_evidence:false,
        digest_sha256:String::new(),
    };
    preview.digest_sha256=digest(&preview)?;
    Ok(preview)
}
pub fn audit(preview:&Preview,r:&register::TaskRegister,
    p:&synthesis::ProjectTaskProposal,
    previous_g:&graph::EvidenceGraph,previous_s:&status::StatusReport,
    current_g:&graph::EvidenceGraph,current_s:&status::StatusReport)
 ->Result<(),String>{
    if preview.schema!=SCHEMA||!preview.human_review_required||
       !preview.no_execution_authority||preview.model_text_is_evidence||
       preview.digest_sha256!=digest(preview)?||
       preview.candidates.iter().any(|c|c.task_complete||c.execution_authorized){
        return Err("PLAN07_UNAUTHORIZED_OR_TAMPERED_PREVIEW".into())
    }
    let independent=analyze(r,p,previous_g,previous_s,current_g,current_s)?;
    if serde_json::to_vec(preview).map_err(|_|"PLAN07_AUDIT_ENCODE")?!=
       serde_json::to_vec(&independent).map_err(|_|"PLAN07_AUDIT_RECOMPUTE")?{
        return Err("PLAN07_PREVIEW_NOT_HOST_REPRODUCIBLE".into())
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    fn fixture()->(graph::EvidenceGraph,status::StatusReport,
        synthesis::ProjectTaskProposal,register::TaskRegister){
        let g=graph::assemble(graph::GraphInput{
            project_scope_sha256:"a".repeat(64),selected_head:Some("b".repeat(40)),
            observed_at_unix:42,complete:true,host_git_checked:false,
            files:vec![graph::FileInput{
                path:"docs/REQUIREMENTS.md".into(),role:"project_document".into(),
                sha256:Some("c".repeat(64))
            }],requirements:vec![graph::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:"c".repeat(64),
                line:2,statement:"- [ ] Must include src/required.rs".into(),
            }],runs:vec![],branches:vec![],extra_limitations:vec![],
        }).unwrap();
        let s=status::classify(&g,&[]).unwrap();
        let cap=synthesis::build_capsule(&g,&s).unwrap();
        let req=&cap.requirements[0];
        let raw=serde_json::json!({"schema":synthesis::RAW_SCHEMA,"tasks":[{
            "requirementId":req.requirement_id,"status":req.status,
            "summary":"Inspect required native source",
            "description":"Review the exact original file and source receipts",
            "priority":"P1","group":"01 Source evidence",
            "evidenceIds":[req.requirement_id],"dependencyIndices":[],
            "acceptanceCriteria":"Independently inspect the original source file presence",
            "blockedBy":"Original source verification missing"
        }]}).to_string();
        let p=synthesis::validate_model_response(&g,&s,&raw,
            &synthesis::model_prompt(&cap).unwrap(),
            "m-test-local","s-test-native","g-test-native").unwrap();
        let r=register::import(&p).unwrap();
        (g,s,p,r)
    }
    #[test]
    fn no_change_is_never_task_completion(){
        let (g,s,p,r)=fixture();
        let preview=analyze(&r,&p,&g,&s,&g,&s).unwrap();
        assert!(!preview.semantic_change);
        assert_eq!(preview.changed_evidence_nodes,0);
        assert!(!preview.candidates[0].task_complete);
        assert!(!preview.candidates[0].execution_authorized);
        audit(&preview,&r,&p,&g,&s,&g,&s).unwrap();
    }
    #[test]
    fn changed_head_only_suggests_reconciliation_and_refuses_replayed_history(){
        let (g,s,p,r)=fixture();
        let mut newer=g.clone();
        newer=graph::assemble(graph::GraphInput{
            project_scope_sha256:r.scope_sha256.clone(),
            selected_head:Some("d".repeat(40)),observed_at_unix:48,
            complete:true,host_git_checked:false,
            files:vec![graph::FileInput{path:"docs/REQUIREMENTS.md".into(),
                role:"project_document".into(),sha256:Some("c".repeat(64))}],
            requirements:vec![graph::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:"c".repeat(64),
                line:2,statement:"- [ ] Must include src/required.rs".into()}],
            runs:vec![],branches:vec![],extra_limitations:vec![],
        }).unwrap();
        let current=status::classify(&newer,&[]).unwrap();
        let preview=analyze(&r,&p,&g,&s,&newer,&current).unwrap();
        assert!(preview.head_changed&&preview.semantic_change);
        assert_eq!(preview.candidates[0].disposition,Disposition::ReviewSourceChange);
        assert!(!preview.candidates[0].task_complete);
        audit(&preview,&r,&p,&g,&s,&newer,&current).unwrap();
        let mut forged=preview.clone();
        forged.candidates[0].task_complete=true;
        assert!(audit(&forged,&r,&p,&g,&s,&newer,&current).is_err());
        let mut other=newer.clone();other.project_scope_sha256="e".repeat(64);
        assert!(analyze(&r,&p,&g,&s,&other,&current).is_err());
    }
    #[test]
    fn exact_accepted_path_is_candidate_only_not_execution(){
        let (g,s,p,r)=fixture();
        let root=std::env::temp_dir().join(format!(
            "lu-plan07-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/required.rs"),b"fn real_source(){}").unwrap();
        // Keep original audited project scope and exact source statement.
        let current_graph=graph::assemble(graph::GraphInput{
            project_scope_sha256:r.scope_sha256.clone(),
            selected_head:Some("d".repeat(40)),observed_at_unix:51,
            complete:true,host_git_checked:false,
            files:vec![graph::FileInput{path:"docs/REQUIREMENTS.md".into(),
                role:"project_document".into(),sha256:Some("c".repeat(64))}],
            requirements:vec![graph::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:"c".repeat(64),
                line:2,statement:"- [ ] Must include src/required.rs".into()}],
            runs:vec![],branches:vec![],extra_limitations:vec![],
        }).unwrap();
        // Presence facts require matching canonical project scope: the
        // artificial fixture deliberately cannot infer acceptance from a
        // different directory.
        let current=status::classify(&current_graph,&[]).unwrap();
        let result=analyze(&r,&p,&g,&s,&current_graph,&current).unwrap();
        assert!(!result.candidates[0].task_complete);
        assert!(!result.candidates[0].exact_criterion_candidate);
        std::fs::remove_dir_all(root).unwrap();
    }
}
