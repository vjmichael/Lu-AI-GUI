//! LU-IPC-GPU-01A: additive, schema-locked NVP GPU resource lease wire.
//! Parsing is structural only: an OriginRefs, signer claim, NVP stop locator,
//! or allowed-looking reply has no execution/VRAM authority.
use lu_core::workspace_ipc::{proto,validate,encode_frame,decode_frame,WorkspaceWireError};
fn origin()->proto::OriginRefs {
    proto::OriginRefs{workspace_ref:"film-1".into(),
        conversation_ref:"conversation-1".into(),user_turn_ref:"turn-1".into(),
        task_ref:"task-1".into(),operation_ref:"op-1".into()}
}
fn frame(body:proto::workspace_frame::Body)->proto::WorkspaceFrame{
    proto::WorkspaceFrame{protocol_major:1,protocol_minor:0,
        request_id:"req-1".into(),session_ref:"nvp-session-1".into(),
        sequence:0,body:Some(body)}
}
#[test]
fn gpu_request_is_canonical_bounded_and_not_an_inference_model_lease(){
    let mut request=proto::GpuLeaseRequest{
        origin:Some(origin()),device_ref:"gpu-0".into(),
        required_free_vram_bytes:16*1024*1024*1024,max_duration_ms:600_000,
    };
    let original=frame(proto::workspace_frame::Body::GpuLeaseRequest(request.clone()));
    let bytes=encode_frame(&original).unwrap();
    assert_eq!(decode_frame(&bytes).unwrap(),original);
    request.required_free_vram_bytes=0;
    assert_eq!(validate(&frame(proto::workspace_frame::Body::GpuLeaseRequest(request.clone()))),
        Err(WorkspaceWireError::Invalid("gpu_lease_limits")));
    request.required_free_vram_bytes=16*1024*1024*1024;
    request.max_duration_ms=8*60*60*1000+1;
    assert_eq!(validate(&frame(proto::workspace_frame::Body::GpuLeaseRequest(request))),
        Err(WorkspaceWireError::Invalid("gpu_lease_limits")));
}
#[test]
fn allowed_lease_needs_core_minted_ref_and_actual_granted_phase(){
    let mut answer=proto::GpuLeaseOutcome{
        operation_ref:"op-1".into(),
        decision:proto::AdmissionDecision::Allowed as i32,
        phase:proto::GpuLeasePhase::Granted as i32,
        lease_ref:"core-lease-1".into(),
        measured_available_vram_bytes:Some(20*1024*1024*1024),
        reason_code:"GPU_READY".into()
    };
    assert!(validate(&frame(proto::workspace_frame::Body::GpuLeaseOutcome(answer.clone()))).is_ok());
    answer.lease_ref.clear();
    assert_eq!(validate(&frame(proto::workspace_frame::Body::GpuLeaseOutcome(answer.clone()))),
        Err(WorkspaceWireError::Invalid("gpu_lease_grant")));
    answer.lease_ref="core-lease-1".into();
    answer.phase=proto::GpuLeasePhase::Draining as i32;
    assert_eq!(validate(&frame(proto::workspace_frame::Body::GpuLeaseOutcome(answer.clone()))),
        Err(WorkspaceWireError::Invalid("gpu_lease_grant")));
    answer.phase=proto::GpuLeasePhase::Granted as i32;
    answer.decision=proto::AdmissionDecision::Denied as i32;
    assert_eq!(validate(&frame(proto::workspace_frame::Body::GpuLeaseOutcome(answer))),
        Err(WorkspaceWireError::Invalid("gpu_lease_denied_ref")));
}
#[test]
fn all_clear_is_only_a_scoped_locator_not_proof_of_worker_stop(){
    let mut release=proto::GpuLeaseRelease{
        origin:Some(origin()),lease_ref:"core-lease-1".into(),
        nvp_worker_stop_ref:"nvp-worker-empty-ref".into(),
    };
    let body=frame(proto::workspace_frame::Body::GpuLeaseRelease(release.clone()));
    assert!(validate(&body).is_ok());
    assert_eq!(decode_frame(&encode_frame(&body).unwrap()).unwrap(),body);
    release.origin=None;
    assert_eq!(validate(&frame(proto::workspace_frame::Body::GpuLeaseRelease(release))),
        Err(WorkspaceWireError::Invalid("gpu_release_origin")));
}

#[test]
fn queued_gpu_work_is_deferred_not_a_device_grant_and_requires_original_poll(){
    let queued=proto::GpuLeaseOutcome{
        operation_ref:"op-1".into(),
        decision:proto::AdmissionDecision::Deferred as i32,
        phase:proto::GpuLeasePhase::Draining as i32,
        lease_ref:"gpu-queued-1".into(),
        measured_available_vram_bytes:None,
        reason_code:"GPU_QUEUED".into(),
    };
    let f=frame(proto::workspace_frame::Body::GpuLeaseOutcome(queued.clone()));
    assert_eq!(decode_frame(&encode_frame(&f).unwrap()).unwrap(),f);
    let poll=proto::GpuLeasePoll{
        origin:Some(origin()),lease_ref:queued.lease_ref.clone(),
        transition:proto::GpuLeaseTransitionKind::Acquire as i32,
    };
    assert!(validate(&frame(proto::workspace_frame::Body::GpuLeasePoll(
        poll.clone()))).is_ok());
    let invalid=proto::GpuLeasePoll{transition:0,..poll};
    assert_eq!(validate(&frame(proto::workspace_frame::Body::GpuLeasePoll(invalid))),
        Err(WorkspaceWireError::Invalid("gpu_poll_transition")));
    let invalid=proto::GpuLeaseOutcome{
        phase:proto::GpuLeasePhase::Granted as i32,..queued
    };
    assert_eq!(validate(&frame(proto::workspace_frame::Body::GpuLeaseOutcome(invalid))),
        Err(WorkspaceWireError::Invalid("gpu_lease_pending_ref")));
}
