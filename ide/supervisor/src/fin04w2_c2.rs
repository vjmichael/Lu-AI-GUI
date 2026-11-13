//! FIN-04W2-C2: small original-supervisor journal verifier, not launcher.
//!
//! The authenticated restricted SCM service owns the Job and the only
//! executable. Original UI consent remains an in-memory one-use supervisor
//! grant. A copied journal / hash is NOT evidence of physical human consent;
//! the service must independently authenticate the live host token/image,
//! protect its journal ACL and own the OS Job before honoring these records.
//! No executable, PID, command-line or DAP request is parsed in this module.
use crate::{JobPhase,JobPlan,Receipt,Store,SupervisorError};
use std::{fs,path::Path};
#[cfg(windows)]
use std::{fs::OpenOptions,io::Write};
#[cfg(windows)]
use sha2::{Digest,Sha256};

pub(crate) const WIRE_BYTES:usize=74;
const C2_MAGIC:&[u8;8]=b"LUC2RUN1";
// One fixed operation, one exact Job, one sealed plan and one service nonce.
// Zero path/PID/argument/DAP/chosen-executable fields or extension lengths.
pub(crate) fn approved_request(
    job_id:&str,plan:&JobPlan,nonce:&[u8;16],
)->Result<[u8;WIRE_BYTES],SupervisorError>{
    if !job_id_valid(job_id)||!crate::validate_plan(plan){
        return Err(denied());
    }
    let plan_sha=crate::fingerprint(plan)?;
    let mut out=[0u8;WIRE_BYTES];
    out[..8].copy_from_slice(C2_MAGIC);
    out[8]=1;out[9]=1; // protocol version and ONLY fixed benign worker.
    for (i,slot) in out[10..26].iter_mut().enumerate(){
        *slot=u8::from_str_radix(&job_id[4+i*2..6+i*2],16)
            .map_err(|_|denied())?;
    }
    for (i,slot) in out[26..58].iter_mut().enumerate(){
        *slot=u8::from_str_radix(&plan_sha[i*2..i*2+2],16)
            .map_err(|_|denied())?;
    }
    out[58..74].copy_from_slice(nonce);
    Ok(out)
}
pub(crate) fn verify_fixed_request(
    packet:&[u8],job_id:&str,plan:&JobPlan,nonce:&[u8;16],
)->Result<(),SupervisorError>{
    if packet!=approved_request(job_id,plan,nonce)?{
        return Err(denied());
    }
    Ok(())
}
fn denied()->SupervisorError{SupervisorError::InvalidApproval}
fn job_id_valid(id:&str)->bool{
    id.len()==36&&id.starts_with("job-")
        &&id[4..].bytes().all(|b|b.is_ascii_hexdigit())
}
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub(crate) enum C2JournalGate{Authorized,Running,ReleasePending}
impl C2JournalGate{
    fn phase(self)->JobPhase{match self{
        Self::Authorized=>JobPhase::Authorized,
        Self::Running=>JobPhase::Running,
        Self::ReleasePending=>JobPhase::ReleasePending,
    }}
    fn count(self)->usize{match self{
        Self::Authorized=>2,Self::Running=>4,Self::ReleasePending=>5,
    }}
}
/// The complete original multi-job journal is replayed with its hash chain;
/// only the exact target job's native phase sequence is eligible. No
/// replacement journal path or arbitrary receipts can arrive over the pipe.
pub(crate) fn check_original_receipts(
    rows:&[Receipt],job_id:&str,plan:&JobPlan,gate:C2JournalGate,
    now_ms:u64,
)->Result<(),SupervisorError>{
    if !job_id_valid(job_id)||!crate::validate_plan(plan)
        ||plan.recipe_id!="dap_lldb_project"
        ||plan.effect_class!="debug_inspection_isolated"
        ||now_ms==0{return Err(denied());}
    let full=Store::replay(rows.to_vec())?;
    if !full.quarantine.is_empty(){return Err(denied());}
    let job=full.jobs.get(job_id).ok_or_else(denied)?;
    let sha=crate::fingerprint(plan)?;
    // The original durable journal intentionally stores ONLY the
    // plan digest and project coordinate. Store::replay reconstructs a
    // blank plan to ensure no serialized approval can become an
    // executable grant after restart. Never require that blank plan to
    // equal the full sealed plan pinned independently by the service:
    // verify its complete SHA-256 and project binding instead.
    if job.phase!=gate.phase()||job.plan_hash!=sha
        ||job.plan.project_id!=plan.project_id{
        return Err(denied());
    }
    let own:Vec<&Receipt>=rows.iter().filter(|r|r.body.job_id==job_id).collect();
    if own.len()!=gate.count(){return Err(denied());}
    let phases=[
        (JobPhase::Prepared,"host_plan_prepared"),
        (JobPhase::Authorized,"fresh_human_approval"),
        (JobPhase::AdmissionPending,"grant_consumed_pending_os_admission"),
        (JobPhase::Running,"native_contained_worker_started"),
        (JobPhase::ReleasePending,"native_tree_empty_release_pending"),
    ];
    let session=&own[0].body.session_id;
    if session.len()!=40||!session.starts_with("session-")
        ||!session[8..].bytes().all(|b|b.is_ascii_hexdigit())
        ||own.iter().any(|r|r.body.session_id!=*session
            ||r.body.plan_sha256!=sha
            ||r.body.project_id!=plan.project_id){
        return Err(denied());
    }
    for (r,(phase,outcome)) in own.iter().zip(phases.iter()){
        if r.body.phase!=*phase||r.body.outcome_code!=*outcome{
            return Err(denied());
        }
    }
    if gate!=C2JournalGate::Authorized
        && own[2].body.evidence_code!="no_worker_yet_started"{
        return Err(denied());
    }
    if gate!=C2JournalGate::Authorized
        && own[3].body.evidence_code!="os_admission_and_input_seal_checked"{
        return Err(denied());
    }
    if gate==C2JournalGate::ReleasePending
        && own[4].body.evidence_code!="independent_service_ack_required"{
        return Err(denied());
    }
    let approved=own[1].body.observed_unix_ms;
    let latest=own.last().unwrap().body.observed_unix_ms;
    if approved>latest||latest>now_ms
        ||now_ms.saturating_sub(approved)>60_000{
        return Err(denied());
    }
    Ok(())
}
pub(crate) fn verify_original_path(
    file:&Path,job_id:&str,plan:&JobPlan,gate:C2JournalGate,
)->Result<(),SupervisorError>{
    if !file.is_absolute(){return Err(denied());}
    let meta=fs::symlink_metadata(file).map_err(|_|denied())?;
    if !meta.is_file()||meta.file_type().is_symlink()
        ||meta.len()==0||meta.len()>2*1024*1024{
        return Err(denied());
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return Err(denied());}
    }
    let now=crate::now_ms()?;
    check_original_receipts(
        &crate::read_journal(&file.to_path_buf())?,job_id,plan,gate,now,
    )
}
/// The service reserves each (original job, sealed plan) ONCE before any
/// suspended process is created. A torn/partial reservation is a permanent
/// replay blocker until an independent recovery operator reconciles it.
#[cfg(windows)]
pub(crate) fn reserve_once(root:&Path,job_id:&str,plan:&JobPlan)
    ->Result<(),SupervisorError>{
    if !root.is_absolute()||!job_id_valid(job_id){
        return Err(denied());
    }
    let meta=fs::symlink_metadata(root).map_err(|_|denied())?;
    use std::os::windows::fs::MetadataExt;
    if !meta.is_dir()||meta.file_type().is_symlink()
        ||meta.file_attributes()&0x400!=0{
        return Err(denied());
    }
    let plan_sha=crate::fingerprint(plan)?;
    let key=format!("{}:{}",job_id,plan_sha);
    let name=format!("fin04w2-c2-{:x}.receipt",Sha256::digest(key.as_bytes()));
    let file=root.join(name);
    let bytes=format!("fin04w2-c2-one-use-v1\n{}\n{}\n",job_id,plan_sha);
    let mut writer=OpenOptions::new().write(true).create_new(true)
        .open(&file).map_err(|_|denied())?;
    writer.write_all(bytes.as_bytes()).map_err(|_|denied())?;
    writer.sync_all().map_err(|_|denied())?;
    drop(writer);
    if fs::read(file).map_err(|_|denied())?!=bytes.as_bytes(){
        return Err(denied());
    }
    Ok(())
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn one_fixed_operation_has_no_path_pid_or_dynamic_command(){
        let plan=JobPlan{
            project_id:format!("ide-{}","a".repeat(32)),
            recipe_id:"dap_lldb_project".into(),
            source_head:"b".repeat(40),input_snapshot_sha256:"c".repeat(64),
            manifest_sha256:"d".repeat(64),toolchain_sha256:"e".repeat(64),
            debug_target_binding_sha256:Some("f".repeat(64)),
            runtime_limit_ms:90_000,memory_limit_bytes:512*1024*1024,
            process_limit:16,log_limit_bytes:65536,
            effect_class:"debug_inspection_isolated".into(),
        };
        let job=format!("job-{}","0".repeat(32));
        let packet=approved_request(&job,&plan,&[7;16]).unwrap();
        assert_eq!(packet.len(),74);
        assert!(verify_fixed_request(&packet,&job,&plan,&[7;16]).is_ok());
        let mut forged=packet;forged[9]=2;
        assert!(verify_fixed_request(&forged,&job,&plan,&[7;16]).is_err());
        assert!(verify_fixed_request(&packet,&job,&plan,&[8;16]).is_err());
        assert!(verify_fixed_request(&packet,&format!("job-{}","1".repeat(32)),
            &plan,&[7;16]).is_err());
        assert!(verify_fixed_request(&packet[..73],&job,&plan,&[7;16]).is_err());
    }
    #[test]fn missing_authorization_is_not_a_service_grant(){
        let plan=JobPlan{
            project_id:format!("ide-{}","a".repeat(32)),
            recipe_id:"dap_lldb_project".into(),
            source_head:"b".repeat(40),input_snapshot_sha256:"c".repeat(64),
            manifest_sha256:"d".repeat(64),toolchain_sha256:"e".repeat(64),
            debug_target_binding_sha256:Some("f".repeat(64)),
            runtime_limit_ms:90_000,memory_limit_bytes:512*1024*1024,
            process_limit:16,log_limit_bytes:65536,
            effect_class:"debug_inspection_isolated".into(),
        };
        let job=format!("job-{}","0".repeat(32));
        assert!(check_original_receipts(&[],&job,&plan,C2JournalGate::Authorized,1).is_err());
        assert!(check_original_receipts(&[],&job,&plan,C2JournalGate::Running,1).is_err());
        assert!(check_original_receipts(&[],&"job-".to_owned(),&plan,
            C2JournalGate::ReleasePending,1).is_err());
    }
}
