use lu_core::workspace_ipc::{
    decode_frame,encode_frame,proto,WorkspaceWireError,
    MAX_FRAME_BYTES,MAX_TYPED_BYTES,WIRE_MAJOR,WIRE_MINOR,
};
use prost::Message;

fn hello()->proto::WorkspaceFrame{
    proto::WorkspaceFrame{
        protocol_major:WIRE_MAJOR,protocol_minor:WIRE_MINOR,
        request_id:"hello-1".into(),session_ref:String::new(),sequence:0,
        body:Some(proto::workspace_frame::Body::Hello(proto::Hello{
            claimed_module_id:"lu.ide".into(),
            claimed_role:proto::ModuleRole::Workspace as i32,
            claimed_package_sha256:vec![4;32],
            minimum_major:WIRE_MAJOR,maximum_major:WIRE_MAJOR,
            maximum_minor:WIRE_MINOR,
            requested_features:vec!["typed-calls".into()],
            client_nonce:vec![5;32],
        })),
    }
}
fn origin()->proto::OriginRefs{
    proto::OriginRefs{
        workspace_ref:"workspace-a".into(),
        conversation_ref:"chat-a".into(),
        user_turn_ref:"turn-a".into(),
        task_ref:"task-a".into(),
        operation_ref:"op-a".into(),
    }
}
fn frame(body:proto::workspace_frame::Body)->proto::WorkspaceFrame{
    proto::WorkspaceFrame{
        protocol_major:WIRE_MAJOR,protocol_minor:WIRE_MINOR,
        request_id:"req-a".into(),session_ref:"host-epoch-a".into(),
        sequence:0,body:Some(body),
    }
}
fn cap(effects:Vec<i32>)->proto::CapabilityDescriptor{
    proto::CapabilityDescriptor{
        capability_id:"cognitum.math.exact".into(),capability_version:1,
        input_type_url:"type.googleapis.com/cognitum.math.v1.ExactRequest".into(),
        output_type_url:"type.googleapis.com/cognitum.math.v1.ExactResult".into(),
        declared_effects:effects,streams_progress:false,
        display_name:"Exact mathematical operation".into(),
    }
}
#[test]
fn hello_versions_and_claims_roundtrip_without_grant(){
    let valid=hello();
    assert_eq!(decode_frame(&encode_frame(&valid).unwrap()).unwrap(),valid);
    let mut incompatible=hello();
    incompatible.protocol_major=2;
    assert_eq!(encode_frame(&incompatible),Err(WorkspaceWireError::UnsupportedVersion));
    let mut unproven=hello();
    if let Some(proto::workspace_frame::Body::Hello(v))=&mut unproven.body{
        v.claimed_package_sha256.pop();
    }
    assert!(encode_frame(&unproven).is_err());
    // Even the valid Hello contains *only* untrusted identity claims.
}
#[test]
fn unknown_fields_duplicate_version_and_oversize_fail_closed(){
    let mut original=encode_frame(&hello()).unwrap();
    original.extend_from_slice(&[0x80,0x06,0x01]); // unknown field 96
    assert_eq!(decode_frame(&original),Err(WorkspaceWireError::NonCanonical));
    let mut duplicate=encode_frame(&hello()).unwrap();
    duplicate.extend_from_slice(&[0x08,0x01]); // duplicate major 1
    assert_eq!(decode_frame(&duplicate),Err(WorkspaceWireError::NonCanonical));
    assert_eq!(decode_frame(&vec![0xff;MAX_FRAME_BYTES+1]),
        Err(WorkspaceWireError::Oversized));
}
#[test]
fn model_generation_goes_to_core_without_model_path_or_key(){
    let req=frame(proto::workspace_frame::Body::Inference(
        proto::CoreInferenceRequest{
            origin:Some(origin()),user_intent_text:"Explain this code".into(),
            context_refs:vec![],max_new_tokens:256,
        }));
    assert_eq!(decode_frame(&encode_frame(&req).unwrap()).unwrap(),req);
    let mut invalid=req.clone();
    if let Some(proto::workspace_frame::Body::Inference(v))=&mut invalid.body{
        v.max_new_tokens=0;
    }
    assert!(encode_frame(&invalid).is_err());
}
#[test]
fn typed_capability_metadata_not_execution_authority(){
    let reg=frame(proto::workspace_frame::Body::Register(proto::RegisterWorkspace{
        module_id:"cognitum".into(),
        kind:proto::ModuleRole::CoreExtension as i32,
        manifest_sha256:vec![1;32],
        capabilities:vec![cap(vec![proto::EffectClass::PureCompute as i32])],
        manifest_revision:1,
    }));
    assert_eq!(decode_frame(&encode_frame(&reg).unwrap()).unwrap(),reg);
    let mut mixed=reg.clone();
    if let Some(proto::workspace_frame::Body::Register(v))=&mut mixed.body{
        v.capabilities[0].declared_effects.push(
            proto::EffectClass::ReadWorkspace as i32);
    }
    assert!(encode_frame(&mixed).is_err()); // No fake READ for pure mathematics.
}
#[test]
fn exact_scoped_invoke_and_oversized_input_refused(){
    let input=proto::TypedValue{
        type_url:"type.googleapis.com/lu.ide.v1.ReadRequest".into(),
        encoded_message:vec![1;8],
    };
    let invoke=frame(proto::workspace_frame::Body::Invoke(
        proto::CapabilityInvoke{
            origin:Some(origin()),capability_id:"lu.ide.read".into(),
            capability_version:1,input:Some(input),
            input_refs:vec![],deadline_unix_ms:1_900_000_000_000,
        }));
    assert_eq!(decode_frame(&encode_frame(&invoke).unwrap()).unwrap(),invoke);
    let mut oversized=invoke.clone();
    if let Some(proto::workspace_frame::Body::Invoke(v))=&mut oversized.body{
        v.input.as_mut().unwrap().encoded_message=vec![1;MAX_TYPED_BYTES+1];
    }
    assert!(encode_frame(&oversized).is_err());
}
#[test]
fn model_lease_requests_are_core_owned_not_load_commands(){
    let req=frame(proto::workspace_frame::Body::AcquireModelLease(
        proto::ModelLeaseRequest{
            origin:Some(origin()),requested_context_tokens:8192,
            requested_duration_ms:60000,
        }));
    assert!(decode_frame(&encode_frame(&req).unwrap()).is_ok());
    let denied=frame(proto::workspace_frame::Body::ModelLeaseOutcome(
        proto::ModelLeaseOutcome{
            operation_ref:"op-a".into(),
            decision:proto::AdmissionDecision::Denied as i32,
            lease_ref:String::new(),reason_code:"MODEL_BUSY".into(),
            expires_at_unix_ms:0,
        }));
    assert!(encode_frame(&denied).is_ok());
    let mut forged=denied.clone();
    if let Some(proto::workspace_frame::Body::ModelLeaseOutcome(v))=&mut forged.body{
        v.lease_ref="borrowed-other-workspace".into();
    }
    assert!(encode_frame(&forged).is_err());
    let release=frame(proto::workspace_frame::Body::ReleaseModelLease(
        proto::ModelLeaseRelease{
            origin:Some(origin()),lease_ref:"host-lease-a".into(),
        }));
    assert!(encode_frame(&release).is_ok()); // Host must recheck owner.
}
#[test]
fn cancellation_signaled_is_not_terminal_rollback(){
    let cancel=frame(proto::workspace_frame::Body::Cancel(proto::CancelOperation{
        operation_ref:"op-a".into(),reason_code:"USER_STOP".into(),
    }));
    assert_eq!(decode_frame(&encode_frame(&cancel).unwrap()).unwrap(),cancel);
    let ack=frame(proto::workspace_frame::Body::CancelAck(
        proto::CancelAcknowledged{operation_ref:"op-a".into(),signal_recorded:true}));
    assert!(encode_frame(&ack).is_ok());
    let receipt=proto::OperationReceipt{
        operation_ref:"op-a".into(),
        state:proto::TerminalState::Uncertain as i32,
        reason_code:"AWAITING_INDEPENDENT_RECONCILIATION".into(),
        output:None,durable_receipt_sha256:vec![],output_refs:vec![],
    };
    let event=proto::WorkspaceFrame{
        sequence:1,body:Some(proto::workspace_frame::Body::OperationEvent(
            proto::OperationEvent{
                operation_ref:"op-a".into(),
                data:Some(proto::operation_event::Data::Terminal(receipt)),
            })),..frame(proto::workspace_frame::Body::Health(proto::HealthRequest{}))
    };
    assert!(encode_frame(&event).is_ok());
    let mut bad=event.clone();bad.sequence=0;
    assert!(encode_frame(&bad).is_err());
}
#[test]
fn credit_and_reconciliation_need_scoped_bounds(){
    let credit=frame(proto::workspace_frame::Body::StreamCredit(
        proto::StreamCredit{operation_ref:"op-a".into(),
            acknowledged_sequence:42,additional_events:64,
        }));
    assert!(encode_frame(&credit).is_ok());
    let mut bad=credit.clone();
    if let Some(proto::workspace_frame::Body::StreamCredit(c))=&mut bad.body{
        c.additional_events=65;
    }
    assert!(encode_frame(&bad).is_err());
    let reconcile=frame(proto::workspace_frame::Body::Reconcile(
        proto::ReconcileOperation{
            operation_ref:"op-a".into(),expected_origin:Some(origin()),
        }));
    assert!(encode_frame(&reconcile).is_ok());
    let unresolved=frame(proto::workspace_frame::Body::Reconciled(
        proto::ReconciledOperation{observed:None,evidence_available:false}));
    assert!(encode_frame(&unresolved).is_ok());
}
#[test]
fn frame_parser_is_not_a_peer_authenticator(){
    // A forged but syntactically complete claim can pass a schema parser.
    // The real transport host MUST bind a trusted OS peer/session and package.
    let forged=hello();
    assert!(encode_frame(&forged).is_ok());
    let _=proto::ModuleRole::ThirdParty as i32;
    let _=prost::Message::encoded_len(&forged);
}
