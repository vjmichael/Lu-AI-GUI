//! LU-IPC-01D-R4-R3-R3-R3: durable SCM-origin executor-stop evidence.
//!
//! A signed receipt is only minted from an actual SCM-held Job-empty witness;
//! it is NOT a grant to execute an effect, and it never marks uncertain effects
//! successful. Installer must provision the non-exportable LocalMachine CNG
//! ECDSA P-256 key and a SYSTEM-writable, protected, non-reparse receipt root.
//! No fallback key, auto-key creation, user-supplied path or user-mode signer.
#![cfg(windows)]

use std::{
    ffi::c_void, fs::{self,File,OpenOptions},
    io::{self,Read,Write}, os::windows::ffi::OsStrExt,
    path::PathBuf,ptr,
};
use crate::{
    ipc_credential_broker::require_system_service_context,
    ipc_installer_enrollment::protected_executor_receipts_root,
    ipc_scm_owned_jobs::{ServiceCallBinding,ServiceStoppedJob,SupervisedJobLane},
};

type Handle=*mut c_void;
const KEY_NAME:&str="LuCoreIpcExecutorStopP256V1";
const KSP:&str="Microsoft Software Key Storage Provider";
const DOMAIN:&[u8]=b"LU_SCM_EXECUTOR_STOP_RECEIPT_V1";
const MAX_RECORD:usize=2048;
const PUBLIC_BLOB_BYTES:usize=72;
const SIGNATURE_BYTES:usize=64;
const NCRYPT_MACHINE_KEY_FLAG:u32=0x20;
const MOVEFILE_WRITE_THROUGH:u32=0x8;
const ECC_P256_PUBLIC_MAGIC:u32=0x31534345; // BCRYPT_ECDSA_PUBLIC_P256_MAGIC

#[link(name="ncrypt")]
unsafe extern "system" {
    fn NCryptOpenStorageProvider(provider:*mut Handle,name:*const u16,flags:u32)->i32;
    fn NCryptOpenKey(provider:Handle,key:*mut Handle,name:*const u16,
        legacy_spec:u32,flags:u32)->i32;
    fn NCryptExportKey(key:Handle,export:Handle,kind:*const u16,
        parameters:Handle,buffer:*mut u8,size:u32,used:*mut u32,flags:u32)->i32;
    fn NCryptSignHash(key:Handle,padding:Handle,hash:*const u8,
        hash_len:u32,signature:*mut u8,signature_len:u32,used:*mut u32,flags:u32)->i32;
    fn NCryptFreeObject(handle:Handle)->i32;
}
#[link(name="bcrypt")]
unsafe extern "system" {
    fn BCryptOpenAlgorithmProvider(algorithm:*mut Handle,name:*const u16,
        implementation:*const u16,flags:u32)->i32;
    fn BCryptHash(algorithm:Handle,secret:*const u8,secret_len:u32,
        input:*const u8,input_len:u32,output:*mut u8,output_len:u32)->i32;
    fn BCryptImportKeyPair(algorithm:Handle,import:Handle,kind:*const u16,
        key:*mut Handle,blob:*const u8,blob_len:u32,flags:u32)->i32;
    fn BCryptVerifySignature(key:Handle,padding:Handle,hash:*const u8,
        hash_len:u32,signature:*const u8,signature_len:u32,flags:u32)->i32;
    fn BCryptDestroyKey(key:Handle)->i32;
    fn BCryptCloseAlgorithmProvider(algorithm:Handle,flags:u32)->i32;
}
#[link(name="kernel32")]
unsafe extern "system" {
    fn MoveFileExW(existing:*const u16,new:*const u16,flags:u32)->i32;
}
fn wide(value:&std::ffi::OsStr)->Vec<u16>{
    value.encode_wide().chain(std::iter::once(0)).collect()
}
fn wide_str(value:&str)->Vec<u16>{wide(std::ffi::OsStr::new(value))}
fn refused(message:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::PermissionDenied,message)
}
fn invalid(message:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::InvalidData,message)
}
struct Ncrypt(Handle);
impl Drop for Ncrypt{
    fn drop(&mut self){if !self.0.is_null(){unsafe{NCryptFreeObject(self.0)};}}
}
struct BCryptAlg(Handle);
impl Drop for BCryptAlg{
    fn drop(&mut self){if !self.0.is_null(){unsafe{BCryptCloseAlgorithmProvider(self.0,0)};}}
}
struct BCryptKey(Handle);
impl Drop for BCryptKey{
    fn drop(&mut self){if !self.0.is_null(){unsafe{BCryptDestroyKey(self.0)};}}
}
pub(crate) fn hash(data:&[u8])->io::Result<[u8;32]>{
    let mut raw=ptr::null_mut();
    if unsafe{BCryptOpenAlgorithmProvider(
        &mut raw,wide_str("SHA256").as_ptr(),ptr::null(),0)}!=0 ||raw.is_null(){
        return Err(refused("CNG SHA-256 unavailable"));
    }
    let provider=BCryptAlg(raw);
    let mut digest=[0u8;32];
    let len=u32::try_from(data.len()).map_err(|_|invalid("oversized receipt"))?;
    if unsafe{BCryptHash(provider.0,ptr::null(),0,
        data.as_ptr(),len,digest.as_mut_ptr(),32)}!=0{
        return Err(refused("CNG receipt digest failed"));
    }
    Ok(digest)
}
fn open_installed_signer()->io::Result<(Ncrypt,Ncrypt)>{
    require_system_service_context()?;
    let mut raw=ptr::null_mut();
    if unsafe{NCryptOpenStorageProvider(&mut raw,wide_str(KSP).as_ptr(),0)}!=0
        ||raw.is_null(){return Err(refused("SCM receipt key provider unavailable"));}
    let provider=Ncrypt(raw);
    let mut key=ptr::null_mut();
    if unsafe{NCryptOpenKey(provider.0,&mut key,wide_str(KEY_NAME).as_ptr(),
        0,NCRYPT_MACHINE_KEY_FLAG)}!=0||key.is_null(){
        return Err(refused("installer-provisioned machine receipt key unavailable"));
    }
    Ok((provider,Ncrypt(key)))
}
fn installed_public_blob(key:Handle)->io::Result<[u8;PUBLIC_BLOB_BYTES]>{
    let mut blob=[0u8;PUBLIC_BLOB_BYTES];
    let mut used=0u32;
    if unsafe{NCryptExportKey(key,ptr::null_mut(),wide_str("ECCPUBLICBLOB").as_ptr(),
        ptr::null_mut(),blob.as_mut_ptr(),blob.len() as u32,&mut used,0)}!=0
        ||used as usize!=blob.len()||!valid_public_blob(&blob){
        return Err(refused("unexpected installed CNG receipt public key"));
    }
    Ok(blob)
}
fn valid_public_blob(blob:&[u8])->bool{
    blob.len()==PUBLIC_BLOB_BYTES
      &&u32::from_le_bytes(blob[0..4].try_into().unwrap())==ECC_P256_PUBLIC_MAGIC
      &&u32::from_le_bytes(blob[4..8].try_into().unwrap())==32
      &&blob[8..].iter().any(|byte|*byte!=0)
}
/// Only the protected installer may publish and pin this public key. This
/// accessor does not create a signing key and never exposes private material.
pub fn installed_public_key_for_protected_installer()->io::Result<[u8;PUBLIC_BLOB_BYTES]>{
    let (_provider,key)=open_installed_signer()?;
    installed_public_blob(key.0)
}
fn sign(body:&[u8])->io::Result<[u8;SIGNATURE_BYTES]>{
    let (_provider,key)=open_installed_signer()?;
    let digest=hash(body)?;
    let mut signature=[0u8;SIGNATURE_BYTES];
    let mut used=0u32;
    if unsafe{NCryptSignHash(key.0,ptr::null_mut(),digest.as_ptr(),32,
        signature.as_mut_ptr(),signature.len() as u32,&mut used,0)}!=0
        ||used as usize!=signature.len(){
        return Err(refused("installed receipt ECDSA signing failed"));
    }
    Ok(signature)
}
pub(crate) fn verify(body:&[u8],signature:&[u8],approved_public_blob:&[u8])->io::Result<()>{
    if !valid_public_blob(approved_public_blob)||signature.len()!=SIGNATURE_BYTES{
        return Err(refused("missing independently approved receipt verification pin"));
    }
    let digest=hash(body)?;
    let mut raw=ptr::null_mut();
    if unsafe{BCryptOpenAlgorithmProvider(
        &mut raw,wide_str("ECDSA_P256").as_ptr(),ptr::null(),0)}!=0
        ||raw.is_null(){return Err(refused("CNG receipt verifier unavailable"));}
    let provider=BCryptAlg(raw);
    let mut key=ptr::null_mut();
    if unsafe{BCryptImportKeyPair(provider.0,ptr::null_mut(),
        wide_str("ECCPUBLICBLOB").as_ptr(),&mut key,approved_public_blob.as_ptr(),
        approved_public_blob.len() as u32,0)}!=0||key.is_null(){
        return Err(refused("approved receipt public key import failed"));
    }
    let key=BCryptKey(key);
    if unsafe{BCryptVerifySignature(key.0,ptr::null_mut(),
        digest.as_ptr(),32,signature.as_ptr(),signature.len() as u32,0)}!=0{
        return Err(refused("SCM receipt signature invalid"));
    }
    Ok(())
}
/// Verify an unrelated installed NVP GPU-stop witness against its OWN
/// protected installer public pin. This function does not choose a key,
/// grant access, or permit the NVP SCM to sign a Lu executor receipt.
pub fn verify_external_p256_signature(
    body:&[u8],signature:&[u8],approved_public_blob:&[u8],
)->io::Result<()>{
    verify(body,signature,approved_public_blob)
}
/// A separate pre-provisioned, per-user NON-SYSTEM CNG identity for the
/// installed Lu Core executable. A signed request remains subject to OS
/// process/file/publisher checks on the service-held named pipe. Installer
/// must seal this user's key and the SCM-owned matching PUBLIC pin.
pub fn sign_core_control_request(body:&[u8])->io::Result<[u8;SIGNATURE_BYTES]>{
    use crate::ipc_pipe_peer::observe_host_process;
    if observe_host_process(std::process::id())?.user_sid=="S-1-5-18"{
        return Err(refused("LocalSystem cannot impersonate a Core control caller"));
    }
    let mut raw=ptr::null_mut();
    if unsafe{NCryptOpenStorageProvider(&mut raw,wide_str(KSP).as_ptr(),0)}!=0
        ||raw.is_null(){return Err(refused("Core CNG provider unavailable"));}
    let provider=Ncrypt(raw);
    let mut key=ptr::null_mut();
    if unsafe{NCryptOpenKey(provider.0,&mut key,
        wide_str("LuCoreIpcCommandP256V1").as_ptr(),0,0)}!=0||key.is_null(){
        return Err(refused("installed Core per-user control key unavailable"));
    }
    let key=Ncrypt(key);
    let digest=hash(body)?;
    let mut signature=[0u8;SIGNATURE_BYTES];
    let mut used=0u32;
    if unsafe{NCryptSignHash(key.0,ptr::null_mut(),digest.as_ptr(),32,
        signature.as_mut_ptr(),SIGNATURE_BYTES as u32,&mut used,0)}!=0
        ||used as usize!=SIGNATURE_BYTES{
        return Err(refused("Core control signature failed"));
    }
    Ok(signature)
}
/// SCM service only: verify independently provisioned Core key, never a
/// certificate/public key from the incoming packet. Not a tool grant.
pub fn verify_core_control_request(body:&[u8],signature:&[u8])->io::Result<()>{
    require_system_service_context()?;
    let trusted=crate::ipc_installer_enrollment::
        read_installed_scm_core_command_public_key()?;
    verify(body,signature,&trusted)
}

fn lane_byte(lane:SupervisedJobLane)->u8{
    match lane{SupervisedJobLane::DlcClient=>1,SupervisedJobLane::CoreExecutor=>2}
}
fn push_scope(out:&mut Vec<u8>,value:&str)->io::Result<()>{
    if value.is_empty()||value.len()>128||value.chars().any(char::is_control){
        return Err(invalid("invalid SCM receipt scope"));
    }
    out.push(value.len() as u8);
    out.extend_from_slice(value.as_bytes());
    Ok(())
}
fn body_of(stop:&ServiceStoppedJob)->io::Result<Vec<u8>>{
    let binding=stop.binding();
    let mut out=DOMAIN.to_vec();
    out.push(1); // receipt wire version
    out.push(lane_byte(stop.lane()));
    for scope in [
        binding.call_id(),binding.fingerprint_sha256(),binding.module_id(),
        binding.session_ref(),binding.core_epoch_ref(),binding.conversation_ref(),
        binding.user_turn_ref(),binding.task_ref(),binding.workspace_ref(),
    ]{push_scope(&mut out,scope)?;}
    for value in [u64::from(stop.original_pid()),stop.original_creation_filetime(),
        u64::from(stop.supervisor_pid()),stop.supervisor_creation_filetime()]{
        if value==0{return Err(invalid("zero SCM OS process generation"));}
        out.extend_from_slice(&value.to_le_bytes());
    }
    if stop.method()!="WINDOWS_JOB_ACTIVE_ZERO_V1"{
        return Err(invalid("unrecognized kernel Job-empty witness"));
    }
    out.push(1); // method discriminator (never a caller supplied string)
    if out.len()+SIGNATURE_BYTES>MAX_RECORD{return Err(invalid("oversized SCM receipt"));}
    Ok(out)
}
fn read_scope<'a>(body:&'a[u8],offset:&mut usize)->io::Result<&'a str>{
    let size=*body.get(*offset).ok_or_else(||invalid("truncated receipt scope"))? as usize;
    *offset+=1;
    if size==0||size>128||*offset+size>body.len(){
        return Err(invalid("invalid receipt scope length"));
    }
    let part=std::str::from_utf8(&body[*offset..*offset+size])
        .map_err(|_|invalid("receipt scope is not UTF-8"))?;
    if part.chars().any(char::is_control){return Err(invalid("receipt scope control"));}
    *offset+=size;
    Ok(part)
}
/// Verified evidence only. It cannot be used as a token, a tool authorization,
/// or an automatic transition from UNCERTAIN to successful execution.
#[derive(Debug)]
pub struct VerifiedExecutorStopReceipt{
    supervisor_pid:u32,
    supervisor_creation_filetime:u64,
    worker_pid:u32,
    worker_creation_filetime:u64,
}
impl VerifiedExecutorStopReceipt{
    pub fn supervisor_pid(&self)->u32{self.supervisor_pid}
    pub fn supervisor_creation_filetime(&self)->u64{self.supervisor_creation_filetime}
    pub fn worker_pid(&self)->u32{self.worker_pid}
    pub fn worker_creation_filetime(&self)->u64{self.worker_creation_filetime}
}
fn verify_exact_body(body:&[u8],binding:&ServiceCallBinding)->io::Result<VerifiedExecutorStopReceipt>{
    if !body.starts_with(DOMAIN){return Err(invalid("receipt domain mismatch"));}
    let mut at=DOMAIN.len();
    if body.get(at)!=Some(&1)||body.get(at+1)!=Some(&2){
        return Err(invalid("receipt is not an executor stop"));
    }
    at+=2;
    for expected in [
        binding.call_id(),binding.fingerprint_sha256(),binding.module_id(),
        binding.session_ref(),binding.core_epoch_ref(),binding.conversation_ref(),
        binding.user_turn_ref(),binding.task_ref(),binding.workspace_ref(),
    ]{
        if read_scope(body,&mut at)?!=expected{
            return Err(refused("SCM receipt original-call binding mismatch"));
        }
    }
    let mut observed=[0u64;4];
    for value in &mut observed{
        let field=body.get(at..at+8).ok_or_else(||invalid("truncated SCM generations"))?;
        *value=u64::from_le_bytes(field.try_into().unwrap());
        if *value==0{return Err(invalid("zero SCM receipt generation"));}
        at+=8;
    }
    if body.get(at)!=Some(&1)||at+1!=body.len(){
        return Err(invalid("invalid SCM proof method or appended bytes"));
    }
    let worker_pid=u32::try_from(observed[0]).map_err(|_|invalid("worker PID overflow"))?;
    let supervisor_pid=u32::try_from(observed[2]).map_err(|_|invalid("supervisor PID overflow"))?;
    Ok(VerifiedExecutorStopReceipt{
        worker_pid,worker_creation_filetime:observed[1],
        supervisor_pid,supervisor_creation_filetime:observed[3],
    })
}
fn receipt_path(call_id:&str)->io::Result<PathBuf>{
    if call_id.len()!=64 ||!call_id.bytes().all(|b|b.is_ascii_hexdigit() && !b.is_ascii_uppercase()){
        return Err(refused("unsafe receipt call filename"));
    }
    Ok(protected_executor_receipts_root()?.join(format!("{call_id}.executor.stop.v1")))
}
fn protected_root()->io::Result<()>{
    protected_executor_receipts_root().map(|_|())
}
/// Service-signed, one-use original-call admission intent. Saved BEFORE
/// native CreateProcessAsUserW resumes the fixed executor. An intent without
/// a later signed kernel stop is NOT executor-stop evidence; it forces manual
/// uncertain-outcome reconciliation after a service crash.
pub fn persist_executor_admission_intent(binding:&ServiceCallBinding)->io::Result<PathBuf>{
    require_system_service_context()?;
    protected_root()?;
    let mut body=b"LU_SCM_EXECUTOR_ADMISSION_INTENT_V1\0".to_vec();
    body.push(1);
    for item in [
        binding.call_id(),binding.fingerprint_sha256(),binding.module_id(),
        binding.session_ref(),binding.core_epoch_ref(),
        binding.conversation_ref(),binding.user_turn_ref(),
        binding.task_ref(),binding.workspace_ref()
    ]{push_scope(&mut body,item)?;}
    let current=crate::ipc_pipe_peer::observe_host_process(std::process::id())?;
    if current.user_sid!="S-1-5-18"||current.session_id!=0||
        current.creation_filetime==0{
        return Err(refused("SCM admission intent missing original service generation"));
    }
    body.extend_from_slice(&u64::from(current.pid).to_le_bytes());
    body.extend_from_slice(&current.creation_filetime.to_le_bytes());
    if body.len()+SIGNATURE_BYTES>MAX_RECORD{
        return Err(invalid("SCM admission intent oversized"));
    }
    let sig=sign(&body)?;
    let directory=protected_executor_receipts_root()?;
    let path=directory.join(format!("{}.executor.admit.v1",binding.call_id()));
    if path.exists(){return Err(refused("duplicate SCM executor admission intent"));}
    let temporary=directory.join(format!(
        "{}.executor.admit.pending.{}.{}",
        binding.call_id(),current.pid,current.creation_filetime));
    let mut file=OpenOptions::new().create_new(true).write(true).open(&temporary)?;
    let wrote=(||->io::Result<()>{
        file.write_all(&body)?;
        file.write_all(&sig)?;
        file.sync_all()?;
        Ok(())
    })();
    drop(file);
    if let Err(error)=wrote{
        let _=fs::remove_file(&temporary);
        return Err(error);
    }
    if unsafe{MoveFileExW(wide(temporary.as_os_str()).as_ptr(),
        wide(path.as_os_str()).as_ptr(),MOVEFILE_WRITE_THROUGH)}==0{
        let error=io::Error::last_os_error();
        let _=fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(path)
}

/// Service-only, after OS-confirmed executor Job termination. A failure to
/// flush or atomically rename is a FAILED receipt, never a successful stop.
pub fn persist_executor_stop(stop:&ServiceStoppedJob)->io::Result<PathBuf>{
    require_system_service_context()?;
    if stop.lane()!=SupervisedJobLane::CoreExecutor{
        return Err(refused("DLC client Job cannot mint executor stop receipt"));
    }
    protected_root()?;
    let body=body_of(stop)?;
    let sig=sign(&body)?;
    let path=receipt_path(stop.binding().call_id())?;
    if path.exists(){return Err(refused("duplicate SCM executor receipt"));}
    let temporary=path.with_extension(format!(
        "pending.{}.{}",stop.supervisor_pid(),stop.supervisor_creation_filetime()));
    let mut file=OpenOptions::new().write(true).create_new(true).open(&temporary)?;
    let write_result=(||->io::Result<()>{
        file.write_all(&body)?;
        file.write_all(&sig)?;
        file.sync_all()?;
        Ok(())
    })();
    drop(file);
    if let Err(error)=write_result{
        let _=fs::remove_file(&temporary);
        return Err(error);
    }
    if unsafe{MoveFileExW(wide(temporary.as_os_str()).as_ptr(),
        wide(path.as_os_str()).as_ptr(),MOVEFILE_WRITE_THROUGH)}==0{
        let err=io::Error::last_os_error();
        let _=fs::remove_file(&temporary);
        return Err(err);
    }
    Ok(path)
}
/// SCM-only retrieval from a fixed, protected receipt path. This is raw
/// signed material; the receiving Core MUST independently verify the
/// signature against its installer-pinned public key and exact original call.
pub fn read_executor_stop_bytes_for_control(original:&ServiceCallBinding)
    ->io::Result<Vec<u8>>{
    require_system_service_context()?;
    let path=receipt_path(original.call_id())?;
    let file=File::open(path)?;
    if file.metadata()?.len()>MAX_RECORD as u64{
        return Err(invalid("oversized SCM receipt"));
    }
    let mut bytes=Vec::new();
    file.take((MAX_RECORD+1) as u64).read_to_end(&mut bytes)?;
    if bytes.len()<DOMAIN.len()+SIGNATURE_BYTES||bytes.len()>MAX_RECORD{
        return Err(invalid("invalid SCM receipt"));
    }
    Ok(bytes)
}
/// Independently verify bytes received over an authenticated Core <-> SCM
/// pipe; no local SYSTEM-only receipt-file read is required by the user Core.
/// PIN MUST be the signed Core install's pinned SCM public verification key.
pub fn verify_executor_stop_bytes(original:&ServiceCallBinding,
    approved_public_blob:&[u8],record:&[u8])
    ->io::Result<VerifiedExecutorStopReceipt>{
    if record.len()<DOMAIN.len()+SIGNATURE_BYTES||record.len()>MAX_RECORD{
        return Err(invalid("SCM receipt wire bound invalid"));
    }
    let len=record.len()-SIGNATURE_BYTES;
    verify(&record[..len],&record[len..],approved_public_blob)?;
    verify_exact_body(&record[..len],original)
}
/// Independent verification: the public CNG key must come from a separately
/// approved protected installer record, never from the receipt or the pipe.
/// Read canonical exact-call file only, with no arbitrary filename/PID lookup.
pub fn verify_executor_stop(
    original:&ServiceCallBinding,approved_public_blob:&[u8],
)->io::Result<VerifiedExecutorStopReceipt>{
    let record=read_executor_stop_bytes_for_control(original)?;
    verify_executor_stop_bytes(original,approved_public_blob,&record)
}

#[cfg(test)]
mod tests{
    use super::*;
    fn binding()->ServiceCallBinding{
        ServiceCallBinding::checked(&"a".repeat(64),&"b".repeat(64),
            "lu.ide","session","epoch","conversation","turn","task","workspace").unwrap()
    }
    #[test]
    fn malformed_and_client_only_receipts_are_refused(){
        let b=binding();
        assert!(verify_exact_body(b"", &b).is_err());
        let mut bytes=DOMAIN.to_vec();
        bytes.extend_from_slice(&[1,1]); // DLC lane is not executor
        assert!(verify_exact_body(&bytes,&b).is_err());
        bytes[DOMAIN.len()+1]=2;
        assert!(verify_exact_body(&bytes,&b).is_err());
        assert!(receipt_path("../evil").is_err());
    }
    #[test]
    fn canonical_executor_body_rejects_wrong_turn_lane_method_and_extra_bytes(){
        let b=binding();
        let mut body=DOMAIN.to_vec();
        body.extend_from_slice(&[1,2]);
        for field in [b.call_id(),b.fingerprint_sha256(),b.module_id(),
            b.session_ref(),b.core_epoch_ref(),b.conversation_ref(),
            b.user_turn_ref(),b.task_ref(),b.workspace_ref()]{
            push_scope(&mut body,field).unwrap();
        }
        for value in [333u64,444,555,666]{body.extend_from_slice(&value.to_le_bytes());}
        body.push(1);
        let approved=verify_exact_body(&body,&b).unwrap();
        assert_eq!(approved.worker_pid,333);
        assert_eq!(approved.worker_creation_filetime,444);
        assert_eq!(approved.supervisor_pid,555);
        assert_eq!(approved.supervisor_creation_filetime,666);
        // A valid signature over the wrong turn is still wrong for this call.
        let stale=ServiceCallBinding::checked(b.call_id(),b.fingerprint_sha256(),
            b.module_id(),b.session_ref(),b.core_epoch_ref(),
            b.conversation_ref(),"other-turn",b.task_ref(),b.workspace_ref()).unwrap();
        assert!(verify_exact_body(&body,&stale).is_err());
        let mut broken=body.clone();
        broken[DOMAIN.len()+1]=1;
        assert!(verify_exact_body(&broken,&b).is_err());
        broken=body.clone();
        *broken.last_mut().unwrap()=2;
        assert!(verify_exact_body(&broken,&b).is_err());
        broken=body.clone();
        broken.push(0);
        assert!(verify_exact_body(&broken,&b).is_err());
    }
    #[test]
    fn unapproved_pins_never_verify(){
        assert!(!valid_public_blob(&[0;PUBLIC_BLOB_BYTES]));
        assert!(verify(b"test",&[0;SIGNATURE_BYTES],&[0;PUBLIC_BLOB_BYTES]).is_err());
        if require_system_service_context().is_err(){
            assert!(sign(b"not privileged").is_err());
        }
    }
}
