//! LU-IPC-GPU-01B: independent, original-call-bound NVP worker-stop proof.
//!
//! The only positive result is a verified P-256 signature from an
//! installer-pinned NVP SCM signer, over a fixed canonical binary record,
//! and an exact original GPU lease/user turn/worker group. This does NOT
//! accept NVP text "all clear", caller-selected public keys, a PID lookup,
//! Lu IDE/SCM executor-stop receipts or a positive unverified JSON flag.
//! A signed receipt is as strong as NVP's OWN installed SCM issuer: its real
//! original Job-empty kernel witness and key custody must pass physical tests.
#![cfg(windows)]
use std::io;
use crate::{
    ipc_installer_enrollment::{
        read_installed_nvp_gpu_stop_public_key,
        read_installed_nvp_gpu_stop_receipt,
    },
    ipc_scm_receipts::verify_external_p256_signature,
};
const DOMAIN:&[u8]=b"LU_NVP_GPU_JOB_EMPTY_V1\0";
const REQUIRED_METHOD:&str="WINDOWS_JOB_ACTIVE_ZERO_V1";
const MAX_SCOPE:usize=128;
fn denied(reason:&'static str)->io::Error{
    io::Error::new(io::ErrorKind::PermissionDenied,reason)
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct NvpOriginalGpuLease<'a>{
    pub lease_ref:&'a str,
    pub device_ref:&'a str,
    pub module_id:&'a str,
    pub workspace_ref:&'a str,
    pub conversation_ref:&'a str,
    pub user_turn_ref:&'a str,
    pub task_ref:&'a str,
    pub core_epoch_ref:&'a str,
    pub nvp_session_ref:&'a str,
}
fn valid_scope(s:&str)->bool{
    !s.is_empty()&&s.len()<=MAX_SCOPE&&s.bytes().all(|ch|
        ch.is_ascii_alphanumeric()||matches!(ch,b'-'|b'_'|b'.'|b':'|b'/'))
}
fn push(out:&mut Vec<u8>,s:&str)->io::Result<()>{
    if !valid_scope(s){return Err(denied("invalid original GPU scope"))}
    out.push(s.len() as u8);
    out.extend_from_slice(s.as_bytes());
    Ok(())
}
/// Exact signed envelope. A protected installed NVP SCM service must write
/// DOMAIN || length-prefixed original fields || length-prefixed method ||
/// service_pid:u32 LE || service_birth:u64 LE || worker_pid:u32 LE ||
/// worker_birth:u64 LE || P256 ECDSA signature (64 raw bytes).
///
/// The service must have held the SAME native original worker-group Job and
/// confirmed ACTIVE_ZERO before signing; this reader cannot independently
/// recreate a dead process's kernel handle from arbitrary PID evidence.
pub fn canonical_body(
    original:&NvpOriginalGpuLease<'_>,service_pid:u32,service_birth:u64,
    worker_pid:u32,worker_birth:u64,
)->io::Result<Vec<u8>>{
    if original.module_id!="nvp.film" ||
       service_pid==0||service_birth==0||worker_pid==0||worker_birth==0{
        return Err(denied("missing original signed NVP SCM custody"))
    }
    let mut out=DOMAIN.to_vec();
    for value in [
        original.lease_ref,original.device_ref,original.module_id,
        original.workspace_ref,original.conversation_ref,
        original.user_turn_ref,original.task_ref,
        original.core_epoch_ref,original.nvp_session_ref,
        REQUIRED_METHOD,
    ]{push(&mut out,value)?;}
    out.extend_from_slice(&service_pid.to_le_bytes());
    out.extend_from_slice(&service_birth.to_le_bytes());
    out.extend_from_slice(&worker_pid.to_le_bytes());
    out.extend_from_slice(&worker_birth.to_le_bytes());
    Ok(out)
}
fn parse_ids_and_verify(
    original:&NvpOriginalGpuLease<'_>,raw:&[u8],approved:&[u8;72],
)->io::Result<()>{
    if !raw.starts_with(DOMAIN)||raw.len()>4096||
       raw.len()<DOMAIN.len()+10+24+64 {
        return Err(denied("invalid NVP GPU witness envelope"))
    }
    let mut at=DOMAIN.len();
    let read_scope=|raw:&[u8],at:&mut usize|->io::Result<String>{
        let len=usize::from(*raw.get(*at).ok_or_else(||denied("truncated witness"))?);
        *at+=1;
        if len==0||len>MAX_SCOPE||raw.len()<*at+len+24+64 {
            return Err(denied("unbounded witness scope"))
        }
        let text=std::str::from_utf8(&raw[*at..*at+len])
            .map_err(|_|denied("invalid utf8 witness"))?;
        if !valid_scope(text){return Err(denied("invalid witness scope"))}
        *at+=len;
        Ok(text.into())
    };
    let expected=[
        original.lease_ref,original.device_ref,original.module_id,
        original.workspace_ref,original.conversation_ref,
        original.user_turn_ref,original.task_ref,
        original.core_epoch_ref,original.nvp_session_ref,
        REQUIRED_METHOD,
    ];
    for field in expected{
        if read_scope(raw,&mut at)?.as_str()!=field{
            return Err(denied("NVP worker receipt is for another original lease"))
        }
    }
    if raw.len()!=at+4+8+4+8+64{
        return Err(denied("NVP worker receipt has trailing or missing bytes"))
    }
    let spid=u32::from_le_bytes(raw[at..at+4].try_into().unwrap());at+=4;
    let sbirth=u64::from_le_bytes(raw[at..at+8].try_into().unwrap());at+=8;
    let wpid=u32::from_le_bytes(raw[at..at+4].try_into().unwrap());at+=4;
    let wbirth=u64::from_le_bytes(raw[at..at+8].try_into().unwrap());at+=8;
    let canonical=canonical_body(original,spid,sbirth,wpid,wbirth)?;
    if canonical.as_slice()!=&raw[..at]{
        return Err(denied("noncanonical NVP GPU stop witness"))
    }
    verify_external_p256_signature(&raw[..at],&raw[at..],approved)
        .map_err(|_|denied("NVP GPU SCM signature verification failed"))
}
/// The original Core worker-stop observer, not a Protobuf or NVP client
/// callback, reads an exact lease-derived path and a distinct protected
/// installer-owned NVP public key. Any missing file/ACL/key/signature fails.
pub fn independently_verified_nvp_worker_empty(
    original:&NvpOriginalGpuLease<'_>,
)->io::Result<()>{
    if original.module_id!="nvp.film"||!valid_scope(original.lease_ref) {
        return Err(denied("not a protected original NVP film GPU lease"))
    }
    let pin=read_installed_nvp_gpu_stop_public_key()?;
    let raw=read_installed_nvp_gpu_stop_receipt(original.lease_ref)?;
    parse_ids_and_verify(original,&raw,&pin)
}
#[cfg(test)]
mod tests{
    use super::*;
    fn original<'a>()->NvpOriginalGpuLease<'a>{
        NvpOriginalGpuLease{
            lease_ref:"lease-1",device_ref:"dxgi:luid_0x12345678_0xabcdef12_phys_0",
            module_id:"nvp.film",workspace_ref:"film-1",
            conversation_ref:"conv-1",user_turn_ref:"turn-1",
            task_ref:"task-1",core_epoch_ref:"epoch-1",nvp_session_ref:"session-1",
        }
    }
    #[test]
    fn unsigned_or_foreign_witness_cannot_be_treated_as_release(){
        let o=original();
        let mut raw=canonical_body(&o,1,2,3,4).unwrap();
        let mut pin=[0u8;72];pin[0..4].copy_from_slice(&0x31534345u32.to_le_bytes());
        pin[4..8].copy_from_slice(&32u32.to_le_bytes());pin[8]=1;
        raw.extend_from_slice(&[0u8;64]);
        assert!(parse_ids_and_verify(&o,&raw,&pin).is_err());
        let altered=NvpOriginalGpuLease{user_turn_ref:"other",..o};
        assert!(parse_ids_and_verify(&altered,&raw,&pin).is_err());
        raw.push(0);
        assert!(parse_ids_and_verify(&original(),&raw,&pin).is_err());
    }
    #[test]
    fn missing_original_job_witness_is_always_refused(){
        let o=original();
        assert!(canonical_body(&o,0,1,2,3).is_err());
        assert!(canonical_body(&o,1,0,2,3).is_err());
        assert!(canonical_body(&o,1,2,0,3).is_err());
        assert!(canonical_body(&o,1,2,3,0).is_err());
    }
}
