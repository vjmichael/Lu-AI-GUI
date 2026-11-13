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
    digest.update(proof.admission_entry_sha256.as_bytes());
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
