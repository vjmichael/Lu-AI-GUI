//! PLAN-03: conservative CPU-only status from an audited evidence graph.
//! No model inference, task authorization, tool execution, or report promotion.
use crate::project_evidence_graph as eg;
use serde::{Serialize,Deserialize};
use sha2::{Sha256,Digest};
use std::collections::{BTreeSet,BTreeMap};
pub const SCHEMA:&str="lu-project-status-v1";
#[derive(Clone,Copy,Debug,Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="SCREAMING_SNAKE_CASE")]
pub enum Status{Missing,Partial,NotIntegrated,Broken,Unverified,Accepted,Blocked,Unknown}
#[derive(Clone,Copy,Debug,Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Confidence{Insufficient,Limited,Grounded}
#[derive(Clone,Copy,Debug,Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum EvidenceHealth{Current,Historical,Conflicting,Insufficient}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum PathObservation{Present,Absent,Unavailable,PermissionDenied,ValidJson,InvalidJson}
#[derive(Clone,Debug)]
pub struct ScopedPathFact{
    path:String,observation:PathObservation,scope_sha256:String,
    observed_head:Option<String>,
}
impl ScopedPathFact{
    pub fn path(&self)->&str{&self.path}
    pub fn observation(&self)->PathObservation{self.observation}
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Finding{
    pub requirement_id:String,pub source_path:String,pub line:Option<u32>,
    pub requirement:String,pub primary:Status,pub dimensions:Vec<Status>,
    pub confidence:Confidence,pub evidence_health:EvidenceHealth,
    pub observed_targets:usize,pub required_targets:usize,
    pub supporting_node_ids:Vec<String>,pub conflicting_claims:bool,
    pub explanation:String,pub next_evidence:String,
}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
pub struct StatusReport{
    pub schema:String,pub project_scope_sha256:String,pub graph_sha256:String,
    pub selected_head:Option<String>,pub observed_at_unix:u64,
    pub complete:bool,pub findings:Vec<Finding>,pub limitations:Vec<String>,
    pub no_execution_authority:bool,pub digest_sha256:String,
}
pub fn explicit_targets(statement:&str)->Vec<String>{
    let allowed=["src/","tests/","test/","docs/","config/",
        "assets/",".github/","lib/","app/","packages/"];
    let mut found=BTreeSet::new();
    for token in statement.split(|c:char|c.is_whitespace()||
        matches!(c,'\u{60}'|'"'|'\''|'('|')'|'['|']'|','|';'|'=')){
        let p=token.trim_matches(|c:char|matches!(c,'.'|':'|'!'|'?'|'*'));
        if allowed.iter().any(|prefix|p.starts_with(prefix))&&
           eg::valid_path(p)&&p.rsplit('/').next()
             .is_some_and(|name|name.contains('.')){
            found.insert(p.to_string());
            if found.len()>=12{break}
        }
    }
    found.into_iter().collect()
}
fn reported_runs(g:&eg::EvidenceGraph,req:&eg::EvidenceNode)
 ->(bool,bool,bool,Vec<String>){
    let mut tests=BTreeSet::new();
    let mut sources=BTreeSet::new();
    for e in g.edges.iter().filter(|e|e.from==req.id&&
        e.relation==eg::Relation::ExplicitMention){
        if let Some(n)=g.nodes.iter().find(|n|n.id==e.to){
            match n.kind{
                eg::NodeKind::Test=>{tests.insert(n.id.clone());},
                eg::NodeKind::Component=>{sources.insert(n.id.clone());},
                _=>{},
            }
        }
    }
    for e in g.edges.iter().filter(|e|
        e.relation==eg::Relation::ChecksSource&&sources.contains(&e.to)){
        tests.insert(e.from.clone());
    }
    let (mut stale,mut pass,mut fail)=(false,false,false);
    let mut ids=Vec::new();
    for e in g.edges.iter().filter(|e|e.relation==eg::Relation::ReportedBy&&
        tests.contains(&e.from)){
        let Some(run)=g.nodes.iter().find(|n|n.id==e.to&&n.kind==eg::NodeKind::Run)
            else{continue};
        ids.push(run.id.clone());
        if run.evidence==eg::EvidenceState::Stale{stale=true;continue}
        if run.evidence!=eg::EvidenceState::Reported{continue}
        for link in g.edges.iter().filter(|link|link.from==run.id&&
            link.relation==eg::Relation::ClaimsOutcome){
            let Some(outcome)=g.nodes.iter().find(|n|n.id==link.to&&
                n.kind==eg::NodeKind::Acceptance)else{continue};
            pass|=outcome.label=="passed";
            fail|=outcome.label=="failed";
        }
    }
    (stale,pass,fail,ids)
}
pub fn classify(g:&eg::EvidenceGraph,facts:&[ScopedPathFact])->Result<StatusReport,String>{
    eg::audit(g)?;
    if facts.len()>256||g.nodes.iter().filter(|n|n.kind==eg::NodeKind::Requirement)
        .count()>128{return Err("PLAN03_BUDGET".into())}
    let mut paths=BTreeMap::new();
    for f in facts{
        if !eg::valid_path(&f.path)||f.scope_sha256!=g.project_scope_sha256||
            f.observed_head!=g.selected_head||
            paths.insert(f.path.clone(),f.clone()).is_some(){
            return Err("PLAN03_FOREIGN_OR_DUPLICATE_PATH_PROOF".into())
        }
    }
    let mut findings=Vec::new();
    for req in g.nodes.iter().filter(|n|n.kind==eg::NodeKind::Requirement){
        let origin=g.edges.iter().any(|e|e.from==req.id&&
            e.relation==eg::Relation::OriginatesFrom&&
            e.evidence==eg::EvidenceState::Inspected&&
            g.nodes.iter().any(|n|n.id==e.to&&n.digest==req.digest));
        let targets=explicit_targets(&req.label);
        let mut supporting=g.edges.iter().filter(|e|e.from==req.id&&
            e.relation==eg::Relation::ExplicitMention)
            .map(|e|e.to.clone()).collect::<Vec<_>>();
        let (mut seen,mut present,mut absent)=(0,0,0);
        for target in &targets{
            match paths.get(target).map(|f|f.observation){
                Some(PathObservation::Present|PathObservation::ValidJson|
                    PathObservation::InvalidJson)=>{seen+=1;present+=1;},
                Some(PathObservation::Absent)=>{seen+=1;absent+=1;},
                _=>{},
            }
        }
        let (stale,pass,fail,runs)=reported_runs(g,req);
        supporting.extend(runs);
        let conflicting=pass&&fail;
        let mut not_integrated=false;
        for branch in g.nodes.iter().filter(|n|n.kind==eg::NodeKind::IntegrationPath){
            if req.label.contains(&format!("refs/heads/{}",branch.label))&&
                g.edges.iter().any(|e|e.from==branch.id&&
                    e.relation==eg::Relation::NotContainedInHead&&
                    e.evidence==eg::EvidenceState::IndependentlyVerified){
                not_integrated=true;supporting.push(branch.id.clone());
            }
        }
        supporting.sort();supporting.dedup();
        let health=if conflicting{EvidenceHealth::Conflicting}
            else if stale{EvidenceHealth::Historical}
            else if !origin{EvidenceHealth::Insufficient}
            else{EvidenceHealth::Current};
        let complete_targets=!targets.is_empty()&&seen==targets.len();
        let mut dimensions=Vec::new();
        let mut reason=Vec::new();
        let mut needed=Vec::new();
        let exact_target=if targets.len()==1{Some(targets[0].as_str())}else{None};
        let syntax_gate=exact_target.is_some_and(|target|
            req.label.trim()==format!("Must validate JSON syntax of {target}")&&
            target.ends_with(".json"));
        let presence_gate=exact_target.is_some_and(|target|
            req.label.trim()==format!("Must include {target}"));
        let branch_gate=g.nodes.iter().filter(|n|n.kind==eg::NodeKind::IntegrationPath)
            .any(|branch|req.label.trim()==format!(
                    "Must contain refs/heads/{} in selected HEAD",branch.label)
                &&g.edges.iter().any(|e|e.from==branch.id&&
                    e.relation==eg::Relation::ContainedInHead&&
                    e.evidence==eg::EvidenceState::IndependentlyVerified));
        let has_permission_blocker=targets.iter().any(|target|
            paths.get(target).is_some_and(|f|
                f.observation==PathObservation::PermissionDenied));
        if !origin{
            dimensions.push(Status::Unknown);
            reason.push("Original requirement document hash is not currently proven.");
            needed.push("Reopen the original requirements document and compare source SHA.");
        }else if conflicting{
            dimensions.push(Status::Unknown);
            reason.push("Untrusted current-head reports contradict each other.");
            needed.push("Obtain independent exact-head test outcome receipts.");
        }else{
            if branch_gate{
                dimensions.push(Status::Accepted);
                reason.push("The exact branch-inclusion criterion was independently verified by native Git; no functional or installed acceptance inferred.");
            }
            if has_permission_blocker{
                dimensions.push(Status::Blocked);
                reason.push("Native scoped inspection was denied by OS permissions, not a missing file.");
                needed.push("Grant operator-approved read access, then repeat inspection.");
            }
            if syntax_gate&&complete_targets&&g.complete&&
                paths.get(exact_target.unwrap()).is_some_and(|f|
                    f.observation==PathObservation::InvalidJson){
                dimensions.push(Status::Broken);
                reason.push("Bounded native JSON parsing independently reproduced a syntax failure for the exact named file; other behavior not assessed.");
                needed.push("Correct JSON syntax and independently recheck the same file.");
            }else if syntax_gate&&complete_targets&&g.complete&&
                paths.get(exact_target.unwrap()).is_some_and(|f|
                    f.observation==PathObservation::ValidJson){
                dimensions.push(Status::Accepted);
                reason.push("Bounded native JSON parser verified only the stated JSON syntax criterion, not application behavior.");
            }else if presence_gate&&complete_targets&&g.complete&&present==1{
                dimensions.push(Status::Accepted);
                reason.push("Native file metadata established the exact presence-only criterion, not correctness of file behavior.");
            }
            if complete_targets&&g.complete&&absent==targets.len(){
                dimensions.push(Status::Missing);
                reason.push("All explicitly required paths are natively absent in selected scope; no semantic feature absence inferred.");
                needed.push("Confirm named paths and supply missing source targets.");
            }else if complete_targets&&g.complete&&present>0&&absent>0{
                dimensions.push(Status::Partial);
                reason.push("Some explicitly named files exist; other named files are absent. Behavior is untested.");
                needed.push("Inspect missing targets, integration, and tests.");
            }else if (present>0||!supporting.is_empty())&&
                !dimensions.contains(&Status::Accepted)&&
                !dimensions.contains(&Status::Broken){
                dimensions.push(Status::Unverified);
                reason.push("Relevant path evidence exists, but runtime and acceptance are not independently verified.");
                needed.push("Collect original current-head native, installed and human acceptance receipts.");
            }else if !not_integrated&&!dimensions.contains(&Status::Accepted)&&
                !dimensions.contains(&Status::Blocked){
                dimensions.push(Status::Unknown);
                reason.push("Implementation evidence is insufficient for a specific verdict.");
                needed.push("Inspect explicit targets or expand source coverage.");
            }
        }
        if stale{
            reason.push("An earlier or unknown-SHA run is stale.");
            needed.push("Re-run exact-head acceptance.");
        }
        if !g.complete{
            reason.push("The source scan was partial.");
            needed.push("Improve scoped coverage; never infer missing functionality from a search miss.");
        }
        if pass&&!fail{reason.push("A current-head report CLAIMS pass, not verified acceptance.");}
        if fail&&!pass{
            reason.push("A current-head report CLAIMS failure, not independently reproduced BROKEN.");
            needed.push("Independently reproduce the failure.");
        }
        if dimensions.is_empty(){dimensions.push(Status::Unknown)}
        dimensions.sort_by_key(|s|match s{
            Status::Broken=>0,Status::Blocked=>1,Status::NotIntegrated=>2,
            Status::Partial=>3,Status::Missing=>4,Status::Accepted=>5,
            Status::Unverified=>6,Status::Unknown=>7,
        });
        dimensions.dedup();
        let confidence=if !origin||conflicting{Confidence::Insufficient}
            else if complete_targets&&g.complete||not_integrated||branch_gate{Confidence::Grounded}
            else{Confidence::Limited};
        findings.push(Finding{
            requirement_id:req.id.clone(),source_path:req.locator.clone(),
            line:req.line,requirement:req.label.clone(),primary:dimensions[0],
            dimensions,confidence,evidence_health:health,
            observed_targets:seen,required_targets:targets.len(),
            supporting_node_ids:supporting,conflicting_claims:conflicting,
            explanation:reason.join(" "),next_evidence:needed.join(" "),
        });
    }
    let mut limitations=g.limitations.clone();
    limitations.push("BROKEN/ACCEPTED/BLOCKED findings are limited to exact native path, JSON syntax, permission and Git inclusion criteria. Broader runtime, integration, installed and human acceptance require separate authenticated original receipts.".into());
    limitations.push("Self-reported CI passes/failures never establish BROKEN or ACCEPTED; MISSING/PARTIAL only cover explicit paths, not general feature completeness.".into());
    let mut result=StatusReport{
        schema:SCHEMA.into(),project_scope_sha256:g.project_scope_sha256.clone(),
        graph_sha256:g.sha256.clone(),selected_head:g.selected_head.clone(),
        inspected_at_unix:g.observed_at_unix,complete:g.complete,
        findings,limitations,no_execution_authority:true,digest_sha256:String::new(),
    };
    result.digest_sha256=hash(&result)?;
    Ok(result)
}
fn hash(r:&StatusReport)->Result<String,String>{
    let mut copy=r.clone();copy.digest_sha256.clear();
    let bytes=serde_json::to_vec(&copy).map_err(|_|"PLAN03_SERIALIZE")?;
    Ok(format!("{:x}",Sha256::digest(&bytes)))
}
pub fn audit(r:&StatusReport,g:&eg::EvidenceGraph)->Result<(),String>{
    eg::audit(g)?;
    if r.schema!=SCHEMA||!r.no_execution_authority||
        r.project_scope_sha256!=g.project_scope_sha256||
        r.graph_sha256!=g.sha256||r.selected_head!=g.selected_head||
        r.inspected_at_unix!=g.observed_at_unix||r.complete!=g.complete||
        r.findings.len()>128||hash(r)?!=r.digest_sha256{
        return Err("PLAN03_CLASSIFICATION_CHANGED_OR_STALE".into())
    }
    Ok(())
}
/// Observe explicit relative targets only under an approved native root.
/// Model text never supplies paths to this function.
pub fn observe_project_paths(root:&std::path::Path,g:&eg::EvidenceGraph)
 ->Result<Vec<ScopedPathFact>,String>{
    use cap_std::{ambient_authority,fs::Dir};
    eg::audit(g)?;
    let meta=std::fs::symlink_metadata(root).map_err(|_|"PLAN03_ROOT_MISSING")?;
    if !meta.is_dir()||meta.file_type().is_symlink(){
        return Err("PLAN03_INVALID_ROOT".into())
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return Err("PLAN03_REPARSE_ROOT".into())}
    }
    let canonical=root.canonicalize().map_err(|_|"PLAN03_ROOT_UNAVAILABLE")?;
    let scope=format!("{:x}",Sha256::digest(canonical.to_string_lossy().as_bytes()));
    if scope!=g.project_scope_sha256{return Err("PLAN03_DIFFERENT_PROJECT".into())}
    let dir=Dir::open_ambient_dir(&canonical,ambient_authority())
        .map_err(|_|"PLAN03_SCOPE_OPEN_FAILED")?;
    let mut paths=BTreeSet::new();
    for req in g.nodes.iter().filter(|n|n.kind==eg::NodeKind::Requirement){
        for p in explicit_targets(&req.label){
            paths.insert(p);
            if paths.len()>256{return Err("PLAN03_TARGET_BUDGET".into())}
        }
    }
    let mut facts=Vec::new();
    for path in paths{
        let mut portion=String::new();
        let parts=path.split('/').collect::<Vec<_>>();
        let mut observation=PathObservation::Unavailable;
        for (i,p) in parts.iter().enumerate(){
            if i>0{portion.push('/')}
            portion.push_str(p);
            match dir.symlink_metadata(&portion){
                Ok(m)=>{
                    if m.file_type().is_symlink()||(!m.is_dir()&&i+1<parts.len()){
                        observation=PathObservation::Unavailable;break
                    }
                    #[cfg(windows)]{
                        use std::os::windows::fs::MetadataExt;
                        if m.file_attributes()&0x400!=0{
                            observation=PathObservation::Unavailable;break
                        }
                    }
                    if i+1==parts.len()&&m.is_file(){
                        observation=PathObservation::Present;
                        if path.ends_with(".json")&&m.len()<=48*1024{
                            if let Ok(mut f)=dir.open(&portion){
                                let mut data=Vec::new();
                                match std::io::Read::by_ref(&mut f)
                                    .take(48*1024+1).read_to_end(&mut data){
                                    Ok(_) if data.len() as u64==m.len()=>{
                                        observation=if serde_json::from_slice::<
                                            serde_json::Value>(&data).is_ok(){
                                            PathObservation::ValidJson
                                        }else{PathObservation::InvalidJson};
                                    },
                                    _=>observation=PathObservation::Unavailable,
                                }
                            }else{observation=PathObservation::Unavailable}
                        }
                    }
                },
                Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{
                    observation=PathObservation::Absent;break
                },
                Err(e) if e.kind()==std::io::ErrorKind::PermissionDenied=>{
                    observation=PathObservation::PermissionDenied;break
                },
                Err(_)=>{observation=PathObservation::Unavailable;break},
            }
        }
        facts.push(ScopedPathFact{path,observation,
            scope_sha256:scope.clone(),observed_head:g.selected_head.clone()});
    }
    Ok(facts)
}
#[cfg(test)]
mod tests{
    use super::*;
    fn input(target:&str)->eg::EvidenceGraph{
        eg::assemble(eg::GraphInput{
            project_scope_sha256:"f".repeat(64),
            selected_head:Some("a".repeat(40)),observed_at_unix:3,
            complete:true,host_git_checked:true,
            files:vec![
                eg::FileInput{path:"docs/REQUIREMENTS.md".into(),role:"project_document".into(),
                    sha256:Some("b".repeat(64))},
                eg::FileInput{path:target.into(),role:"source_path".into(),
                    sha256:Some("c".repeat(64))}],
            requirements:vec![eg::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:"b".repeat(64),line:2,
                statement:format!("Must implement {target}")}],
            runs:vec![],branches:vec![],extra_limitations:vec![],
        }).unwrap()
    }
    fn fact(g:&eg::EvidenceGraph,p:&str,v:PathObservation)->ScopedPathFact{
        ScopedPathFact{path:p.into(),observation:v,
            scope_sha256:g.project_scope_sha256.clone(),
            observed_head:g.selected_head.clone()}
    }
    #[test]
    fn hashed_source_does_not_prove_acceptance(){
        let g=input("src/core.rs");
        let r=classify(&g,&[fact(&g,"src/core.rs",PathObservation::Present)]).unwrap();
        audit(&r,&g).unwrap();
        assert_eq!(r.findings[0].primary,Status::Unverified);
    }
    #[test]
    fn missing_requires_scoped_negative_probe(){
        let g=input("src/absent.rs");
        assert_ne!(classify(&g,&[]).unwrap().findings[0].primary,Status::Missing);
        let r=classify(&g,&[fact(&g,"src/absent.rs",PathObservation::Absent)]).unwrap();
        assert_eq!(r.findings[0].primary,Status::Missing);
        let mut tampered=r.clone();tampered.findings[0].primary=Status::Accepted;
        assert!(audit(&tampered,&g).is_err());
    }
    #[test]
    fn foreign_proof_denied(){
        let g=input("src/thing.rs");
        let mut x=fact(&g,"src/thing.rs",PathObservation::Present);
        x.scope_sha256="0".repeat(64);
        assert!(classify(&g,&[x]).is_err());
    }

    #[test]
    fn partial_explicit_file_coverage_does_not_mean_behavior_accepted(){
        let mut inputs=eg::GraphInput{
            project_scope_sha256:"f".repeat(64),selected_head:Some("a".repeat(40)),
            observed_at_unix:9,complete:true,host_git_checked:true,
            files:vec![
                eg::FileInput{path:"docs/REQUIREMENTS.md".into(),role:"project_document".into(),sha256:Some("b".repeat(64))},
                eg::FileInput{path:"src/a.rs".into(),role:"source_path".into(),sha256:Some("c".repeat(64))},
            ],
            requirements:vec![eg::RequirementInput{path:"docs/REQUIREMENTS.md".into(),
                sha256:"b".repeat(64),line:2,
                statement:"Must implement src/a.rs and src/b.rs".into()}],
            runs:vec![],branches:vec![],extra_limitations:vec![],
        };
        let g=eg::assemble(inputs).unwrap();
        let r=classify(&g,&[
            fact(&g,"src/a.rs",PathObservation::Present),
            fact(&g,"src/b.rs",PathObservation::Absent),
        ]).unwrap();
        assert_eq!(r.findings[0].primary,Status::Partial);
        assert_eq!(r.findings[0].observed_targets,2);
        assert!(!r.findings[0].dimensions.contains(&Status::Accepted));
    }
    #[test]
    fn separate_branch_is_not_current_head_integration(){
        let head="a".repeat(40);
        let g=eg::assemble(eg::GraphInput{
            project_scope_sha256:"f".repeat(64),selected_head:Some(head),
            observed_at_unix:9,complete:true,host_git_checked:true,
            files:vec![eg::FileInput{path:"docs/REQUIREMENTS.md".into(),
                role:"project_document".into(),sha256:Some("b".repeat(64))}],
            requirements:vec![eg::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:"b".repeat(64),
                line:2,statement:"Must integrate refs/heads/candidate before release".into()}],
            runs:vec![],branches:vec![eg::BranchInput{ref_name:"candidate".into(),
                tip:"c".repeat(40),contained_in_head:false}],
            extra_limitations:vec![],
        }).unwrap();
        let r=classify(&g,&[]).unwrap();
        assert_eq!(r.findings[0].primary,Status::NotIntegrated);
        assert_ne!(r.findings[0].primary,Status::Accepted);
        assert_eq!(r.findings[0].confidence,Confidence::Grounded);
    }
    #[test]
    fn conflicting_same_head_report_claims_are_unknown_not_broken(){
        let head="a".repeat(40);
        let make_run=|result:&str,id:&str|eg::RunInput{
            report_path:format!("reports/{id}.json"),
            report_sha256:if result=="passed"{"c".repeat(64)}else{"d".repeat(64)},
            claimed_head:head.clone(),test_path:"tests/feature.test.js".into(),
            source_path:Some("src/feature.rs".into()),run_id:id.into(),
            outcome:result.into(),artifact_path:None,artifact_sha256:None,
        };
        let g=eg::assemble(eg::GraphInput{
            project_scope_sha256:"f".repeat(64),selected_head:Some(head),
            observed_at_unix:9,complete:true,host_git_checked:true,
            files:vec![
                eg::FileInput{path:"docs/REQUIREMENTS.md".into(),role:"project_document".into(),sha256:Some("b".repeat(64))},
                eg::FileInput{path:"src/feature.rs".into(),role:"source_path".into(),sha256:Some("e".repeat(64))},
                eg::FileInput{path:"tests/feature.test.js".into(),role:"test_path".into(),sha256:Some("f".repeat(64))},
            ],
            requirements:vec![eg::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:"b".repeat(64),
                line:2,statement:"Must test src/feature.rs with tests/feature.test.js".into()}],
            runs:vec![make_run("passed","one"),make_run("failed","two")],
            branches:vec![],extra_limitations:vec![],
        }).unwrap();
        let r=classify(&g,&[]).unwrap();
        assert_eq!(r.findings[0].evidence_health,EvidenceHealth::Conflicting);
        assert!(r.findings[0].conflicting_claims);
        assert_eq!(r.findings[0].primary,Status::Unknown);
        assert!(!r.findings[0].dimensions.contains(&Status::Broken));
    }
    #[test]
    fn stale_pass_cannot_become_accepted(){
        let current="a".repeat(40);
        let g=eg::assemble(eg::GraphInput{
            project_scope_sha256:"f".repeat(64),selected_head:Some(current),
            observed_at_unix:9,complete:true,host_git_checked:true,
            files:vec![
                eg::FileInput{path:"docs/REQUIREMENTS.md".into(),role:"project_document".into(),sha256:Some("b".repeat(64))},
                eg::FileInput{path:"src/feature.rs".into(),role:"source_path".into(),sha256:Some("d".repeat(64))},
                eg::FileInput{path:"tests/feature.test.js".into(),role:"test_path".into(),sha256:Some("e".repeat(64))},
            ],
            requirements:vec![eg::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:"b".repeat(64),
                line:2,statement:"Must test src/feature.rs with tests/feature.test.js".into()}],
            runs:vec![eg::RunInput{
                report_path:"reports/old.json".into(),report_sha256:"f".repeat(64),
                claimed_head:"c".repeat(40),test_path:"tests/feature.test.js".into(),
                source_path:Some("src/feature.rs".into()),run_id:"historical".into(),
                outcome:"passed".into(),artifact_path:None,artifact_sha256:None}],
            branches:vec![],extra_limitations:vec![],
        }).unwrap();
        let r=classify(&g,&[]).unwrap();
        assert_eq!(r.findings[0].evidence_health,EvidenceHealth::Historical);
        assert_ne!(r.findings[0].primary,Status::Accepted);
    }


    #[test]
    fn native_json_and_file_presence_gates_are_strictly_scoped(){
        fn scoped_requirement(text:&str)->eg::EvidenceGraph{
            eg::assemble(eg::GraphInput{
                project_scope_sha256:"f".repeat(64),selected_head:Some("a".repeat(40)),
                observed_at_unix:8,complete:true,host_git_checked:true,
                files:vec![eg::FileInput{path:"docs/REQUIREMENTS.md".into(),
                    role:"project_document".into(),sha256:Some("b".repeat(64))}],
                requirements:vec![eg::RequirementInput{
                    path:"docs/REQUIREMENTS.md".into(),sha256:"b".repeat(64),
                    line:2,statement:text.into()}],
                runs:vec![],branches:vec![],extra_limitations:vec![],
            }).unwrap()
        }
        let json="config/gate.json";
        let g=scoped_requirement("Must validate JSON syntax of config/gate.json");
        let broken=classify(&g,&[fact(&g,json,PathObservation::InvalidJson)]).unwrap();
        assert_eq!(broken.findings[0].primary,Status::Broken);
        let accepted=classify(&g,&[fact(&g,json,PathObservation::ValidJson)]).unwrap();
        assert_eq!(accepted.findings[0].primary,Status::Accepted);
        let claim=classify(&g,&[fact(&g,json,PathObservation::Present)]).unwrap();
        assert_ne!(claim.findings[0].primary,Status::Accepted);
        let g=scoped_requirement("Must include config/gate.json");
        assert_eq!(classify(&g,&[fact(&g,json,PathObservation::Present)])
            .unwrap().findings[0].primary,Status::Accepted);
        assert_eq!(classify(&g,&[fact(&g,json,PathObservation::PermissionDenied)])
            .unwrap().findings[0].primary,Status::Blocked);
        assert_ne!(classify(&g,&[fact(&g,json,PathObservation::Unavailable)])
            .unwrap().findings[0].primary,Status::Blocked);
    }
    #[test]
    fn verified_git_branch_inclusion_only_accepts_exact_stated_criterion(){
        let g=eg::assemble(eg::GraphInput{
            project_scope_sha256:"f".repeat(64),selected_head:Some("a".repeat(40)),
            observed_at_unix:9,complete:true,host_git_checked:true,
            files:vec![eg::FileInput{path:"docs/REQUIREMENTS.md".into(),
                role:"project_document".into(),sha256:Some("b".repeat(64))}],
            requirements:vec![eg::RequirementInput{
                path:"docs/REQUIREMENTS.md".into(),sha256:"b".repeat(64),line:2,
                statement:"Must contain refs/heads/feature in selected HEAD".into()}],
            branches:vec![eg::BranchInput{ref_name:"feature".into(),
                tip:"a".repeat(40),contained_in_head:true}],
            runs:vec![],extra_limitations:vec![],
        }).unwrap();
        let r=classify(&g,&[]).unwrap();
        assert_eq!(r.findings[0].primary,Status::Accepted);
        assert!(r.findings[0].explanation.contains("branch-inclusion"));
    }

    #[test]
    fn explicit_targets_are_not_unbounded_semantic_search(){
        assert!(explicit_targets("Make a good editor").is_empty());
        assert_eq!(explicit_targets("Need src/model.rs, plus tests/model.test.js"),
            vec!["src/model.rs".to_string(),"tests/model.test.js".to_string()]);
        assert!(explicit_targets("Do ../outside.rs").is_empty());
    }
}
