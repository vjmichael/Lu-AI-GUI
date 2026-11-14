//! E1-P09: read-only, host-authenticated reviewed-app effect evidence.
//!
//! A model's final answer or tool-feedback audit is NOT an OS completion
//! receipt. This projection counts durable APPLIED / UNCERTAIN / FAILED
//! effects without returning a command, pathname, raw worker output,
//! Protobuf payload, fingerprint or journal-opaque worker call ID.
//!
//! Only the trusted host chooses the app-data root. An opaque host turn
//! ID can locate its OWN ledger, never select an arbitrary filesystem path.
//! This module issues no commands, permissions, tools or retries.
use std::{fs,path::{Path,PathBuf}};
use rusqlite::{Connection,OpenFlags};
use serde::Serialize;

const MAX_LEDGER_BYTES:u64=64*1024*1024;
const MAX_EFFECTS:u32=512;

#[derive(Clone,Debug,Default,PartialEq,Eq,Serialize)]
#[serde(rename_all="camelCase")]
pub struct ReviewedEffectCounts{
    /// A durable effect in APPLIED state, not necessarily current OS state.
    pub applied:u32,
    pub uncertain:u32,
    pub failed:u32,
    pub executing:u32,
    pub prepared:u32,
    /// APPLIED records with a nonempty retained journal receipt.
    /// A receipt does not prove an old worker is still gone after restart.
    pub applied_with_receipt:u32,
    /// E1-P10: distinct host factual observations, not worker self-reports.
    pub independently_verified:u32,
    pub outcome_unsupported:u32,
    pub outcome_conflict:u32,
}
impl ReviewedEffectCounts{
    pub fn total(&self)->u32{
        self.applied+self.uncertain+self.failed+self.executing+self.prepared
    }
    pub fn unresolved(&self)->bool{
        self.uncertain>0||self.executing>0||self.prepared>0
    }
}
/// Strict display classification: a model answer never upgrades an
/// unresolved, failed or missing OS effect journal to successful execution.
pub fn classify_reviewed_evidence(
    final_answers:u32,generation_failures:u32,rejections:u32,
    counts:Option<&ReviewedEffectCounts>
)->&'static str{
    if counts.is_some_and(ReviewedEffectCounts::unresolved){
        return "EFFECT_UNCERTAIN_OR_IN_FLIGHT"
    }
    if generation_failures>0{return "MODEL_FAILED"}
    if counts.is_some_and(|v|v.failed>0){return "EFFECT_FAILED"}
    if counts.is_some_and(|v|v.outcome_conflict>0){
        return "OUTCOME_CONFLICT"
    }
    if rejections>0{return "REJECTED"}
    if counts.is_some_and(|v|v.applied>v.applied_with_receipt){
        return "APPLIED_RECEIPT_MISSING"
    }
    if final_answers>0&&counts.is_some_and(|v|v.applied>0
        &&v.applied==v.independently_verified){
        return "INDEPENDENTLY_VERIFIED_AT_OBSERVATION"
    }
    if final_answers>0&&counts.is_some_and(|v|v.applied>0){
        return "ANSWER_RECORDED_WITH_APPLIED_JOURNAL"
    }
    if final_answers>0{return "MODEL_ANSWER_ONLY"}
    "INCOMPLETE_OR_INTERRUPTED"
}

fn valid_turn_id(value:&str)->bool{
    let bytes=value.as_bytes();
    bytes.len()==34&&bytes.starts_with(b"g-")&&bytes[2..]
        .iter().all(|b|b.is_ascii_digit()||(*b>=b'a'&&*b<=b'f'))
}
fn no_linked_file(path:&Path)->Result<Option<fs::Metadata>,String>{
    let meta=match fs::symlink_metadata(path){
        Ok(meta)=>meta,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(None),
        Err(_)=>return Err("Reviewed evidence inaccessible".into())
    };
    if !meta.is_file()||meta.file_type().is_symlink(){
        return Err("Reviewed evidence file is not an ordinary file".into())
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{
            return Err("Reviewed evidence reparse file refused".into())
        }
    }
    Ok(Some(meta))
}
fn check_sidecars(path:&Path)->Result<(),String>{
    // SQLite WAL and SHM files are separate namespace entries. Refuse linked
    // versions of either instead of trusting an attacker-controlled sidecar.
    let raw=path.as_os_str();
    for suffix in ["-wal","-shm"]{
        let mut name=raw.to_os_string();
        name.push(suffix);
        let _=no_linked_file(&PathBuf::from(name))?;
    }
    Ok(())
}

/// No missing-ledger fabrication: None means no effect database exists.
/// Always filter by a separately authenticated user turn in the caller.
pub fn read_reviewed_effect_counts(
    profile_dir:&Path,host_turn_id:&str
)->Result<Option<ReviewedEffectCounts>,String>{
    if !valid_turn_id(host_turn_id){
        return Err("Invalid reviewed evidence turn identity".into())
    }
    let file=profile_dir.join(format!("lu-reviewed-effects-{host_turn_id}.sqlite"));
    let Some(meta)=no_linked_file(&file)? else{return Ok(None)};
    if meta.len()>MAX_LEDGER_BYTES{
        return Err("Reviewed evidence exceeds bounded journal size".into())
    }
    check_sidecars(&file)?;
    let flags=OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection=Connection::open_with_flags(&file,flags)
        .map_err(|_|"Reviewed effect evidence database unavailable")?;
    let mut statement=connection.prepare(
        "SELECT state,COUNT(*),
           COALESCE(SUM(CASE WHEN receipt IS NOT NULL
                  AND LENGTH(receipt)>0 THEN 1 ELSE 0 END),0)
         FROM lu_effects
         GROUP BY state"
    ).map_err(|_|"Reviewed effect evidence schema unavailable")?;
    let rows=statement.query_map([],|row|Ok((
        row.get::<_,String>(0)?,
        row.get::<_,i64>(1)?,
        row.get::<_,i64>(2)?
    ))).map_err(|_|"Reviewed effect evidence query failed")?;
    let mut counts=ReviewedEffectCounts::default();
    for row in rows{
        let (state,total,receipts)=row
            .map_err(|_|"Reviewed effect evidence damaged")?;
        let value=u32::try_from(total)
            .map_err(|_|"Reviewed effect count invalid")?;
        let nonempty=u32::try_from(receipts)
            .map_err(|_|"Reviewed effect receipt count invalid")?;
        if value>MAX_EFFECTS||nonempty>value
            ||counts.total().saturating_add(value)>MAX_EFFECTS{
            return Err("Reviewed effect evidence exceeds count limit".into())
        }
        match state.as_str(){
            "APPLIED"=>{counts.applied=value;counts.applied_with_receipt=nonempty;},
            "UNCERTAIN"=>counts.uncertain=value,
            "FAILED"=>counts.failed=value,
            "EXECUTING"=>counts.executing=value,
            "PREPARED"=>counts.prepared=value,
            _=>return Err("Invalid effect state in reviewed journal".into())
        }
    }
    // Legacy E1-P08/P09 effect databases can predate this separate
    // verifier table. Missing table means NO independent verification.
    let exists:i64=connection.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table'
         AND name='lu_independent_outcomes'",
        [],|row|row.get(0)
    ).map_err(|_|"Outcome evidence schema unavailable")?;
    if exists==1{
        let mut q=connection.prepare(
            "SELECT o.verdict,COUNT(*) FROM lu_independent_outcomes o
             JOIN lu_effects e ON e.call_id=o.call_id
             WHERE e.state='APPLIED' AND e.receipt IS NOT NULL
             GROUP BY o.verdict"
        ).map_err(|_|"Outcome journal query unavailable")?;
        let rows=q.query_map([],|row|Ok((
            row.get::<_,String>(0)?,row.get::<_,i64>(1)?
        ))).map_err(|_|"Outcome journal query failed")?;
        for row in rows{
            let (verdict,n)=row.map_err(|_|"Outcome journal row invalid")?;
            let n=u32::try_from(n).map_err(|_|"Outcome count overflow")?;
            if n>MAX_EFFECTS{return Err("Outcome count exceeded limit".into())}
            match verdict.as_str(){
                "VERIFIED"=>counts.independently_verified=n,
                "UNSUPPORTED"=>counts.outcome_unsupported=n,
                "CONFLICT"=>counts.outcome_conflict=n,
                _=>return Err("Unexpected independent outcome state".into())
            }
        }
        if counts.independently_verified
            .saturating_add(counts.outcome_unsupported)
            .saturating_add(counts.outcome_conflict)>counts.applied{
            return Err("Outcome count exceeds applied effects".into())
        }
    }
    Ok(Some(counts))
}

#[cfg(test)]
mod tests{
    use super::*;
    use lu_core::{
        contracts::{ProtoPayload,ToolInvocation},
        journal::{EffectJournal,PrepareOutcome}
    };
    use std::time::{SystemTime,UNIX_EPOCH};
    fn root()->PathBuf{
        std::env::temp_dir().join(format!(
            "lu-e1-p09-reviewed-evidence-{}-{}",
            std::process::id(),SystemTime::now()
                .duration_since(UNIX_EPOCH).unwrap().as_nanos()
        ))
    }
    fn invocation(call:&str)->ToolInvocation{
        ToolInvocation{
            call_id:call.into(),tool_id:"lu.reviewed.git.head".into(),
            workspace_id:Some("w-reviewed".into()),
            input:ProtoPayload{type_url:"lu.e1.request".into(),bytes:vec![1]}
        }
    }
    #[test]
    fn actual_sqlite_effect_states_are_counted_but_payloads_never_returned(){
        let root=root();fs::create_dir_all(&root).unwrap();
        let id=format!("g-{:032x}",15);
        let db=root.join(format!("lu-reviewed-effects-{id}.sqlite"));
        assert_eq!(read_reviewed_effect_counts(&root,&id).unwrap(),None);
        let effects=EffectJournal::open(&db).unwrap();
        assert_eq!(effects.prepare(&invocation("call-applied")).unwrap(),PrepareOutcome::New);
        effects.mark_executing("call-applied").unwrap();
        effects.mark_applied("call-applied",b"SECRET-RAW-RECEIPT").unwrap();
        effects.prepare(&invocation("call-uncertain")).unwrap();
        effects.mark_executing("call-uncertain").unwrap();
        effects.mark_uncertain("call-uncertain").unwrap();
        effects.prepare(&invocation("call-prepared")).unwrap();
        let observed=read_reviewed_effect_counts(&root,&id).unwrap().unwrap();
        assert_eq!(observed.applied,1);
        assert_eq!(observed.applied_with_receipt,1);
        assert_eq!(observed.uncertain,1);
        assert_eq!(observed.prepared,1);
        assert_eq!(observed.total(),3);
        assert!(observed.unresolved());
        let displayed=serde_json::to_string(&observed).unwrap();
        assert!(!displayed.contains("SECRET"));
        assert!(!displayed.contains("call-"));
        assert!(!displayed.contains("w-reviewed"));
        drop(effects);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn model_answer_never_overrides_failed_uncertain_or_missing_native_receipt(){
        let mut counts=ReviewedEffectCounts{applied:1,applied_with_receipt:1,
            ..ReviewedEffectCounts::default()};
        assert_eq!(classify_reviewed_evidence(1,0,0,None),"MODEL_ANSWER_ONLY");
        assert_eq!(classify_reviewed_evidence(1,0,0,Some(&counts)),
            "ANSWER_RECORDED_WITH_APPLIED_JOURNAL");
        counts.uncertain=1;
        assert_eq!(classify_reviewed_evidence(1,0,0,Some(&counts)),
            "EFFECT_UNCERTAIN_OR_IN_FLIGHT");
        counts.uncertain=0;
        counts.failed=1;
        assert_eq!(classify_reviewed_evidence(1,0,0,Some(&counts)),"EFFECT_FAILED");
        counts.failed=0;
        counts.applied_with_receipt=0;
        assert_eq!(classify_reviewed_evidence(1,0,0,Some(&counts)),
            "APPLIED_RECEIPT_MISSING");
        counts.applied_with_receipt=1;
        assert_eq!(classify_reviewed_evidence(1,1,0,Some(&counts)),"MODEL_FAILED");
        assert_eq!(classify_reviewed_evidence(1,0,1,Some(&counts)),"REJECTED");
    }
    #[test]
    fn independent_outcome_is_distinct_from_worker_applied_receipt(){
        use lu_core::journal::IndependentOutcomeVerdict;
        use sha2::{Digest,Sha256};
        let root=root();fs::create_dir_all(&root).unwrap();
        let id=format!("g-{:032x}",45);
        let db=root.join(format!("lu-reviewed-effects-{id}.sqlite"));
        let journal=EffectJournal::open(&db).unwrap();
        for call in ["verified","unsupported","conflict"]{
            journal.prepare(&invocation(call)).unwrap();
            journal.mark_executing(call).unwrap();
            journal.mark_applied(call,b"OS-receipt").unwrap();
        }
        let observed=format!("{:x}",Sha256::digest(b"facts"));
        journal.record_independent_outcome("verified",b"OS-receipt",
            IndependentOutcomeVerdict::Verified,"GIT_HEAD_DIRECT_V1",
            Some(&observed)).unwrap();
        journal.record_independent_outcome("unsupported",b"OS-receipt",
            IndependentOutcomeVerdict::Unsupported,
            "GIT_STATUS_UNSUPPORTED_V1",None).unwrap();
        journal.record_independent_outcome("conflict",b"OS-receipt",
            IndependentOutcomeVerdict::Conflict,
            "GIT_BRANCH_DIRECT_V1",None).unwrap();
        let view=read_reviewed_effect_counts(&root,&id).unwrap().unwrap();
        assert_eq!(view.applied,3);
        assert_eq!(view.independently_verified,1);
        assert_eq!(view.outcome_unsupported,1);
        assert_eq!(view.outcome_conflict,1);
        assert_eq!(classify_reviewed_evidence(1,0,0,Some(&view)),
            "OUTCOME_CONFLICT");
        let serialized=serde_json::to_string(&view).unwrap();
        assert!(!serialized.contains("OS-receipt"));
        assert!(!serialized.contains("verified\""));
        drop(journal);fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_non_host_ids_and_never_looks_at_arbitrary_paths(){
        let root=root();fs::create_dir_all(&root).unwrap();
        for id in ["../etc/passwd","g-deadbeef","g-FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "g-0000000000000000000000000000000/","g-0000000000000000000000000000000x"]{
            assert!(read_reviewed_effect_counts(&root,id).is_err(),"{id}");
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[cfg(unix)]
    fn linked_journal_is_rejected_even_when_target_is_real(){
        use std::os::unix::fs::symlink;
        let root=root();fs::create_dir_all(&root).unwrap();
        let id=format!("g-{:032x}",3);
        let target=root.join("innocent.bin");
        fs::write(&target,b"private").unwrap();
        symlink(&target,root.join(format!("lu-reviewed-effects-{id}.sqlite"))).unwrap();
        assert!(read_reviewed_effect_counts(&root,&id).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
