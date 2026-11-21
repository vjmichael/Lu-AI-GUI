//! Core-side reviewed installed Windows SCM supervisor identity gate.
//! This proves ONLY that the pinned native SCM service process is alive;
//! no stored file, PID, PATH, or arbitrary SYSTEM process can mint a
//! durable native-executor termination receipt. Actual service-hosted Job
//! custody and SCM crash recovery remain separate installed work.
#![cfg(windows)]

use sha2::{Digest,Sha256};
use std::{fs::File,io::{self,Read,Seek,SeekFrom},path::Path};

const MAX_INSTALLED_SUPERVISOR_IMAGE:u64=128*1024*1024;

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum ScmSupervisorError{
    UnapprovedPins,
    ServiceUnavailable,
    SignedImageMismatch,
    ReceiptUnavailable,
}

pub struct ApprovedScmPins{
    /// Independently installed and protected Lu Core service executable.
    /// Never create these pins from a Hello, DLL, model output or the
    /// executable's own self-asserted metadata.
    pub absolute_image_path:String,
    pub image_sha256:[u8;32],
    pub signer_der_sha256:[u8;32],
}
fn nonzero(value:&[u8;32])->bool{value.iter().any(|b|*b!=0)}
fn exact_digest<R:Read>(mut reader:R,maximum:u64)->io::Result<[u8;32]>{
    let mut hash=Sha256::new();
    let mut amount=0u64;
    let mut buffer=[0u8;64*1024];
    loop{
        let read=reader.read(&mut buffer)?;
        if read==0{break;}
        amount=amount.checked_add(read as u64)
            .ok_or_else(||io::Error::other("supervisor image size overflow"))?;
        if amount>maximum{
            return Err(io::Error::other("supervisor image is oversized"));
        }
        hash.update(&buffer[..read]);
    }
    if amount==0{return Err(io::Error::other("empty supervisor image"));}
    Ok(hash.finalize().into())
}
fn verify_pins(pins:&ApprovedScmPins,image:&File,signer_der:&[u8])
    ->Result<(),ScmSupervisorError>{
    if !Path::new(&pins.absolute_image_path).is_absolute()
        ||!nonzero(&pins.image_sha256)
        ||!nonzero(&pins.signer_der_sha256)
        ||signer_der.is_empty(){
        return Err(ScmSupervisorError::UnapprovedPins);
    }
    let mut retained=image.try_clone()
        .map_err(|_|ScmSupervisorError::SignedImageMismatch)?;
    retained.seek(SeekFrom::Start(0))
        .map_err(|_|ScmSupervisorError::SignedImageMismatch)?;
    let actual=exact_digest(retained,MAX_INSTALLED_SUPERVISOR_IMAGE)
        .map_err(|_|ScmSupervisorError::SignedImageMismatch)?;
    let cert:[u8;32]=Sha256::digest(signer_der).into();
    if actual!=pins.image_sha256 ||cert!=pins.signer_der_sha256{
        return Err(ScmSupervisorError::SignedImageMismatch);
    }
    Ok(())
}

/// Verify an independently signed, write-through SCM executor-stop receipt
/// against an installer-owned public key and exact original call binding.
/// This function never upgrades UNCERTAIN, grants execution, or trusts a
/// caller-supplied receipt filename or public key. A restarted Core must
/// reconstruct original from its own authoritative immutable grant/journal.
pub fn verify_installed_executor_stop(
    original:&lu_win_worker_job::ipc_scm_owned_jobs::ServiceCallBinding,
)->Result<lu_win_worker_job::ipc_scm_receipts::VerifiedExecutorStopReceipt,ScmSupervisorError>{
    let approved=lu_win_worker_job::ipc_installer_enrollment::
        read_installed_scm_receipt_public_key()
        .map_err(|_|ScmSupervisorError::ReceiptUnavailable)?;
    lu_win_worker_job::ipc_scm_receipts::verify_executor_stop(original,&approved)
        .map_err(|_|ScmSupervisorError::ReceiptUnavailable)
}

/// Holds the genuine SCM query handle, original live process generation,
/// locked signed image file, and exact independent publisher/image pins.
/// It never contains worker Jobs, and is NOT executor custody evidence.
pub struct VerifiedScmSupervisor{
    native:lu_win_worker_job::ipc_scm_supervisor::RunningScmSupervisor,
}
impl VerifiedScmSupervisor{
    pub fn observe(pins:&ApprovedScmPins)->Result<Self,ScmSupervisorError>{
        if !Path::new(&pins.absolute_image_path).is_absolute()
            ||!nonzero(&pins.image_sha256)
            ||!nonzero(&pins.signer_der_sha256){
            return Err(ScmSupervisorError::UnapprovedPins);
        }
        let native=lu_win_worker_job::ipc_scm_supervisor::
            RunningScmSupervisor::observe(&pins.absolute_image_path)
            .map_err(|_|ScmSupervisorError::ServiceUnavailable)?;
        verify_pins(pins,native.image_file(),native.signer_certificate_der())?;
        native.check_alive().map_err(|_|ScmSupervisorError::ServiceUnavailable)?;
        Ok(Self{native})
    }
    pub fn require_live(&self)->Result<(),ScmSupervisorError>{
        self.native.check_alive().map_err(|_|ScmSupervisorError::ServiceUnavailable)
    }
    /// Read-only SCM identity: not a native tool executor or kernel Job receipt.
    pub fn scm_service_pid(&self)->u32{self.native.process_id()}
    pub fn scm_service_birth(&self)->u64{self.native.process_creation_filetime()}
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn source_or_model_cannot_mint_arbitrary_scm_identity(){
        let zero=ApprovedScmPins{absolute_image_path:"not-an-install.exe".into(),
            image_sha256:[1;32],signer_der_sha256:[1;32]};
        assert!(matches!(VerifiedScmSupervisor::observe(&zero),
            Err(ScmSupervisorError::UnapprovedPins)));
        let missing=ApprovedScmPins{absolute_image_path:
            r"C:\Windows\System32\not-the-lu-scm-supervisor.exe".into(),
            image_sha256:[0x45;32],signer_der_sha256:[0x12;32]};
        assert!(matches!(VerifiedScmSupervisor::observe(&missing),
            Err(ScmSupervisorError::ServiceUnavailable)));
    }
    #[test]
    fn native_image_digest_requires_complete_bounded_nonempty_input(){
        let actual=exact_digest(&b"reviewed supervisor image"[..],512).unwrap();
        let expected:[u8;32]=Sha256::digest(b"reviewed supervisor image").into();
        assert_eq!(actual,expected);
        assert!(exact_digest(&b""[..],512).is_err());
        assert!(exact_digest(&b"oversized"[..],3).is_err());
    }
}
