//! AGENT-01L: read-only OS process-custody witness reader.
//!
//! Windows evidence is written only after the live, assigned Win32 Job
//! handle itself successfully observes BasicAccounting.ActiveProcesses=0.
//! Linux process-group SIGKILL alone is NOT sufficient against setsid().
//! Linux only accepts the separately supervised delegated-cgroup-v2
//! custodian's exact original-turn recursive populated-zero receipt; absence
//! of genuine delegation or OS proof remains unverified.
//!
//! This is an historical kernel observation, not proof of a running process's
//! current state, a portable cgroup guarantee, a secret signature, or a grant.
use crate::agent_recovery_decision::IndependentProcessCustody;
use std::{fs,path::{Path,PathBuf}};
#[cfg(target_os="linux")]
use std::os::unix::fs::PermissionsExt;
#[cfg(any(windows,target_os="linux"))]
use rusqlite::{params,Connection,OpenFlags,OptionalExtension};

pub struct HostOriginalJobCustody<'a>{
    profile:&'a Path,
}
impl<'a> HostOriginalJobCustody<'a>{
    /// The trusted host selects this profile path. Never allow a WebView,
    /// model response or saved plan to choose a journal directory.
    pub fn from_host_profile(profile:&'a Path)->Self{Self{profile}}
}
fn bounded_journal(path:&Path,allow_empty:bool)->bool{
    let Ok(meta)=fs::symlink_metadata(path)else{return false};
    if !meta.is_file()||meta.file_type().is_symlink()
        ||(meta.len()==0&&!allow_empty)||meta.len()>64*1024*1024{
        return false
    }
    #[cfg(windows)]{
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes()&0x400!=0{return false}
    }
    true
}
fn exact_id(value:&str,prefix:&str)->bool{
    value.len()==prefix.len()+32&&value.starts_with(prefix)
        &&value[prefix.len()..].bytes().all(|b|
            b.is_ascii_hexdigit()&&!b.is_ascii_uppercase())
}
#[cfg(windows)]
fn windows_original_job_empty(
    profile:&Path,chat:&str,turn:&str,call:&str,
)->bool{
    if !exact_id(chat,"c-")||!exact_id(turn,"g-")
        ||call.len()>128||call.is_empty()||!call.bytes().all(|b|
            b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
        ||!["-gh","-gs","-gb","-gc","-fm"]
            .iter().any(|s|call.ends_with(s)){
        return false
    }
    let file=profile.join(format!("lu-reviewed-task-{turn}.sqlite"));
    if !bounded_journal(&file,false){return false}
    for suffix in ["-wal","-shm","-journal"]{
        let side=PathBuf::from(format!("{}{}",file.display(),suffix));
        match fs::symlink_metadata(&side){
            Ok(_) if !bounded_journal(&side,true)=>return false,
            Err(e) if e.kind()!=std::io::ErrorKind::NotFound=>return false,
            _=>{},
        }
    }
    let Ok(db)=Connection::open_with_flags(
        &file,OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX
    )else{return false};
    // ALL independent identity joins must match the already verified
    // AGENT-01C exact effect call, saved host chat and execution turn.
    // A forged task status alone cannot create the kernel observation row.
    let row:Result<Option<(String,String,String,String,String)>,_>=db.query_row(
        "SELECT t.state,t.workspace_id,o.conversation_id,o.user_turn_id,
                j.method
         FROM lu_task_windows_job_custody j
         JOIN lu_tasks t ON t.task_id=j.task_id
         JOIN lu_reviewed_turn_origin o ON o.singleton=1
         WHERE j.task_id=?1 AND j.workspace_id=?2
         LIMIT 1",
        params![call,turn],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
    ).optional();
    matches!(row,Ok(Some((state,workspace,saved_chat,saved_turn,method)))
        if state=="COMPLETED"&&workspace==turn&&saved_chat==chat
           &&saved_turn==turn&&method=="WINDOWS_JOB_ACTIVE_ZERO_V1")
}
/// Kernel-custodian receipt is written from an out-of-process first-party
/// executable on EOF after cgroup.kill and recursive cgroup.events populated0.
/// It remains historical data only: AGENT-01K separately rechecks the actual
/// task, effect, independent oracle, current ANSF and released host owner.
#[cfg(target_os="linux")]
fn linux_original_cgroup_empty(
    profile:&Path,chat:&str,turn:&str,call:&str,
)->bool{
    if !exact_id(chat,"c-")||!exact_id(turn,"g-")
        ||call.len()>128||call.is_empty()||!call.bytes().all(|b|
            b.is_ascii_alphanumeric()||b==b'-'||b==b'_')
        ||!["-gh","-gs","-gb","-gc","-fm"]
            .iter().any(|s|call.ends_with(s)){
        return false
    }
    let task=profile.join(format!("lu-reviewed-task-{turn}.sqlite"));
    if !bounded_journal(&task,false){return false}
    for suffix in ["-wal","-shm","-journal"]{
        let side=PathBuf::from(format!("{}{}",task.display(),suffix));
        match fs::symlink_metadata(&side){
            Ok(_) if !bounded_journal(&side,true)=>return false,
            Err(e) if e.kind()!=std::io::ErrorKind::NotFound=>return false,
            _=>{},
        }
    }
    let Ok(db)=Connection::open_with_flags(&task,
        OpenFlags::SQLITE_OPEN_READ_ONLY|OpenFlags::SQLITE_OPEN_NO_MUTEX
    )else{return false};
    let row:Result<Option<(String,String,String,String)>,_>=db.query_row(
        "SELECT t.state,t.workspace_id,o.conversation_id,o.user_turn_id
         FROM lu_tasks t JOIN lu_reviewed_turn_origin o ON o.singleton=1
         WHERE t.task_id=?1 AND t.workspace_id=?2",
        params![call,turn],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))
    ).optional();
    if !matches!(row,Ok(Some((state,workspace,saved_chat,saved_turn)))
        if state=="COMPLETED"&&workspace==turn&&saved_chat==chat
            &&saved_turn==turn){
        return false
    }
    let receipt=profile.join(format!("lu-cgroup-os-{turn}-{call}.receipt"));
    let Ok(meta)=fs::symlink_metadata(&receipt)else{return false};
    if !meta.is_file()||meta.file_type().is_symlink()
        ||meta.len()==0||meta.len()>4096
        ||meta.permissions().mode()&0o077!=0{
        return false
    }
    let Ok(data)=fs::read_to_string(&receipt)else{return false};
    // Strictly one proof per exact task and human turn. No model-supplied
    // path, arbitrary PID or mutation of the old TaskStore is consulted.
    let lines=data.lines().collect::<Vec<_>>();
    if lines.len()!=6
        ||lines[0]!="LU_CGROUP_V2_EMPTY_V1"
        ||lines[1]!=format!("CHAT={chat}")
        ||lines[2]!=format!("TURN={turn}")
        ||lines[3]!=format!("CALL={call}")
        ||lines[5]!="POPULATED=0"{
        return false
    }
    let Some(nonce)=lines[4].strip_prefix("NONCE=")else{return false};
    nonce.len()==32&&nonce.bytes().all(|b|
        b.is_ascii_digit()||(b'a'..=b'f').contains(&b))
}
impl IndependentProcessCustody for HostOriginalJobCustody<'_>{
    fn worker_and_descendants_ended(
        &self,chat:&str,execution_turn:&str,effect_call:&str
    )->bool{
        #[cfg(windows)]
        {return windows_original_job_empty(
            self.profile,chat,execution_turn,effect_call
        )}
        #[cfg(target_os="linux")]
        {return linux_original_cgroup_empty(
            self.profile,chat,execution_turn,effect_call
        )}
        #[cfg(all(not(windows),not(target_os="linux")))]
        {
            let _=(&self.profile,chat,execution_turn,effect_call);
            false // Unsupported OS: no independent full-tree witness.
        }
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn exact_ids_do_not_accept_case_or_cross_type(){
        assert!(exact_id(&format!("g-{}","a".repeat(32)),"g-"));
        assert!(!exact_id(&format!("g-{}","A".repeat(32)),"g-"));
        assert!(!exact_id(&format!("c-{}","a".repeat(32)),"g-"));
    }
    #[test]
    fn missing_original_task_journal_never_attests_os_cleanup(){
        let profile=std::env::temp_dir().join(format!(
            "lu-agent01l-missing-{}-{}",std::process::id(),42
        ));
        let witness=HostOriginalJobCustody::from_host_profile(&profile);
        assert!(!witness.worker_and_descendants_ended(
            &format!("c-{}","1".repeat(32)),
            &format!("g-{}","2".repeat(32)),"real-call-gh"
        ));
    }
    #[cfg(not(windows))]
    #[test]
    fn linux_pgid_guardian_never_claims_escaped_descendant_custody(){
        let witness=HostOriginalJobCustody::from_host_profile(Path::new("/"));
        assert!(!witness.worker_and_descendants_ended(
            &format!("c-{}","1".repeat(32)),
            &format!("g-{}","2".repeat(32)),"real-call-gh"
        ));
    }
}
