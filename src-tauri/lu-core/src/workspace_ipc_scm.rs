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
    /// Independently installed public verification pin, not supplied by
    /// SCM, a DLC, receipt bytes or the model.
    pub receipt_p256_public_blob:[u8;72],
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
    receipt_public_blob:[u8;72],
}
impl VerifiedScmSupervisor{
    pub fn observe(pins:&ApprovedScmPins)->Result<Self,ScmSupervisorError>{
        if !Path::new(&pins.absolute_image_path).is_absolute()
            ||!nonzero(&pins.image_sha256)
            ||!nonzero(&pins.signer_der_sha256)
            ||u32::from_le_bytes(pins.receipt_p256_public_blob[0..4]
                .try_into().unwrap())!=0x31534345
            ||u32::from_le_bytes(pins.receipt_p256_public_blob[4..8]
                .try_into().unwrap())!=32{

            return Err(ScmSupervisorError::UnapprovedPins);
        }
        let native=lu_win_worker_job::ipc_scm_supervisor::
            RunningScmSupervisor::observe(&pins.absolute_image_path)
            .map_err(|_|ScmSupervisorError::ServiceUnavailable)?;
        verify_pins(pins,native.image_file(),native.signer_certificate_der())?;
        native.check_alive().map_err(|_|ScmSupervisorError::ServiceUnavailable)?;
        Ok(Self{native,receipt_public_blob:pins.receipt_p256_public_blob})
    }
    pub fn require_live(&self)->Result<(),ScmSupervisorError>{
        self.native.check_alive().map_err(|_|ScmSupervisorError::ServiceUnavailable)
    }
    /// Authenticate a real installed Core-originated read-only private
    /// SCM health round trip against the SAME pinned OS service generation.
    /// This is an actual local service channel, not a DLC capability grant.
    pub async fn private_scm_health(&self)->Result<(),ScmSupervisorError>{
        use lu_win_worker_job::ipc_scm_control_wire::{ScmControl,ScmCommand};
        self.require_live()?;
        let witness=self.native.original_process_witness();
        let query=ScmControl{
            command:ScmCommand::ReadStatus,
            service_pid:witness.pid,
            service_birth:witness.creation_filetime,
            original:None,
        };
        lu_win_worker_job::ipc_scm_control::issue_core_command(&query,&witness).await
            .map_err(|_|ScmSupervisorError::ServiceUnavailable)?;
        self.require_live()
    }
    /// SCM-only non-secret readiness preflight: confirms an independently
    /// installer-provisioned NVP GPU stop-issuer public key is present before
    /// authorizing any GGUF unload for diffusion. A key's existence is NOT
    /// evidence that the LTX Job has stopped; the original signed Job-empty
    /// receipt is verified separately during the release request.
    pub async fn private_nvp_stop_issuer_ready(
        &self,
        original:&lu_win_worker_job::ipc_scm_control_wire::OriginalCall,
    )->Result<(),ScmSupervisorError>{
        use lu_win_worker_job::ipc_scm_control_wire::{ScmControl,ScmCommand};
        if original.module_id!="nvp.film"{
            return Err(ScmSupervisorError::UnapprovedPins)
        }
        self.require_live()?;
        let witness=self.native.original_process_witness();
        let command=ScmControl{
            command:ScmCommand::VerifyNvpStopIssuerPinned,
            service_pid:witness.pid,
            service_birth:witness.creation_filetime,
            original:Some(original.clone()),
        };
        lu_win_worker_job::ipc_scm_control::issue_core_command(
            &command,&witness
        ).await.map_err(|_|ScmSupervisorError::ReceiptUnavailable)?;
        self.require_live()
    }

    /// Exact-command scope is always supplied from a host-owned, durable
    /// original-turn grant, never from a DLC Frame or model-origin text.
    pub(crate) async fn private_original_command(
        &self,
        original:&lu_win_worker_job::ipc_scm_owned_jobs::ServiceCallBinding,
        action:lu_win_worker_job::ipc_scm_control_wire::ScmCommand,
    )->Result<lu_win_worker_job::ipc_scm_control::ScmControlResponse,ScmSupervisorError>{
        use lu_win_worker_job::ipc_scm_control_wire::{ScmControl,OriginalCall};
        self.require_live()?;
        let witness=self.native.original_process_witness();
        let call=OriginalCall{
            call_id:original.call_id().into(),
            fingerprint_sha256:original.fingerprint_sha256().into(),
            module_id:original.module_id().into(),
            session_ref:original.session_ref().into(),
            core_epoch_ref:original.core_epoch_ref().into(),
            conversation_ref:original.conversation_ref().into(),
            user_turn_ref:original.user_turn_ref().into(),
            task_ref:original.task_ref().into(),
            workspace_ref:original.workspace_ref().into(),
        };
        let query=ScmControl{
            command:action,service_pid:witness.pid,
            service_birth:witness.creation_filetime,original:Some(call),
        };
        let result=lu_win_worker_job::ipc_scm_control::
            issue_core_command(&query,&witness).await
            .map_err(|_|ScmSupervisorError::ServiceUnavailable)?;
        self.require_live()?;
        Ok(result)
    }
    /// Fetch signed SCM-native Job-empty material over the independently
    /// authenticated private channel, then verify against *Core's installed
    /// public key pin*. No SYSTEM-only file reads by the desktop account.
    /// A valid stop NEVER converts UNCERTAIN to APPLIED or grants replay.
    pub(crate) async fn verified_original_executor_stop(
        &self,
        original:&lu_win_worker_job::ipc_scm_owned_jobs::ServiceCallBinding,
    )->Result<lu_win_worker_job::ipc_scm_receipts::VerifiedExecutorStopReceipt,ScmSupervisorError>{
        use lu_win_worker_job::ipc_scm_control_wire::ScmCommand;
        let response=self.private_original_command(
            original,ScmCommand::InspectStoppedExecutor).await?;
        let material=response.signed_receipt
            .ok_or(ScmSupervisorError::ReceiptUnavailable)?;
        lu_win_worker_job::ipc_scm_receipts::verify_executor_stop_bytes(
            original,&self.receipt_public_blob,&material)
            .map_err(|_|ScmSupervisorError::ReceiptUnavailable)
    }
    /// Recovery path: only the original exclusive Core SQLite owner may
    /// reconstruct immutable call/turn/SCM generation for inspection.
    /// An independently verified signer receipt quarantines old execution;
    /// it NEVER marks the effect APPLIED or releases automatic replay.
    pub(crate) async fn reconcile_original_executor_stop(
        &self,journal:&crate::journal::ExclusiveIpcEffectJournal,
        call_id:&str,
    )->Result<(),ScmSupervisorError>{
        let original=journal.original_scm_grant_for_call(call_id)
            .map_err(|_|ScmSupervisorError::ReceiptUnavailable)?;
        let proof=self.verified_original_executor_stop(&original.binding).await?;
        journal.record_verified_scm_executor_stop(&original,&proof)
            .map_err(|_|ScmSupervisorError::ReceiptUnavailable)
    }
    /// Retained native SCM generation from the independently pinned signer
    /// and actual service process; not reconstructible from a PID supplied
    /// in an NVP frame or from a JSON/installer claim.
    pub(crate) fn original_process_witness(&self)
        ->Result<lu_win_worker_job::ipc_pipe_peer::WindowsPipePeer,ScmSupervisorError>{
        self.require_live()?;
        Ok(self.native.original_process_witness())
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
            image_sha256:[1;32],signer_der_sha256:[1;32],
            receipt_p256_public_blob:[0;72]};
        assert!(matches!(VerifiedScmSupervisor::observe(&zero),
            Err(ScmSupervisorError::UnapprovedPins)));
        let mut public=[0u8;72];
        public[0..4].copy_from_slice(&0x31534345u32.to_le_bytes());
        public[4..8].copy_from_slice(&32u32.to_le_bytes());
        public[8]=1;
        let missing=ApprovedScmPins{absolute_image_path:
            r"C:\Windows\System32\not-the-lu-scm-supervisor.exe".into(),
            image_sha256:[0x45;32],signer_der_sha256:[0x12;32],
            receipt_p256_public_blob:public};
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
