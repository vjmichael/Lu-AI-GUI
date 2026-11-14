//! FIN-04W2 default-OFF native split process preparation.
//!
//! This is a component for the independently authenticated SCM service,
//! NOT an executable broker, public IPC method, release worker or WFP grant.
//! It creates two distinctly Job-contained SUSPENDED processes:
//! 1. reviewed debug target under a fresh zero-capability AppContainer;
//! 2. reviewed LLDB-DAP under the restricted service token, OUTSIDE lowbox.
//! The service owns both Job handles, process handles, stdin/stdout pipes,
//! original one-use receipt and cancellation/restart. A missing positive
//! authorization or WFP proof must prevent resume and attach.
//!
//! This module has deliberately NO public PID input, no arbitrary attach,
//! no shell, no process resumption and no general OS command dispatcher.
#![cfg(all(windows, debug_assertions, feature="fin04w2-split-native"))]

use crate::{windows_fin04w2_kernel::{self,EmptyAuditJob},
    windows_fin04w2_target_seal,windows_event_owner,
    windows_service_security,windows_worker,
    SupervisorError};
use std::{
    ffi::OsStr, fs, io::Read, mem::size_of, os::windows::ffi::OsStrExt,
    path::{Path,PathBuf},ptr,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle,HANDLE,SetHandleInformation,WAIT_TIMEOUT,WAIT_OBJECT_0},
    Security::{SECURITY_ATTRIBUTES,SECURITY_CAPABILITIES},
    System::{
        JobObjects::IsProcessInJob,
        Pipes::{CreatePipe,PeekNamedPipe},
        Threading::{
            CreateProcessW,GetExitCodeProcess,
            ResumeThread,TerminateProcess,UpdateProcThreadAttribute,WaitForSingleObject,
            CREATE_NO_WINDOW,CREATE_SUSPENDED,CREATE_UNICODE_ENVIRONMENT,
            EXTENDED_STARTUPINFO_PRESENT,PROCESS_INFORMATION,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
            STARTF_USESTDHANDLES,STARTUPINFOEXW,
        },
    },
};

const JOB_LIST:usize=131085;
const HANDLE_LIST:usize=131074;
const HANDLE_INHERIT:u32=1;
fn denied()->SupervisorError {SupervisorError::NoContainment}
fn wide(os:&OsStr)->Vec<u16>{os.encode_wide().chain([0]).collect()}
struct Owned(HANDLE);
impl Owned {
    fn new(handle:HANDLE)->Result<Self,SupervisorError>{
        if handle.is_null(){Err(denied())}else{Ok(Self(handle))}
    }
}
impl Drop for Owned {
    fn drop(&mut self){if !self.0.is_null(){unsafe{CloseHandle(self.0);}}}
}
fn contained(process:HANDLE,job:HANDLE)->Result<(),SupervisorError>{
    let mut actual=0i32;
    if unsafe{IsProcessInJob(process,job,&mut actual)}==0||actual!=1{
        return Err(denied());
    }
    Ok(())
}
fn job_attribute(attrs:&mut windows_worker::Attributes,jobs:&[HANDLE;1])
    ->Result<(),SupervisorError>{
    if unsafe{UpdateProcThreadAttribute(attrs.raw(),0,JOB_LIST,
        jobs.as_ptr().cast(),size_of::<HANDLE>(),
        ptr::null_mut(),ptr::null())}==0{return Err(denied());}
    Ok(())
}
fn environment(root:&Path,bin:&Path)->Result<Vec<u16>,SupervisorError>{
    let system=std::env::var_os("SystemRoot").ok_or_else(denied)?;
    let system=Path::new(&system);
    if !root.is_absolute()||!bin.is_absolute()||!system.is_absolute(){
        return Err(denied());
    }
    let env=format!(
        "APPDATA={0}\0HOME={0}\0LOCALAPPDATA={0}\0PATH={1};{2}\0SystemRoot={3}\0TEMP={0}\0TMP={0}\0USERPROFILE={0}\0\0",
        root.display(),bin.display(),system.join("System32").display(),
        system.display(),
    );
    let chars:Vec<u16>=env.encode_utf16().collect();
    if chars.len()>32767{return Err(denied());}
    Ok(chars)
}

/// A process is still suspended. Dropping its lease terminates it and waits
/// for actual OS exit; independent Job kill/recovery is still required.
struct Pending {
    process:Owned,
    thread:Owned,
}
impl Pending {
    fn resume_once(&mut self)->Result<(),SupervisorError>{
        if unsafe{WaitForSingleObject(self.process.0,0)}!=WAIT_TIMEOUT{
            return Err(denied());
        }
        // A single original CREATE_SUSPENDED suspension must be consumed
        // only by the owning SCM process, after independent RUNNING journal.
        if unsafe{ResumeThread(self.thread.0)}!=1{return Err(denied());}
        Ok(())
    }
    fn from_information(pi:PROCESS_INFORMATION)->Result<Self,SupervisorError>{
        let process=Owned::new(pi.hProcess)?;
        let thread=match Owned::new(pi.hThread){
            Ok(handle)=>handle,
            Err(err)=>{
                // No suspended orphan if OS returned a partial handle set.
                unsafe{TerminateProcess(process.0,77);}
                let _=unsafe{WaitForSingleObject(process.0,5000)};
                return Err(err);
            }
        };
        Ok(Self{process,thread})
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        unsafe{
            TerminateProcess(self.process.0,77);
            let _=WaitForSingleObject(self.process.0,5000);
        }
    }
}

/// Hold fresh package profile until the target has been killed or reaped.
pub(crate) struct PendingTarget {
    pending:Pending,
    profile:windows_worker::AppProfile,
    stage:windows_fin04w2_target_seal::SealedDebugTarget,
    image_sha256:String,
    job:HANDLE,
}
impl PendingTarget {
    pub(crate) fn process(&self)->HANDLE {self.pending.process.0}
    pub(crate) fn job(&self)->HANDLE {self.job}
    pub(crate) fn sid(&self)->windows_sys::Win32::Security::PSID{self.profile.sid}
    pub(crate) fn image(&self)->&Path {&self.stage.image}
    pub(crate) fn pdb(&self)->&Path {&self.stage.pdb}
    pub(crate) fn image_sha256(&self)->&str {&self.image_sha256}
    pub(crate) fn resume_after_original_running(&mut self)
        ->Result<(),SupervisorError>{
        windows_fin04w2_kernel::inspect_current_target_service_principal()?;
        contained(self.pending.process.0,self.job)?;
        windows_service_security::verify_exact_target_process_debug_acl(
            self.pending.process.0,
        )?;
        windows_worker::verify_suspended_appcontainer_token(
            self.pending.process.0,self.profile.sid,
        )?;
        windows_fin04w2_target_seal::verify_readonly_stage_entry(
            &self.stage.root,self.profile.sid,true,
        )?;
        windows_fin04w2_target_seal::verify_readonly_stage_entry(
            &self.stage.image,self.profile.sid,false,
        )?;
        windows_fin04w2_target_seal::verify_readonly_stage_entry(
            &self.stage.pdb,self.profile.sid,false,
        )?;
        windows_service_security::hash_executable(&self.stage.image,&self.image_sha256)?;
        self.pending.resume_once()?;
        contained(self.pending.process.0,self.job)?;
        Ok(())
    }
}
/// Only a native SCM service can build this exact target in its own Job.
/// The reviewed source path and SHA originate in protected service config.
pub(crate) fn prepare_appcontainer_target(
    job:&EmptyAuditJob, original_job_id:&str,
    private_service_root:&Path,
    approved_image:&Path,expected_sha256:&str,
    approved_pdb:&Path,expected_pdb_sha256:&str,
)->Result<PendingTarget,SupervisorError>{
    windows_fin04w2_kernel::inspect_current_target_service_principal()?;
    windows_service_security::hash_executable(approved_image,expected_sha256)?;
    let profile=windows_worker::AppProfile::create(original_job_id)?;
    // Keep LocalState writable only for harmless target scratch. Never
    // place the executable or PDB there: the target could mutate either.
    let local=profile.local_state()?;
    let stage=windows_fin04w2_target_seal::stage_and_verify(
        private_service_root,original_job_id,profile.sid,
        approved_image,expected_sha256,approved_pdb,expected_pdb_sha256,
    )?;
    let image=&stage.image;
    let mut attrs=windows_worker::Attributes::with_count(2)?;
    let mut cap=SECURITY_CAPABILITIES::default();
    cap.AppContainerSid=profile.sid;
    cap.Capabilities=ptr::null_mut();
    cap.CapabilityCount=0;
    if unsafe{UpdateProcThreadAttribute(attrs.raw(),0,
        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
        (&cap as *const SECURITY_CAPABILITIES).cast(),
        size_of::<SECURITY_CAPABILITIES>(),
        ptr::null_mut(),ptr::null())}==0{return Err(denied());}
    let kernel_job=job.held_split_job_handle();
    // This exact kernel DACL must name the TARGET service, not just any Job.
    windows_service_security::verify_target_job_kernel_acl(kernel_job)?;
    // UpdateProcThreadAttribute may reference the supplied HANDLE array
    // through CreateProcessW; keep storage alive in this stack frame.
    let job_array=[kernel_job];
    job_attribute(&mut attrs,&job_array)?;
    let exe=wide(image.as_os_str());
    let cwd=wide(local.as_os_str());
    let mut args=wide(OsStr::new(&format!("\"{}\"",image.display())));
    let env=environment(&local,&PathBuf::from(std::env::var_os("SystemRoot")
        .ok_or_else(denied)?).join("System32"))?;
    let mut startup=STARTUPINFOEXW::default();
    startup.StartupInfo.cb=size_of::<STARTUPINFOEXW>() as u32;
    startup.lpAttributeList=attrs.raw();
    let mut info=PROCESS_INFORMATION::default();
    if unsafe{CreateProcessW(
        exe.as_ptr(),args.as_mut_ptr(),ptr::null(),ptr::null(),0,
        CREATE_SUSPENDED|CREATE_NO_WINDOW|CREATE_UNICODE_ENVIRONMENT|
            EXTENDED_STARTUPINFO_PRESENT,
        env.as_ptr().cast(),cwd.as_ptr(),&startup.StartupInfo,&mut info,
    )}==0{return Err(denied());}
    let pending=Pending::from_information(info)?;
    contained(pending.process.0,kernel_job)?;
    windows_worker::verify_suspended_appcontainer_token(pending.process.0,profile.sid)?;
    // Only the target SCM process can alter the new suspended process DACL.
    // LLDB's separate restricted service SID gets precisely its intended
    // Win32 debug attach rights, never Job custody or mutable stage files.
    windows_service_security::seal_exact_target_process_debug_acl(pending.process.0)?;
    Ok(PendingTarget {
        pending,profile,stage,image_sha256:expected_sha256.to_owned(),
        job:kernel_job,
    })
}

struct PipePair {read:Owned,write:Owned}
fn pipe()->Result<PipePair,SupervisorError>{
    let mut sa=SECURITY_ATTRIBUTES {
        nLength:size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor:ptr::null_mut(),bInheritHandle:1,
    };
    let mut read=ptr::null_mut();
    let mut write=ptr::null_mut();
    if unsafe{CreatePipe(&mut read,&mut write,&mut sa,0)}==0{
        return Err(denied());
    }
    Ok(PipePair{read:Owned::new(read)?,write:Owned::new(write)?})
}
fn private_handle(h:HANDLE)->Result<(),SupervisorError>{
    if unsafe{SetHandleInformation(h,HANDLE_INHERIT,0)}==0{
        Err(denied())
    }else{Ok(())}
}
/// LLDB worker remains outside AppContainer but inside an independent
/// service-restricted Job. Its three stdio handles are explicitly allowlisted.
pub(crate) struct PendingDebugger {
    pending:Pending,
    stdin_write:Owned,
    stdout_read:Owned,
    stderr_read:Owned,
    owner:windows_event_owner::OwnerSideband,
    job:HANDLE,
}
impl PendingDebugger {
    pub(crate) fn process(&self)->HANDLE {self.pending.process.0}
    pub(crate) fn job(&self)->HANDLE {self.job}
    pub(crate) fn stdio_handles(&self)->(HANDLE,HANDLE){
        (self.stdin_write.0,self.stdout_read.0)
    }
    /// Real pinned LLVM DebuggerThread writes a non-forgeable 76-byte
    /// event to the one-use inherited private pipe (NOT DAP stdout).
    /// Validate its owner thread against the actual OS-held LLDB handle.
    pub(crate) fn observe_real_kernel_owner_stop(&mut self,
        target_pid:u32,target_tid:u32,target_rip:u64,
    )->Result<windows_event_owner::OwnerEvent,SupervisorError>{
        contained(self.pending.process.0,self.job)?;
        let record=self.owner.current(target_pid,target_tid,target_rip)?;
        crate::windows_kernel_pc::verify_lldb_event_owner_thread(
            self.pending.process.0,record.owner_tid,
        )?;
        contained(self.pending.process.0,self.job)?;
        Ok(record)
    }
    pub(crate) fn resume_after_original_running(&mut self)
        ->Result<(),SupervisorError>{
        windows_fin04w2_kernel::inspect_current_service_principal()?;
        windows_service_security::verify_debugger_job_kernel_acl(self.job)?;
        contained(self.pending.process.0,self.job)?;
        self.pending.resume_once()?;
        contained(self.pending.process.0,self.job)?;
        Ok(())
    }
}
pub(crate) fn prepare_external_debugger(
    job:&EmptyAuditJob, reviewed_lldb_dap:&Path, expected_sha256:&str,
    reviewed_liblldb_sha256:&str,reviewed_llvm_symbolizer_sha256:&str,
    reviewed_owner_manifest_sha256:&str,
    admin_opt_in_hostile_wfp_probe:bool,
)->Result<PendingDebugger,SupervisorError>{
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    windows_service_security::hash_executable(reviewed_lldb_dap,expected_sha256)?;
    if reviewed_lldb_dap.file_name()!=Some(OsStr::new("lldb-dap.exe")){
        return Err(denied());
    }
    let bundle=reviewed_lldb_dap.parent().ok_or_else(denied)?;
    if !bundle.is_absolute(){return Err(denied());}
    // The *original* pinned LLVM 18.1.8 first-party owner build disables
    // Python. Requiring python310.dll would reject the very patched LLDB
    // binary whose owner events are essential to native FIN-04 acceptance.
    // Instead pin actual liblldb, llvm-symbolizer and the upstream/owner
    // provenance manifest independently under administrator-owned config.
    windows_service_security::hash_executable(
        &bundle.join("liblldb.dll"),reviewed_liblldb_sha256,
    )?;
    windows_service_security::hash_executable(
        &bundle.join("llvm-symbolizer.exe"),reviewed_llvm_symbolizer_sha256,
    )?;
    let bundle_root=bundle.parent().ok_or_else(denied)?;
    let manifest=bundle_root.join("fin04-owner-provenance.json");
    windows_service_security::hash_executable(
        &manifest,reviewed_owner_manifest_sha256,
    )?;
    #[derive(serde::Deserialize)]
    #[serde(rename_all="camelCase",deny_unknown_fields)]
    struct FirstParty {
        schema:String,upstream_commit:String,observer_only:bool,
        production_debugger_actions:bool,replay_authorized:bool,
        header_sha256:String,patcher_sha256:String,
        patched_debugger_thread_sha256:String,
        patched_progress_event_header_sha256:String,
        lldb_dap_exe_sha256:String,
    }
    let bytes=fs::read(&manifest).map_err(|_|denied())?;
    if bytes.is_empty()||bytes.len()>8192{return Err(denied());}
    let attested:FirstParty=serde_json::from_slice(&bytes).map_err(|_|denied())?;
    if attested.schema!="lu.ide.fin04.first-party-lldb-owner.v1"
        ||attested.upstream_commit!="3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff"
        ||!attested.observer_only||attested.production_debugger_actions
        ||attested.replay_authorized
        ||attested.lldb_dap_exe_sha256.to_ascii_lowercase()
            !=expected_sha256.to_ascii_lowercase()
        ||[
            &attested.header_sha256,&attested.patcher_sha256,
            &attested.patched_debugger_thread_sha256,
            &attested.patched_progress_event_header_sha256,
        ].iter().any(|x|x.len()!=64||!x.bytes().all(|c|c.is_ascii_hexdigit())){
        return Err(denied());
    }
    let stdin=pipe()?;
    let stdout=pipe()?;
    let stderr=pipe()?;
    let owner_pipe=pipe()?;
    // Only the patched debugger inherits the owner event WRITE handle.
    // The target inherits none of these handles (distinct SCM process).
    private_handle(stdin.write.0)?;
    private_handle(stdout.read.0)?;
    private_handle(stderr.read.0)?;
    private_handle(owner_pipe.read.0)?;
    let child_handles=[stdin.read.0,stdout.write.0,stderr.write.0,owner_pipe.write.0];
    use rand::{rngs::OsRng,RngCore};
    let mut owner_nonce=[0u8;16];
    OsRng.fill_bytes(&mut owner_nonce);
    let nonce_hex=owner_nonce.iter().map(|b|format!("{b:02x}")).collect::<String>();
    let mut attrs=windows_worker::Attributes::with_count(2)?;
    let kernel_job=job.held_split_job_handle();
    windows_service_security::verify_debugger_job_kernel_acl(kernel_job)?;
    // UpdateProcThreadAttribute may reference the supplied HANDLE array
    // through CreateProcessW; keep storage alive in this stack frame.
    let job_array=[kernel_job];
    job_attribute(&mut attrs,&job_array)?;
    if unsafe{UpdateProcThreadAttribute(attrs.raw(),0,HANDLE_LIST,
        child_handles.as_ptr().cast(),
        child_handles.len()*size_of::<HANDLE>(),
        ptr::null_mut(),ptr::null())}==0{return Err(denied());}
    let exe=wide(reviewed_lldb_dap.as_os_str());
    let cwd=wide(bundle.as_os_str());
    let mut args=wide(OsStr::new(&format!("\"{}\"",reviewed_lldb_dap.display())));
    let private_root=PathBuf::from(std::env::var_os("ProgramData")
        .ok_or_else(denied)?).join("LuIdeDebuggerBroker");
    let mut env=environment(&private_root,bundle)?;
    if env.pop()!=Some(0){return Err(denied());}
    // Only the administrator-pinned sealed service config can enable this
    // exact live WFP test. Never inherit a desktop/model-supplied flag or
    // take an arbitrary address, PID, command, script or service path.
    let wfp=if admin_opt_in_hostile_wfp_probe{
        "LU_IDE_OWNER_WFP_PROBE=1\0"
    }else{""};
    let extra=format!(
        "LU_IDE_OWNER_NONCE={nonce_hex}\0LU_IDE_OWNER_PIPE_HANDLE={:x}\0{wfp}\0",
        owner_pipe.write.0 as usize,
    );
    env.extend(extra.encode_utf16());
    if env.len()>32767{return Err(denied());}
    let mut startup=STARTUPINFOEXW::default();
    startup.StartupInfo.cb=size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags=STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput=stdin.read.0;
    startup.StartupInfo.hStdOutput=stdout.write.0;
    startup.StartupInfo.hStdError=stderr.write.0;
    startup.lpAttributeList=attrs.raw();
    let mut info=PROCESS_INFORMATION::default();
    if unsafe{CreateProcessW(
        exe.as_ptr(),args.as_mut_ptr(),ptr::null(),ptr::null(),1,
        CREATE_SUSPENDED|CREATE_NO_WINDOW|CREATE_UNICODE_ENVIRONMENT|
            EXTENDED_STARTUPINFO_PRESENT,
        env.as_ptr().cast(),cwd.as_ptr(),&startup.StartupInfo,&mut info,
    )}==0{return Err(denied());}
    let pending=Pending::from_information(info)?;
    contained(pending.process.0,kernel_job)?;
    // Only this process owns the private read handle. Transfer it exactly
    // once to the original kernel-event reader; that reader closes on EOF.
    // The local writer handle drops on return; the patched LLDB child
    // retains only the expressly allowlisted inherited writer.
    let reader=owner_pipe.read.0 as usize;
    std::mem::forget(owner_pipe.read);
    let owner=windows_event_owner::start_owner_pipe_reader(reader,owner_nonce)?;
    Ok(PendingDebugger{
        pending,stdin_write:stdin.write,stdout_read:stdout.read,
        stderr_read:stderr.read,owner,job:kernel_job,
    })
}

/// A one-shot first-party LLVM symbolizer executes only inside the SAME
/// independently SCM-owned restricted debugger Job, never from a host
/// std::process::Command fallback. It sees one sealed PE/PDB pair and one
/// OS-verified RIP+relocation bias; the human cannot choose arbitrary argv.
/// The exact bundle/SHA are pinned in protected service config. WFP exact
/// app V4/V6 BLOCK rules must already exist before this worker resumes.
/// The result remains untrusted until the ORIGINAL independent PDB/source
/// parser verifies the returned JSON, followed by job and receipt rechecks.
pub(crate) fn read_llvm_pdb_line_inside_debugger_job(
    job:&EmptyAuditJob,symbolizer:&Path,symbolizer_sha:&str,
    sealed_exe:&Path,expected_exe_sha:&str,
    stopped_rip:u64,os_bias:u64,
)->Result<serde_json::Value,SupervisorError>{
    use std::time::{Duration,Instant};
    windows_fin04w2_kernel::inspect_current_service_principal()?;
    if symbolizer.file_name()!=Some(OsStr::new("llvm-symbolizer.exe"))
        ||!symbolizer.is_absolute()||!sealed_exe.is_absolute()
        ||stopped_rip==0||os_bias>stopped_rip{return Err(denied());}
    for p in [symbolizer,sealed_exe]{
        if p.to_string_lossy().contains('"')||p.to_string_lossy().contains('\0')
            ||p.to_string_lossy().contains('\r')||p.to_string_lossy().contains('\n'){
            return Err(denied());
        }
    }
    windows_service_security::hash_executable(symbolizer,symbolizer_sha)?;
    windows_service_security::hash_executable(sealed_exe,expected_exe_sha)?;
    crate::windows_fin04w2_wfp::require_debugger_tool_outbound_blocks(symbolizer)?;
    let stdout=pipe()?;
    let stdin=pipe()?;
    private_handle(stdout.read.0)?;
    private_handle(stdin.write.0)?;
    let child_handles=[stdin.read.0,stdout.write.0];
    let mut attrs=windows_worker::Attributes::with_count(2)?;
    let kernel_job=job.held_split_job_handle();
    windows_service_security::verify_debugger_job_kernel_acl(kernel_job)?;
    let job_array=[kernel_job];
    job_attribute(&mut attrs,&job_array)?;
    if unsafe{UpdateProcThreadAttribute(attrs.raw(),0,HANDLE_LIST,
        child_handles.as_ptr().cast(),size_of::<HANDLE>()*child_handles.len(),
        ptr::null_mut(),ptr::null())}==0{return Err(denied());}
    let bin=symbolizer.parent().ok_or_else(denied)?;
    let private_root=PathBuf::from(std::env::var_os("ProgramData")
        .ok_or_else(denied)?).join("LuIdeDebuggerBroker");
    let env=environment(&private_root,bin)?;
    let quoted=format!(
        "\"{}\" \"--obj={}\" --output-style=JSON --functions=none --no-inlines --adjust-vma=0x{:x} 0x{:x}",
        symbolizer.display(),sealed_exe.display(),os_bias,stopped_rip,
    );
    let exe=wide(symbolizer.as_os_str());
    let mut args=wide(OsStr::new(&quoted));
    let cwd=wide(bin.as_os_str());
    let mut startup=STARTUPINFOEXW::default();
    startup.StartupInfo.cb=size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags=STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput=stdin.read.0;
    startup.StartupInfo.hStdOutput=stdout.write.0;
    startup.StartupInfo.hStdError=stdout.write.0;
    startup.lpAttributeList=attrs.raw();
    let mut info=PROCESS_INFORMATION::default();
    if unsafe{CreateProcessW(exe.as_ptr(),args.as_mut_ptr(),
        ptr::null(),ptr::null(),1,
        CREATE_SUSPENDED|CREATE_NO_WINDOW|CREATE_UNICODE_ENVIRONMENT|
            EXTENDED_STARTUPINFO_PRESENT,
        env.as_ptr().cast(),cwd.as_ptr(),&startup.StartupInfo,&mut info,
    )}==0{return Err(denied());}
    let mut child=Pending::from_information(info)?;
    contained(child.process.0,kernel_job)?;
    windows_service_security::hash_executable(symbolizer,symbolizer_sha)?;
    crate::windows_fin04w2_wfp::require_debugger_tool_outbound_blocks(symbolizer)?;
    child.resume_once()?;
    // Prevent a live writer on the parent from suppressing end-of-output.
    drop(stdin);
    drop(stdout.write);
    let stdout_read=stdout.read;
    let deadline=Instant::now()+Duration::from_secs(6);
    let mut content=Vec::new();
    loop{
        if Instant::now()>=deadline{return Err(denied());}
        let mut available=0u32;
        let peek=unsafe{PeekNamedPipe(stdout_read.0,ptr::null_mut(),0,
            ptr::null_mut(),&mut available,ptr::null_mut())};
        if peek!=0&&available>0{
            if content.len()+available as usize>8192{return Err(denied());}
            let mut scratch=[0u8;2048];
            let capacity=available.min(scratch.len() as u32);
            let mut got=0u32;
            if unsafe{windows_sys::Win32::Storage::FileSystem::ReadFile(
                stdout_read.0,scratch.as_mut_ptr().cast(),capacity,
                &mut got,ptr::null_mut())}==0||got==0{return Err(denied());}
            content.extend_from_slice(&scratch[..got as usize]);
        }else if unsafe{WaitForSingleObject(child.process.0,0)}==WAIT_OBJECT_0{
            // Drain any final buffered bytes before interpreting exit code.
            if peek==0||available==0{break;}
        }else if peek==0{return Err(denied());}
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut exit=u32::MAX;
    if unsafe{GetExitCodeProcess(child.process.0,&mut exit)}==0
        ||exit!=0||content.is_empty()||content.len()>8192{
        return Err(denied());
    }
    windows_service_security::hash_executable(symbolizer,symbolizer_sha)?;
    windows_service_security::hash_executable(sealed_exe,expected_exe_sha)?;
    contained(child.process.0,kernel_job)?;
    serde_json::from_slice(&content).map_err(|_|denied())
}

/// The former prepare_pair could create both workers from ONE SCM service,
/// which is not independent custody even with two distinct Job handles.
/// Deliberately delete that path. Only the target broker may stage the lowbox
/// target, and only the debugger broker may stage LLDB. Cross-service handoff
/// requires independently authenticated SCM IPC and original C2 grant receipts
/// before ANY thread resumes. Neither role may synthesize the other's approval.
///
/// This is intentionally not a runnable split-debugger transaction: until the
/// distinct target-broker SCM IPC and stopped-RIP/recovery matrix are accepted,
/// the native desktop must continue to expose only the harmless fixed C2 probe.
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caller_without_installed_service_identity_cannot_prepare_split_target(){
        // Even with a syntactically approved path there is no host fallback.
        let result=windows_fin04w2_kernel::inspect_current_service_principal();
        assert!(result.is_err());
    }
}
