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
use std::{io::Read,fs::OpenOptions,os::unix::fs::{MetadataExt,OpenOptionsExt,PermissionsExt}};
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
        {
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
/// SEC-06C: the fixed systemd Git broker's original nonce is durably
/// bound before the reviewed operation in Lu's task journal. The historical
/// kernel-empty receipt is then read only from root-owned OS storage.
/// Neither an arbitrary same-UID file nor a fabricated model result is proof.
#[cfg(target_os="linux")]
fn linux_os_owned_git_original_receipt(
    db:&Connection,chat:&str,turn:&str,call:&str,
)->Option<bool>{
    let row:Result<Option<(String,String,String,String,String)>,_>=db.query_row(
        "SELECT a.chat,a.turn,a.nonce,a.git_sha256,t.state
         FROM lu_task_linux_os_owned_git_admission a
         JOIN lu_tasks t ON t.task_id=a.task_id
         JOIN lu_reviewed_turn_origin o ON o.singleton=1
         WHERE a.task_id=?1 AND a.chat=?2 AND a.turn=?3
           AND t.workspace_id=?3
           AND o.conversation_id=?2 AND o.user_turn_id=?3",
        params![call,chat,turn],
        |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
    ).optional();
    let (saved_chat,saved_turn,nonce,digest,state)=match row{
        Ok(Some(row))=>row,
        Ok(None)=>return None, // Older owned-cgroup receipt format.
        Err(_)=>return None, // Legacy DB may predate this schema.
    };
    let valid_hex=|s:&str,n:usize|
        s.len()==n&&s.bytes().all(|b|
            b.is_ascii_digit()||(b'a'..=b'f').contains(&b));
    if state!="COMPLETED"||saved_chat!=chat||saved_turn!=turn
        ||!valid_hex(&nonce,32)||!valid_hex(&digest,64){
        return Some(false)
    }
    let root=Path::new("/var/lib/lu-sec06c/receipts");
    let Ok(meta)=fs::symlink_metadata(root)else{return Some(false)};
    if !meta.is_dir()||meta.file_type().is_symlink()||meta.uid()!=0
        ||meta.permissions().mode()&0o022!=0{return Some(false)}
    let path=root.join(format!("{nonce}.receipt"));
    const NOFOLLOW:i32=0o400000;
    let Ok(mut input)=OpenOptions::new().read(true)
        .custom_flags(NOFOLLOW).open(path)else{return Some(false)};
    let Ok(meta)=input.metadata()else{return Some(false)};
    if !meta.is_file()||meta.uid()!=0||meta.nlink()!=1
        ||meta.permissions().mode()&0o022!=0||meta.len()>1024{
        return Some(false)
    }
    let mut bytes=Vec::new();
    if input.take(1025).read_to_end(&mut bytes).is_err(){return Some(false)}
    let Ok(raw)=String::from_utf8(bytes)else{return Some(false)};
    let expected=format!(
        "LU_SEC06C_OWNED_GIT_V1\nNONCE={nonce}\nCHAT={chat}\nTURN={turn}\nCALL={call}\nGIT_SHA256={digest}\nUNIT=lu-sec06c-git-{nonce}.service\nORIGINAL_UID=",
    );
    let Some(uid_tail)=raw.strip_prefix(&expected)else{return Some(false)};
    let Some((uid,remaining))=uid_tail.split_once("\n")else{return Some(false)};
    if uid.parse::<u32>().ok().is_none_or(|n|n==0){return Some(false)}
    Some(remaining=="KERNEL_EMPTY=1\nEFFECT=COMPLETED\n")
}
/// SEC-06C distinct reviewed metadata/media worker: only an immutable
/// original Lu task->nonce+binary+resource+kind binding can join the matching
/// root-owned systemd kernel-empty receipt. A cancelled/abandoned operation
/// remains unaccepted even after its original cgroup eventually becomes empty.
#[cfg(target_os="linux")]
fn linux_os_owned_probe_original_receipt(
    db:&Connection,chat:&str,turn:&str,call:&str,
)->Option<bool>{
    let row:Result<Option<(String,String,String,String,String,String,String)>,_>=
        db.query_row(
          "SELECT a.chat,a.turn,a.nonce,a.kind,
                  a.binary_sha256,a.resource_sha256,t.state
           FROM lu_task_linux_os_owned_probe_admission a
           JOIN lu_tasks t ON t.task_id=a.task_id
           JOIN lu_reviewed_turn_origin o ON o.singleton=1
           WHERE a.task_id=?1 AND a.chat=?2 AND a.turn=?3
             AND t.workspace_id=?3 AND o.conversation_id=?2
             AND o.user_turn_id=?3",
          params![call,chat,turn],
          |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
                 r.get(4)?,r.get(5)?,r.get(6)?))
        ).optional();
    let (saved_chat,saved_turn,nonce,kind,binary,resource,state)=match row{
        Ok(Some(x))=>x,
        _=>return None,
    };
    let hex=|v:&str,n:usize|v.len()==n&&v.bytes().all(|b|
        b.is_ascii_digit()||(b'a'..=b'f').contains(&b));
    if state!="COMPLETED"||saved_chat!=chat||saved_turn!=turn
        ||!hex(&nonce,32)||!hex(&binary,64)||!hex(&resource,64)
        ||!matches!(kind.as_str(),"ffprobe-wav"|"ffprobe-mp4"
            |"pdfinfo-pdf"|"exiftool-image"){
        return Some(false)
    }
    let root=Path::new("/var/lib/lu-sec06c/probe-receipts");
    let Ok(meta)=fs::symlink_metadata(root)else{return Some(false)};
    if !meta.is_dir()||meta.file_type().is_symlink()||meta.uid()!=0
        ||meta.permissions().mode()&0o022!=0{return Some(false)}
    let path=root.join(format!("{nonce}.receipt"));
    const NOFOLLOW:i32=0o400000;
    let Ok(mut file)=OpenOptions::new().read(true)
        .custom_flags(NOFOLLOW).open(path)else{return Some(false)};
    let Ok(meta)=file.metadata()else{return Some(false)};
    if !meta.is_file()||meta.uid()!=0||meta.nlink()!=1
        ||meta.len()>1024||meta.permissions().mode()&0o022!=0{
        return Some(false)
    }
    let mut bytes=Vec::new();
    if file.take(1025).read_to_end(&mut bytes).is_err(){
        return Some(false)
    }
    let Ok(raw)=String::from_utf8(bytes)else{return Some(false)};
    let prefix=format!(
        "LU_SEC06C_OWNED_PROBE_V1\nNONCE={nonce}\nCHAT={chat}\nTURN={turn}\nCALL={call}\nKIND={kind}\nBINARY_SHA256={binary}\nRESOURCE_SHA256={resource}\nUNIT=lu-sec06c-probe-{nonce}.service\nORIGINAL_UID=",
    );
    let Some(rem)=raw.strip_prefix(&prefix)else{return Some(false)};
    let Some((uid,outcome))=rem.split_once("\n")else{return Some(false)};
    if uid.parse::<u32>().ok().is_none_or(|v|v==0){return Some(false)}
    Some(outcome=="KERNEL_EMPTY=1\nEFFECT=COMPLETED\n")
}
/// SEC-06C immutable root-owned first-party scanner/archive/version receipt.
/// Acceptance joins the original completed task and exact human turn; a
/// cancelled/revoked/uncertain effect cannot reuse an old zero-cgroup proof.
#[cfg(target_os="linux")]
fn linux_os_owned_native_original_receipt(
    db:&Connection,chat:&str,turn:&str,call:&str,
)->Option<bool>{
    let row:Result<Option<(String,String,String,String,String,String,String)>,_>=
        db.query_row(
          "SELECT a.chat,a.turn,a.nonce,a.kind,
                  a.binary_sha256,a.argv_sha256,t.state
           FROM lu_task_linux_os_owned_native_admission a
           JOIN lu_tasks t ON t.task_id=a.task_id
           JOIN lu_reviewed_turn_origin o ON o.singleton=1
           WHERE a.task_id=?1 AND a.chat=?2 AND a.turn=?3
             AND t.workspace_id=?3 AND o.conversation_id=?2
             AND o.user_turn_id=?3",
          params![call,chat,turn],
          |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,
                 r.get(4)?,r.get(5)?,r.get(6)?))
        ).optional();
    let (saved_chat,saved_turn,nonce,kind,binary,argv,state)=match row{
        Ok(Some(v))=>v,_=>return None,
    };
    let hex=|v:&str,n:usize|v.len()==n&&v.bytes().all(|c|
        c.is_ascii_digit()||(b'a'..=b'f').contains(&c));
    if saved_chat!=chat||saved_turn!=turn||state!="COMPLETED"
        ||!hex(&nonce,32)||!hex(&binary,64)||!hex(&argv,64)
        ||!matches!(kind.as_str(),"scan-file"|"archive-preview"
            |"archive-extract"|"fixed-version"|"fixed-artifact"|"app-version"){
        return Some(false)
    }
    let root=Path::new("/var/lib/lu-sec06c/native-receipts");
    let Ok(meta)=fs::symlink_metadata(root)else{return Some(false)};
    if !meta.is_dir()||meta.file_type().is_symlink()||meta.uid()!=0
        ||meta.permissions().mode()&0o022!=0{return Some(false)}
    let file=root.join(format!("{nonce}.receipt"));
    const NOFOLLOW:i32=0o400000;
    let Ok(f)=OpenOptions::new().read(true)
        .custom_flags(NOFOLLOW).open(file)else{return Some(false)};
    let Ok(meta)=f.metadata()else{return Some(false)};
    if !meta.is_file()||meta.uid()!=0||meta.nlink()!=1
        ||meta.len()>1024||meta.permissions().mode()&0o022!=0{
        return Some(false)
    }
    let mut bytes=Vec::new();
    if f.take(1025).read_to_end(&mut bytes).is_err(){return Some(false)}
    let Ok(raw)=String::from_utf8(bytes)else{return Some(false)};
    let prefix=format!(
        "LU_SEC06C_OWNED_NATIVE_V1\nNONCE={nonce}\nCHAT={chat}\nTURN={turn}\nCALL={call}\nKIND={kind}\nBINARY_SHA256={binary}\nARGV_SHA256={argv}\nUNIT=lu-sec06c-native-{nonce}.service\nORIGINAL_UID=",
    );
    let Some(tail)=raw.strip_prefix(&prefix)else{return Some(false)};
    let Some((uid,outcome))=tail.split_once("\n")else{return Some(false)};
    if uid.parse::<u32>().ok().is_none_or(|n|n==0){return Some(false)}
    Some(outcome=="KERNEL_EMPTY=1\nEFFECT=COMPLETED\n")
}
/// Kernel-custodian receipt is written from an out-of-process first-party
/// executable on EOF after cgroup.kill and recursive cgroup.events populated0.
/// It remains historical data only: AGENT-01K separately rechecks the actual
/// task, effect, independent oracle, current ANSF and released host owner.
#[cfg(target_os="linux")]
fn linux_original_cgroup_empty(
    profile:&Path,chat:&str,turn:&str,call:&str,
)->bool{
    // The original full human-turn/journal identity is required for ALL
    // finite OS-owned domains. Never restrict it to legacy Git/FFprobe
    // suffixes BEFORE independently checking the new root-owned native
    // scanner/archive/version receipt. The legacy mutable-profile fallback
    // remains suffix-restricted below, after all root-owned proof checks.
    if !exact_id(chat,"c-")||!exact_id(turn,"g-")
        ||call.len()>128||call.is_empty()||!call.bytes().all(|b|
            b.is_ascii_alphanumeric()||b==b'-'||b==b'_'){
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
    if let Some(accepted)=linux_os_owned_git_original_receipt(
        &db,chat,turn,call
    ){return accepted}
    if let Some(accepted)=linux_os_owned_probe_original_receipt(
        &db,chat,turn,call
    ){return accepted}
    if let Some(accepted)=linux_os_owned_native_original_receipt(
        &db,chat,turn,call
    ){return accepted}
    // The legacy mutable-profile cgroup receipt is supported only for its
    // preexisting finite original Git/FFprobe task suffixes; all newer native
    // tasks require their separate immutable root-owned receipt instead.
    if !["-gh","-gs","-gb","-gc","-fm"]
        .iter().any(|suffix|call.ends_with(suffix)){return false}
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
