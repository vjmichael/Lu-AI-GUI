//! FIN-04W2-C: verify the ORIGINAL one-use supervisor grant journal.
//!
//! A syntactically valid named-pipe command, client PID, Job membership,
//! preinstalled plan or admin-supplied string is never creator consent.
//! The original in-memory grant is issued only by Supervisor::record_human_approval,
//! consumed by native host admission, and cannot survive a process restart.
//! This module independently replays the ORIGINAL fsynced journal and denies
//! everything except a single fresh Authorized -> AdmissionPending transition.
//! The journal is not signed: its authenticity relies on OS-protected origin
//! AND independently pinned/attested host process; hash chaining alone does
//! not establish identity. Service IPC must check both before this verifier.
use crate::{JobPhase, JobPlan, Receipt, SupervisorError};
use sha2::{Digest, Sha256};
use std::{fs::{self, OpenOptions}, io::Write, path::Path, time::{SystemTime, UNIX_EPOCH}};

const MAX_AGE_MS: u64 = 60_000;
fn denied() -> SupervisorError { SupervisorError::InvalidApproval }

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConsumedOriginalApproval {
    pub(crate) job_id: String,
    pub(crate) plan_sha256: String,
    pub(crate) admission_entry_sha256: String,
    pub(crate) session_id: String,
}
fn valid_job(value: &str) -> bool {
    value.len() == 36
        && value.starts_with("job-")
        && value[4..].bytes().all(|b| b.is_ascii_hexdigit())
}
fn now_ms() -> Result<u64, SupervisorError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)
        .map_err(|_|denied())?.as_millis().try_into().map_err(|_|denied())?)
}

/// A verified chain is necessary but NOT proof that a user confirmed anything:
/// only the independently pinned host's original journal with exact ACLs is
/// eligible. Never accept arbitrary serialized receipts from a pipe request.
pub(crate) fn verify_consumed_original_approval(
    receipts: &[Receipt], job_id: &str, plan: &JobPlan,
    expected_plan_sha256: &str,
    now: u64,
) -> Result<ConsumedOriginalApproval, SupervisorError> {
    if !valid_job(job_id) || now == 0
        || !crate::validate_plan(plan)
        || plan.recipe_id != "dap_lldb_project"
        || plan.effect_class != "debug_inspection_isolated"
        || expected_plan_sha256 != crate::fingerprint(plan)?
    { return Err(denied()); }
    // Full replay rejects a forged/missing/stale/duplicate event, altered
    // hash chain, invalid transition or incorrectly scoped job.
    let store = crate::Store::replay(receipts.to_vec())?;
    if store.quarantine.iter().any(|other| other != job_id) {
        return Err(denied());
    }
    let state = store.jobs.get(job_id).ok_or_else(denied)?;
    if state.phase != JobPhase::AdmissionPending
        || state.plan_hash != expected_plan_sha256
        || state.plan.project_id != plan.project_id
    { return Err(denied()); }
    let original: Vec<&Receipt> = receipts.iter()
        .filter(|x| x.body.job_id == job_id).collect();
    if original.len() != 3 { return Err(denied()); }
    let prepared = original[0];
    let approved = original[1];
    let consumed = original[2];
    if prepared.body.phase != JobPhase::Prepared
        || prepared.body.outcome_code != "host_plan_prepared"
        || approved.body.phase != JobPhase::Authorized
        || approved.body.outcome_code != "fresh_human_approval"
        || consumed.body.phase != JobPhase::AdmissionPending
        || consumed.body.outcome_code != "grant_consumed_pending_os_admission"
        || consumed.body.evidence_code != "no_worker_yet_started"
        || consumed.body.session_id.len() != 40
        || !consumed.body.session_id.starts_with("session-")
        || !consumed.body.session_id[8..].bytes().all(|b|b.is_ascii_hexdigit())
        || prepared.body.session_id != approved.body.session_id
        || approved.body.session_id != consumed.body.session_id
        || prepared.body.project_id != plan.project_id
        || approved.body.project_id != plan.project_id
        || consumed.body.project_id != plan.project_id
        || original.iter().any(|r| r.body.plan_sha256 != expected_plan_sha256)
        || approved.body.observed_unix_ms > consumed.body.observed_unix_ms
        || consumed.body.observed_unix_ms > now
        || now.saturating_sub(approved.body.observed_unix_ms) > MAX_AGE_MS
        || consumed.entry_hash.len() != 64
    { return Err(denied()); }
    Ok(ConsumedOriginalApproval {
        job_id: job_id.to_owned(),
        plan_sha256: expected_plan_sha256.to_owned(),
        admission_entry_sha256: consumed.entry_hash.clone(),
        session_id: consumed.body.session_id.clone(),
    })
}

/// An Authorized receipt is not by itself executable authority. It
/// allows the separately authenticated, fixed service to PREPARE a child
/// suspended in an OS Job; only the original in-memory grant, consumed by
/// the original host AFTER native custody is proven, can authorize resume.
pub(crate) fn verify_fresh_original_authorized(
    receipts:&[Receipt],job_id:&str,plan:&JobPlan,sha:&str,now:u64,
)->Result<(),SupervisorError>{
    if !valid_job(job_id) || now==0 || !crate::validate_plan(plan)
        || plan.recipe_id!="dap_lldb_project"
        || plan.effect_class!="debug_inspection_isolated"
        || sha!=crate::fingerprint(plan)? {return Err(denied());}
    let store=crate::Store::replay(receipts.to_vec())?;
    if !store.quarantine.is_empty(){return Err(denied());}
    let job=store.jobs.get(job_id).ok_or_else(denied)?;
    if job.phase!=JobPhase::Authorized || job.plan_hash!=sha
        || job.plan.project_id!=plan.project_id {return Err(denied());}
    let current:Vec<&Receipt>=receipts.iter().filter(|x|x.body.job_id==job_id).collect();
    if current.len()!=2 {return Err(denied());}
    if current[0].body.phase!=JobPhase::Prepared
        || current[0].body.outcome_code!="host_plan_prepared"
        || current[1].body.phase!=JobPhase::Authorized
        || current[1].body.outcome_code!="fresh_human_approval"
        || current[0].body.session_id!=current[1].body.session_id
        || current.iter().any(|x|x.body.plan_sha256!=sha
            || x.body.project_id!=plan.project_id)
        || current[1].body.observed_unix_ms<current[0].body.observed_unix_ms
        || current[1].body.observed_unix_ms>now
        || now.saturating_sub(current[1].body.observed_unix_ms)>MAX_AGE_MS {
        return Err(denied());
    }
    Ok(())
}
pub(crate) fn verify_original_authorized_file(
    path:&Path,job_id:&str,plan:&JobPlan,sha:&str,
)->Result<(),SupervisorError>{
    let rows=original_receipts_from_path(path)?;
    verify_fresh_original_authorized(&rows,job_id,plan,sha,now_ms()?)
}
/// Original host receives S only after the service has already inserted a
/// suspended worker into the same Job it will retain through release. On G,
/// the service independently verifies the original native journal contains
/// both fsynced AdmissionPending and Running for this exact Job/session,
/// preserving the matching original consumed grant. On R, it requires a
/// further original ReleasePending for the exact same sequence.
pub(crate) fn verify_original_execution_phase(
    path:&Path,job_id:&str,plan:&JobPlan,sha:&str,
    expected:JobPhase,
)->Result<(),SupervisorError>{
    let rows=original_receipts_from_path(path)?;
    let store=crate::Store::replay(rows.clone())?;
    let job=store.jobs.get(job_id).ok_or_else(denied)?;
    if job.phase!=expected || job.plan_hash!=sha
        || !matches!(expected,JobPhase::Running|JobPhase::ReleasePending)
        || store.quarantine.iter().any(|x|x!=job_id)
    {return Err(denied());}
    let mine:Vec<&Receipt>=rows.iter().filter(|x|x.body.job_id==job_id).collect();
    let needed=if expected==JobPhase::Running{4}else{5};
    if mine.len()!=needed {return Err(denied());}
    let consumed=verify_consumed_original_approval(
        &rows[..rows.iter().position(|x|x.body.job_id==job_id
            && x.body.phase==JobPhase::Running).ok_or_else(denied)?],
        job_id,plan,sha,now_ms()?,
    )?;
    if consumed.job_id!=job_id || consumed.plan_sha256!=sha {
        return Err(denied());
    }
    if mine[3].body.phase!=JobPhase::Running
        || mine[3].body.outcome_code!="native_contained_worker_started"
        || mine[3].body.evidence_code!="os_admission_and_input_seal_checked"
        || mine[3].body.session_id!=consumed.session_id
    {return Err(denied());}
    if expected==JobPhase::ReleasePending
        && (mine[4].body.phase!=JobPhase::ReleasePending
            || mine[4].body.outcome_code!="native_tree_empty_release_pending"
            || mine[4].body.evidence_code!="independent_service_ack_required"
            || mine[4].body.session_id!=consumed.session_id)
    {return Err(denied());}
    Ok(())
}

fn original_receipts_from_path(path:&Path)->Result<Vec<Receipt>,SupervisorError>{
    if !path.is_absolute(){return Err(denied());}
    let meta=fs::symlink_metadata(path).map_err(|_|denied())?;
    if !meta.is_file() || meta.file_type().is_symlink()
        || meta.len()==0 || meta.len()>2*1024*1024 {
        return Err(denied());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {return Err(denied());}
    }
    crate::read_journal(&path.to_path_buf())
}

/// The service independently reads a fixed installer-pinned journal path.
/// OS ACL, location, host PID and image SHA are checked by service caller;
/// this routine never accepts a journal or a path from a named-pipe frame.
pub(crate) fn verify_original_journal_file(
    path: &Path, job_id: &str, plan: &JobPlan,
    expected_plan_sha256: &str,
) -> Result<ConsumedOriginalApproval, SupervisorError> {
    if !path.is_absolute() { return Err(denied()); }
    let metadata = fs::symlink_metadata(path).map_err(|_|denied())?;
    if !metadata.is_file() || metadata.file_type().is_symlink()
        || metadata.len() == 0 || metadata.len() > 2 * 1024 * 1024
    { return Err(denied()); }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 { return Err(denied()); }
    }
    let receipts = crate::read_journal(&path.to_path_buf())?;
    verify_consumed_original_approval(
        &receipts, job_id, plan, expected_plan_sha256, now_ms()?,
    )
}

/// Durable one-use SERVICE-side denial of replay (including restart).
/// The root is the original independently ACL-verified ProgramData folder,
/// never a project/host-selected location or symlink. Persist *before* any
/// suspended native child is created. A crash after this point denies replay
/// and requires recovery; it cannot silently execute twice.
#[cfg(windows)]
pub(crate) fn consume_once_in_private_service_root(
    private_root: &Path,
    proof: &ConsumedOriginalApproval,
) -> Result<(), SupervisorError> {
    let root_meta=fs::symlink_metadata(private_root).map_err(|_|denied())?;
    if !root_meta.is_dir() || root_meta.file_type().is_symlink() {
        return Err(denied());
    }
    use std::os::windows::fs::MetadataExt;
    if root_meta.file_attributes() & 0x400 != 0 {
        return Err(denied());
    }
    if !private_root.is_absolute()
        || !valid_job(&proof.job_id)
        || proof.plan_sha256.len() != 64
        || proof.admission_entry_sha256.len() != 64
        || !proof.plan_sha256.bytes().all(|b|b.is_ascii_hexdigit())
        || !proof.admission_entry_sha256.bytes().all(|b|b.is_ascii_hexdigit())
    { return Err(denied()); }
    let mut digest=Sha256::new();
    digest.update(proof.job_id.as_bytes());
    digest.update(proof.plan_sha256.as_bytes());
    // One admitted Job may never execute twice, even if a previously
    // consumed host journal is rewritten with a different admission receipt.
    // The entry SHA is stored as evidence, NOT used to mint a new filename.
    let filename=format!("fin04w2-consumed-{:x}.receipt",digest.finalize());
    let destination=private_root.join(filename);
    // create_new is atomic; stale partial files MUST remain blockers.
    let mut file=OpenOptions::new().write(true).create_new(true)
        .open(destination).map_err(|_|denied())?;
    let text=format!("fin04w2-original-consumed-v1\n{}\n{}\n{}\n{}\n",
        proof.job_id, proof.plan_sha256,
        proof.admission_entry_sha256, proof.session_id);
    file.write_all(text.as_bytes()).map_err(|_|denied())?;
    file.sync_all().map_err(|_|denied())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn project_plan() -> JobPlan {
        JobPlan {
            project_id:format!("ide-{}","a".repeat(32)),
            recipe_id:"dap_lldb_project".into(),
            source_head:"b".repeat(40),
            input_snapshot_sha256:"c".repeat(64),
            manifest_sha256:"d".repeat(64),
            toolchain_sha256:"e".repeat(64),
            debug_target_binding_sha256:Some("f".repeat(64)),
            runtime_limit_ms:90_000,memory_limit_bytes:512*1024*1024,
            process_limit:16,log_limit_bytes:65536,
            effect_class:"debug_inspection_isolated".into(),
        }
    }
    #[test]
    fn original_supervisor_one_use_grant_replay_stale_and_tampering() {
        use crate::{Supervisor,OsContainmentProof};
        // A unit-level stand-in for the independently verified native
        // OS worker proof. This test does NOT claim to test Windows kernel
        // admission or genuine interactive human UI confirmation.
        let os=OsContainmentProof {
            platform:"windows".into(),
            restricted_identity:true,
            filesystem_access_limited:true,network_access_limited:true,
            child_contained_before_resume:true,process_tree_kill_verified:true,
            memory_quota:true,cpu_quota:true,pid_quota:true,
            protected_journal_broker:true,
        };
        let source=std::env::temp_dir().join(format!(
            "fin04w2-original-{:016x}.jsonl",rand::random::<u64>()
        ));
        let plan=project_plan();
        let sha=crate::fingerprint(&plan).unwrap();
        let supervisor=Supervisor::open(source.clone()).unwrap();
        let job=supervisor.prepare(plan.clone()).unwrap();
        let pending=supervisor.receipt_history(&job.job_id).unwrap();
        assert!(verify_consumed_original_approval(
            &pending,&job.job_id,&plan,&sha,now_ms().unwrap()
        ).is_err());
        supervisor.record_human_approval(
            &job.job_id,&sha,&plan.source_head,true,
        ).unwrap(); // isolated fixture-only simulated GUI approval
        let approved=supervisor.receipt_history(&job.job_id).unwrap();
        assert!(verify_consumed_original_approval(
            &approved,&job.job_id,&plan,&sha,now_ms().unwrap()
        ).is_err());
        supervisor.consume_grant_after_attestation(
            &job.job_id,&plan,&os,
        ).unwrap(); // test-only OS proof, NOT a real service receipt
        assert!(supervisor.consume_grant_after_attestation(
            &job.job_id,&plan,&os,
        ).is_err());
        let receipts=supervisor.receipt_history(&job.job_id).unwrap();
        let witnessed=verify_consumed_original_approval(
            &receipts,&job.job_id,&plan,&sha,now_ms().unwrap()
        ).unwrap();
        assert_eq!(witnessed.admission_entry_sha256,receipts[2].entry_hash);
        assert!(verify_original_journal_file(
            &source,&job.job_id,&plan,&sha,
        ).is_ok());
        assert!(verify_consumed_original_approval(
            &receipts,&job.job_id,&plan,&sha,
            receipts[1].body.observed_unix_ms + MAX_AGE_MS + 1,
        ).is_err());
        let mut corrupted=receipts.clone();
        corrupted[1].body.outcome_code="frontend_click_claimed".into();
        assert!(verify_consumed_original_approval(
            &corrupted,&job.job_id,&plan,&sha,now_ms().unwrap()
        ).is_err());
        let other=format!("job-{}","e".repeat(32));
        assert!(verify_consumed_original_approval(
            &receipts,&other,&plan,&sha,now_ms().unwrap()
        ).is_err());
        supervisor.native_mark_running(&job.job_id).unwrap();
        assert!(verify_original_journal_file(
            &source,&job.job_id,&plan,&sha,
        ).is_err());
        let _=std::fs::remove_file(&source);
    }

    #[cfg(windows)]
    #[test]
    fn service_one_use_replay_barrier_survives_reopen() {
        let root=std::env::temp_dir().join(format!(
            "fin04w2-barrier-{:016x}",rand::random::<u64>()
        ));
        std::fs::create_dir(&root).unwrap();
        let proof=ConsumedOriginalApproval {
            job_id:format!("job-{}","c".repeat(32)),
            plan_sha256:"d".repeat(64),
            admission_entry_sha256:"e".repeat(64),
            session_id:"session-ci".into(),
        };
        consume_once_in_private_service_root(&root,&proof).unwrap();
        assert!(consume_once_in_private_service_root(&root,&proof).is_err());
        let mut rewritten=proof.clone();
        rewritten.admission_entry_sha256="f".repeat(64);
        assert!(consume_once_in_private_service_root(&root,&rewritten).is_err());
        let _=std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reject_missing_original_approval_or_fake_staged_hashes() {
        let plan=JobPlan {
            project_id:format!("ide-{}","a".repeat(32)),
            recipe_id:"dap_lldb_project".into(),
            source_head:"b".repeat(40),input_snapshot_sha256:"c".repeat(64),
            manifest_sha256:"d".repeat(64),toolchain_sha256:"e".repeat(64),
            debug_target_binding_sha256:Some("f".repeat(64)),
            runtime_limit_ms:90_000,memory_limit_bytes:512*1024*1024,
            process_limit:16,log_limit_bytes:65536,
            effect_class:"debug_inspection_isolated".into(),
        };
        let sha=crate::fingerprint(&plan).unwrap();
        let id=format!("job-{}","0".repeat(32));
        assert!(verify_consumed_original_approval(&[],&id,&plan,&sha,now_ms().unwrap()).is_err());
        assert!(verify_consumed_original_approval(&[],&id,&plan,&"a".repeat(64),now_ms().unwrap()).is_err());
    }
}
