//! M4-P04 actual ANSF-source claim assessments for a REAL local Qwen GGUF run.
//! The model is an external decoder; this example manufactures NO answers,
//! only original ANSF user entries and the production source-linked projection.
use lu_core::{
    ansf::{AnsfStore,EntryRole},
    ansf_belief_revision::{Claim,ClaimState},
    ansf_source_ids::AnsfSourceId,
    continuity::ContinuityStore,journal::EffectJournal
};
use serde_json::{json,Value};
use std::{path::PathBuf,time::{SystemTime,UNIX_EPOCH},fs};

fn at()->u64{SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()}
fn claim(id:&str,source:i64,value:&str,subject:&str,version:&str)->Claim{
    Claim{claim_id:id.into(),session_id:"c-local-benchmark".into(),
        source_entry_id:source,project_id:"lu".into(),task_id:"m4".into(),
        subject_key:subject.into(),predicate_key:"backend".into(),
        value:value.into(),application_version:version.into(),
        expires_unix:at()+3600,supersedes_id:None}
}
fn classification(s:ClaimState)->&'static str{
    match s{
        ClaimState::Reported=>"REPORTED_UNVERIFIED",
        ClaimState::IndependentlyVerified=>"INDEPENDENTLY_VERIFIED",
        ClaimState::Contradicted=>"CONTRADICTED",
        ClaimState::Stale=>"STALE",
        ClaimState::Retracted=>"RETRACTED",
        ClaimState::Superseded=>"SUPERSEDED",
    }
}
fn evidence(book:&AnsfStore,continuity:&ContinuityStore,journal:&EffectJournal,
    subject:&str,version:&str)->Vec<Value>{
    book.assess_claims("c-local-benchmark","lu","m4",subject,"backend",
       version,continuity,journal,|_|false
    ).unwrap().into_iter().map(|x|{
        json!({"originalSourceId":AnsfSourceId::new(
            x.claim.session_id,x.claim.source_entry_id).unwrap().encode(),
            "assertion":x.claim.value,"state":classification(x.state),
            "sourceSha256":x.source_sha256,
            "noPermissions":true})
    }).collect()
}
fn main(){
    let root=PathBuf::from(std::env::args().nth(1).expect("one root directory argument"));
    if root.exists(){panic!("Refusing to overwrite existing original ANSF fixture")}
    fs::create_dir_all(&root).unwrap();
    let bookpath=root.join("originals.sqlite");
    let book=AnsfStore::open(&bookpath).unwrap();
    book.create_session("c-local-benchmark","ws-benchmark","M4 source provenance").unwrap();
    book.start_chapter("c-local-benchmark","Actual reported source events").unwrap();
    let first=book.append_entry("c-local-benchmark","observed-one",
        EntryRole::User,"Reported","Model runtime was described as Vulkan.").unwrap();
    let second=book.append_entry("c-local-benchmark","observed-two",
        EntryRole::User,"Conflicting report","Another report claimed ROCm.").unwrap();
    let third=book.append_entry("c-local-benchmark","observed-three",
        EntryRole::User,"Versioned report","Historical app v1 advertised CPU.").unwrap();
    let correction=book.append_entry("c-local-benchmark","observed-four",
        EntryRole::User,"Correction","The old backend report is withdrawn.").unwrap();
    let continuity=ContinuityStore::open(&root.join("continuity.sqlite")).unwrap();
    let journal=EffectJournal::open(&root.join("effects.sqlite")).unwrap();
    let a=claim("c-one",first,"Vulkan","current","2.0");
    let b=claim("c-two",second,"ROCm","current","2.0");
    let c=claim("c-three",third,"CPU","previous","1.0");
    book.record_reported_claim(&a).unwrap();
    book.record_reported_claim(&c).unwrap();
    let mut cases=Vec::new();
    let mut add=|id:&str,subject:&str,version:&str,expected:&str|{
        cases.push(json!({"case":id,"subject":subject,"version":version,
            "expected":expected,"claims":evidence(&book,&continuity,&journal,
                subject,version)}))
    };
    add("reported_only","current","2.0","UNVERIFIED");
    add("version_expired","previous","2.0","STALE");
    drop(add);
    book.record_reported_claim(&b).unwrap();
    cases.push(json!({"case":"contradictory","subject":"current","version":"2.0",
        "expected":"CONFLICT","claims":evidence(&book,&continuity,&journal,"current","2.0")}));
    book.retract_reported_or_verified_claim(
        "c-two","c-local-benchmark",correction,"User withdrew conflicting report"
    ).unwrap();
    cases.push(json!({"case":"corrected","subject":"current","version":"2.0",
        "expected":"UNVERIFIED","claims":evidence(&book,&continuity,&journal,"current","2.0")}));
    book.retract_reported_or_verified_claim(
        "c-one","c-local-benchmark",correction,"User withdrew remaining report"
    ).unwrap();
    cases.push(json!({"case":"retracted","subject":"current","version":"2.0",
        "expected":"RETRACTED","claims":evidence(&book,&continuity,&journal,"current","2.0")}));
    drop(book);
    let reopened=AnsfStore::open(&bookpath).unwrap();
    cases.push(json!({"case":"restart_reopened","subject":"current","version":"2.0",
        "expected":"RETRACTED","claims":evidence(&reopened,&continuity,&journal,"current","2.0")}));
    println!("{}",json!({"schema":"lu-m4-real-model-v1","cases":cases,
        "source":"real-original-ansf-sqlite", "toolPermissions":false}));
}
