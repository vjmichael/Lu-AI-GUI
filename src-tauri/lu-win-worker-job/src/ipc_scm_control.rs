//! LU-IPC-01D-R4-R3-R3-R4: SCM-only Windows private control transport.
//!
//! This is NOT the public lu.workspace.v0 endpoint. A protected installer
//! supplies exact pinned signed Lu Core executable, user SID and non-exported
//! per-user CNG command identity. The SCM compares BOTH the kernel pipe peer
//! and the signed immutable original command with a fresh SCM nonce. No
//! model/DLC can select the endpoint, worker binary, PID or operating token.
//!
//! Launch remains default-deny until an installed, separately reviewed fixed
//! AppContainer worker launcher and durable original-grant ledger exist.
//! Stop may only target an original service-held executor Job; a successful
//! ACK is not independently verifiable until the Core reopens the receipt.
#![cfg(windows)]
use std::{
    fs::File,io::{self,Read,Seek,SeekFrom},sync::atomic::{AtomicBool,Ordering},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt,AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions,NamedPipeClient,NamedPipeServer},
    time::timeout,
};
use crate::{
    ipc_credential_broker::require_system_service_context,
    ipc_installer_enrollment::{InstalledScmCorePins,read_installed_scm_core_pins,
        read_installed_scm_receipt_public_key},
    ipc_pipe_peer::{create_scm_control_pipe,observe_host_process,
        observe_connected_peer,WindowsPipePeer},
    ipc_scm_owned_jobs::{ScmJobKeeper,ServiceCallBinding,SupervisedJobLane},
    ipc_scm_receipts::{hash,persist_executor_stop,sign_core_control_request,
        verify_core_control_request,verify_executor_stop,
        read_executor_stop_bytes_for_control},
    ipc_signed_package::{inspect_signed_client},
    ipc_scm_control_wire::{ScmCommand,ScmControl,OriginalCall,
        CHALLENGE_BYTES,SIGNATURE_BYTES,MAX_CONTROL_BODY},
};
const PIPE:&str=r"\\.\pipe\LuCoreIpcScmControlV1";
const IO_DEADLINE:Duration=Duration::from_secs(3);
const SERVICE_TICK:Duration=Duration::from_millis(250);
const MAX_IMAGE:u64=128*1024*1024;
const MAX_SIGNED_RECEIPT:usize=2048;
const CORE_SERVICE_STOPPED:u8=2;
const CORE_SERVICE_READ_OK:u8=1;
const CORE_SERVICE_RECEIPT_EXISTS:u8=3;
const CORE_SERVICE_LAUNCHED:u8=4;

#[link(name="bcrypt")]
unsafe extern "system"{
    fn BCryptGenRandom(handle:*mut std::ffi::c_void,bytes:*mut u8,
        length:u32,flags:u32)->i32;
}
#[link(name="kernel32")]
unsafe extern "system" {
    fn GetNamedPipeServerProcessId(pipe:*mut std::ffi::c_void,pid:*mut u32)->i32;
}
fn denied(message:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::PermissionDenied,message)
}
fn random_challenge()->io::Result<[u8;CHALLENGE_BYTES]>{
    let mut bytes=[0u8;CHALLENGE_BYTES];
    // BCRYPT_USE_SYSTEM_PREFERRED_RNG=2: no private RNG state fallback.
    if unsafe{BCryptGenRandom(std::ptr::null_mut(),bytes.as_mut_ptr(),
        CHALLENGE_BYTES as u32,2)}!=0||bytes.iter().all(|b|*b==0){
        return Err(denied("OS challenge RNG unavailable"));
    }
    Ok(bytes)
}
fn digest_opened_file(original:&File)->io::Result<[u8;32]>{
    let mut file=original.try_clone()?;
    file.seek(SeekFrom::Start(0))?;
    let mut data=Vec::new();
    file.take(MAX_IMAGE+1).read_to_end(&mut data)?;
    if data.is_empty()||data.len() as u64>MAX_IMAGE{
        return Err(denied("enrolled Core signed image exceeds size limit"));
    }
    hash(&data)
}
fn original_binding(call:&OriginalCall)->io::Result<ServiceCallBinding>{
    ServiceCallBinding::checked(
        &call.call_id,&call.fingerprint_sha256,&call.module_id,
        &call.session_ref,&call.core_epoch_ref,&call.conversation_ref,
        &call.user_turn_ref,&call.task_ref,&call.workspace_ref)
}
fn signed_kernel_peer(
    pipe:&NamedPipeServer,pins:&InstalledScmCorePins,
)->io::Result<crate::ipc_signed_package::SignedClientLease>{
    let peer=observe_connected_peer(pipe)?;
    if peer.user_sid!=pins.user_sid||peer.session_id==0
       ||peer.image_path!=pins.image_path{
        return Err(denied("SCM command peer not original installed Lu Core"));
    }
    // Must follow the last complete ReadFile of this very pipe. This holds
    // the real process handle and signed executable through dispatch.
    let lease=inspect_signed_client(pipe,&pins.image_path)?;
    if lease.witness()!=&peer{
        return Err(denied("SCM command peer generation changed"));
    }
    if digest_opened_file(lease.image_file())?!=pins.image_sha256 ||
       hash(lease.signer_certificate_der())?!=pins.signer_sha256{
        return Err(denied("SCM command executable or publisher pin mismatch"));
    }
    lease.check_alive()?;
    Ok(lease)
}
async fn read_signed_packet(pipe:&mut NamedPipeServer)->io::Result<(Vec<u8>,[u8;SIGNATURE_BYTES])>{
    let mut length=[0u8;4];
    timeout(IO_DEADLINE,pipe.read_exact(&mut length)).await
        .map_err(|_|denied("SCM request header timeout"))??;
    let size=u32::from_be_bytes(length) as usize;
    if size==0||size>MAX_CONTROL_BODY {
        return Err(denied("SCM command body exceeds fixed bound"));
    }
    let mut body=vec![0u8;size];
    let mut signature=[0u8;SIGNATURE_BYTES];
    timeout(IO_DEADLINE,async{
        pipe.read_exact(&mut body).await?;
        pipe.read_exact(&mut signature).await
    }).await.map_err(|_|denied("SCM request body timeout"))??;
    Ok((body,signature))
}
async fn serve_one(
    pipe:&mut NamedPipeServer,keeper:&ScmJobKeeper,pins:&InstalledScmCorePins
)->io::Result<()>{
    require_system_service_context()?;
    let service=observe_host_process(std::process::id())?;
    if service.user_sid!="S-1-5-18" || service.session_id!=0{
        return Err(denied("private SCM service is not unimpersonated SYSTEM"));
    }
    let nonce=random_challenge()?;
    timeout(IO_DEADLINE,pipe.write_all(&nonce)).await
        .map_err(|_|denied("SCM challenge write timeout"))??;
    let (body,signature)=read_signed_packet(pipe).await?;
    let lease=signed_kernel_peer(pipe,pins)?;
    verify_core_control_request(&body,&signature)?;
    let command=ScmControl::parse_exact(
        &body,nonce,service.pid,service.creation_filetime)
        .map_err(|_|denied("SCM signed command refused by canonical parser"))?;
    lease.check_alive()?;
    let mut transmitted_receipt=None;
    let result=match command.command{
        ScmCommand::ReadStatus=>CORE_SERVICE_READ_OK,
        ScmCommand::LaunchFixedExecutor=>{
            let binding=original_binding(command.original.as_ref()
                .ok_or_else(||denied("SCM launch original call absent"))?)?;
            // This accepts no executable/argv or PID from the request.
            // A missing approved fixed worker/AppContainer profile fails
            // before any worker resumes. The SCM retains the only Job.
            keeper.launch_fixed_for_service(binding,lease.witness().pid)?;
            CORE_SERVICE_LAUNCHED
        }
        ScmCommand::StopExactExecutor=>{
            let binding=original_binding(command.original.as_ref()
                .ok_or_else(||denied("SCM stop original call absent"))?)?;
            // Only the original SCM-held Job may produce an actual kernel
            // empty proof. Persistence can fail; then no ACK is emitted.
            let stopped=keeper.stop_exact_for_service(
                &binding,SupervisedJobLane::CoreExecutor)?;
            persist_executor_stop(&stopped)?;
            CORE_SERVICE_STOPPED
        }
        ScmCommand::InspectStoppedExecutor=>{
            let binding=original_binding(command.original.as_ref()
                .ok_or_else(||denied("SCM receipt original call absent"))?)?;
            let pin=read_installed_scm_receipt_public_key()?;
            verify_executor_stop(&binding,&pin)?;
            transmitted_receipt=Some(read_executor_stop_bytes_for_control(&binding)?);
            CORE_SERVICE_RECEIPT_EXISTS
        }
    };
    lease.check_alive()?;
    // This 1-byte status is NOT a signed receipt or an execution grant.
    timeout(IO_DEADLINE,async{
        pipe.write_all(&[result]).await?;
        if let Some(receipt)=transmitted_receipt{
            let size=u32::try_from(receipt.len())
                .map_err(|_|denied("SCM receipt wire too large"))?;
            if size==0||size as usize>MAX_SIGNED_RECEIPT{
                return Err(denied("SCM receipt wire not bounded"));
            }
            pipe.write_all(&size.to_be_bytes()).await?;
            pipe.write_all(&receipt).await?;
        }
        pipe.flush().await
    }).await.map_err(|_|denied("SCM response timeout"))??;
    Ok(())
}
/// Actual installed SCM-only listener. No model, DLC or network endpoint.
/// Every connection is single-command, nonce-challenged, bounded, and the
/// SCM service continues only when the protected installed configuration is
/// present. A malformed peer is disconnected without a success response.
pub fn serve_control<F:FnOnce()->io::Result<()>>(
    keeper:&ScmJobKeeper,stop:&AtomicBool,on_ready:F,
)->io::Result<()>{
    require_system_service_context()?;
    let pins=read_installed_scm_core_pins()?;
    // Enforce installed public verification key and exact protected Core
    // peer configuration BEFORE advertising SERVICE_RUNNING.
    crate::ipc_installer_enrollment::
        read_installed_scm_core_command_public_key()?;
    let rt=tokio::runtime::Builder::new_current_thread()
        .enable_all().build()?;
    let mut ready=Some(on_ready);
    rt.block_on(async{
        while !stop.load(Ordering::Acquire){
            // A crashed/replaced Core loses its SCM worker by OS observation;
            // never infer original process liveness from an IPC socket alone.
            for stopped in keeper.stop_orphaned_core_executors()?{
                persist_executor_stop(&stopped)?;
            }
            let mut pipe=create_scm_control_pipe(&pins.user_sid)?;
            if let Some(report_ready)=ready.take(){report_ready()?;}
            match timeout(SERVICE_TICK,pipe.connect()).await{
                Ok(Ok(()))=>{
                    // One hostile client can occupy only this bounded time.
                    // Refusals never authorize a worker or stop another Job.
                    let _=serve_one(&mut pipe,keeper,&pins).await;
                }
                Ok(Err(err))=>return Err(err),
                Err(_)=>(), // drop pending connect and check SCM STOP
            }
        }
        Ok(())
    })
}
fn observe_server(client:&NamedPipeClient,expected:&WindowsPipePeer)->io::Result<()>{
    use std::os::windows::io::AsRawHandle;
    let mut pid=0u32;
    if unsafe{GetNamedPipeServerProcessId(
        client.as_raw_handle().cast(),&mut pid)}==0||pid!=expected.pid{
        return Err(denied("SCM control named-pipe owner changed"));
    }
    let actual=observe_host_process(pid)?;
    if actual!=*expected ||actual.user_sid!="S-1-5-18"||
        actual.session_id!=0 {
        return Err(denied("SCM control endpoint process epoch mismatch"));
    }
    Ok(())
}
/// Core-side private transport client. Caller MUST hold an independently
/// verified exact SCM signed-image lease across this call. This function
/// never receives a DLC WorkspaceFrame and never binds effects on its own.
/// The response itself is not an authority token. Only the exact signed
/// executor receipt bytes, independently verified by Core against its
/// separately installer-pinned key, may enter durable reconciliation.
pub struct ScmControlResponse{
    pub code:u8,
    pub signed_receipt:Option<Vec<u8>>,
}
pub async fn issue_core_command(
    command:&ScmControl,expected_scm:&WindowsPipePeer
)->io::Result<ScmControlResponse>{
    if command.service_pid!=expected_scm.pid ||
       command.service_birth!=expected_scm.creation_filetime{
        return Err(denied("stale Core-to-SCM service epoch"));
    }
    let mut pipe=ClientOptions::new().open(PIPE)?;
    observe_server(&pipe,expected_scm)?;
    let mut challenge=[0u8;CHALLENGE_BYTES];
    timeout(IO_DEADLINE,pipe.read_exact(&mut challenge)).await
        .map_err(|_|denied("SCM challenge timeout"))??;
    let body=command.canonical(challenge)
        .map_err(|_|denied("invalid original Core control binding"))?;
    let signature=sign_core_control_request(&body)?;
    observe_server(&pipe,expected_scm)?;
    timeout(IO_DEADLINE,async{
        pipe.write_all(&(body.len() as u32).to_be_bytes()).await?;
        pipe.write_all(&body).await?;
        pipe.write_all(&signature).await?;
        pipe.flush().await
    }).await.map_err(|_|denied("SCM signed command transmission timeout"))??;
    let mut result=[0u8;1];
    timeout(IO_DEADLINE,pipe.read_exact(&mut result)).await
        .map_err(|_|denied("SCM signed response timeout"))??;
    let signed_receipt=if command.command==ScmCommand::InspectStoppedExecutor{
        let mut head=[0u8;4];
        timeout(IO_DEADLINE,pipe.read_exact(&mut head)).await
            .map_err(|_|denied("SCM signed receipt header timeout"))??;
        let size=u32::from_be_bytes(head) as usize;
        if size==0||size>MAX_SIGNED_RECEIPT{
            return Err(denied("SCM receipt response oversized"));
        }
        let mut receipt=vec![0u8;size];
        timeout(IO_DEADLINE,pipe.read_exact(&mut receipt)).await
            .map_err(|_|denied("SCM receipt response timeout"))??;
        Some(receipt)
    }else{None};
    observe_server(&pipe,expected_scm)?;
    if !matches!((command.command,result[0]),
        (ScmCommand::ReadStatus,CORE_SERVICE_READ_OK)|
        (ScmCommand::LaunchFixedExecutor,CORE_SERVICE_LAUNCHED)|
        (ScmCommand::StopExactExecutor,CORE_SERVICE_STOPPED)|
        (ScmCommand::InspectStoppedExecutor,CORE_SERVICE_RECEIPT_EXISTS)){
        return Err(denied("SCM private reply mismatched command"));
    }
    Ok(ScmControlResponse{code:result[0],signed_receipt})
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn private_pipe_is_not_any_dlc_endpoint(){
        assert_ne!(PIPE,r"\\.\pipe\lu-ipc-00000000000000000000000000000000");
        assert!(PIPE.starts_with(r"\\.\pipe\LuCoreIpcScmControl"));
    }
    #[test]
    fn uninstalled_ordinary_user_cannot_open_scm_control(){
        let peer=observe_host_process(std::process::id()).unwrap();
        if peer.user_sid!="S-1-5-18"{
            assert!(read_installed_scm_core_pins().is_err());
            assert!(create_scm_control_pipe("invalid").is_err());
        }
    }
}
