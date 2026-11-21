//! LU-IPC-01B: transport-agnostic Lu workspace Protobuf v0 draft.
//! The parser is deliberately structural. Its success is NEVER peer identity,
//! user consent, model admission, tool authority, OS custody or effect success.
//! The future native host must verify all of those *outside* this parser.
use prost::Message;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/lu.workspace.v0.rs"));
}

pub const WIRE_MAJOR:u32=1;
pub const WIRE_MINOR:u32=0;
pub const MAX_FRAME_BYTES:usize=256*1024;
pub const MAX_TYPED_BYTES:usize=64*1024;
pub const MAX_CAPABILITIES:usize=64;
pub const MAX_OBJECT_REFS:usize=16;
pub const MAX_STREAM_CREDIT:u32=64;

#[derive(Debug,Clone,PartialEq,Eq)]
pub enum WorkspaceWireError {
    Empty,
    Oversized,
    Decode,
    NonCanonical,
    UnsupportedVersion,
    Invalid(&'static str),
}

fn name(value:&str,max:usize)->Result<(),WorkspaceWireError>{
    if value.is_empty()||value.len()>max||
       !value.bytes().all(|b|b.is_ascii_alphanumeric()||
           matches!(b,b'-'|b'_'|b'.'|b':'|b'/')) {
        return Err(WorkspaceWireError::Invalid("reference"));
    }
    Ok(())
}
fn optional_name(value:&str,max:usize)->Result<(),WorkspaceWireError>{
    if value.is_empty(){Ok(())}else{name(value,max)}
}
fn code(value:&str)->Result<(),WorkspaceWireError>{
    name(value,96)
}
fn sha(value:&[u8])->Result<(),WorkspaceWireError>{
    if value.len()!=32{return Err(WorkspaceWireError::Invalid("sha256"))}
    Ok(())
}
fn type_url(value:&str)->Result<(),WorkspaceWireError>{
    let Some(qualified)=value.strip_prefix("type.googleapis.com/")else{
        return Err(WorkspaceWireError::Invalid("type_url"));
    };
    if qualified.len()>160||!qualified.contains('.') {
        return Err(WorkspaceWireError::Invalid("type_url"));
    }
    name(qualified,160)
}
fn origin(o:&proto::OriginRefs)->Result<(),WorkspaceWireError>{
    name(&o.workspace_ref,128)?;
    name(&o.conversation_ref,128)?;
    name(&o.user_turn_ref,128)?;
    name(&o.task_ref,128)?;
    name(&o.operation_ref,128)
}
fn object(o:&proto::ObjectRef)->Result<(),WorkspaceWireError>{
    name(&o.owner_module_id,128)?;
    name(&o.object_ref,128)?;
    sha(&o.sha256)?;
    if o.size_bytes==0||o.expiry_unix_ms==0 {
        return Err(WorkspaceWireError::Invalid("object_ref_bounds"))
    }
    Ok(())
}
fn typed(t:&proto::TypedValue)->Result<(),WorkspaceWireError>{
    type_url(&t.type_url)?;
    if t.encoded_message.len()>MAX_TYPED_BYTES{
        return Err(WorkspaceWireError::Invalid("typed_payload_bytes"))
    }
    // The receiving domain MUST decode the registered exact type_url and
    // validate its semantics before invoking the original native registry.
    Ok(())
}
fn desc(c:&proto::CapabilityDescriptor)->Result<(),WorkspaceWireError>{
    name(&c.capability_id,128)?;
    if !c.capability_id.contains('.')||c.capability_version==0||
       c.display_name.is_empty()||c.display_name.len()>160 {
        return Err(WorkspaceWireError::Invalid("capability_descriptor"))
    }
    type_url(&c.input_type_url)?;
    type_url(&c.output_type_url)?;
    if c.declared_effects.is_empty()||c.declared_effects.len()>8 {
        return Err(WorkspaceWireError::Invalid("effect_class"))
    }
    let mut pure=false;
    for effect in &c.declared_effects {
        if *effect==0 || proto::EffectClass::try_from(*effect).is_err(){
            return Err(WorkspaceWireError::Invalid("effect_class"));
        }
        pure |= *effect==proto::EffectClass::PureCompute as i32;
    }
    if pure&&c.declared_effects.len()!=1{
        return Err(WorkspaceWireError::Invalid("mixed_pure_effect"));
    }
    // PURE is only a declaration. Existing CORE ToolEffect has no PURE
    // variant; the future adapter MUST REFUSE rather than mislabel a read.
    Ok(())
}
fn receipt(r:&proto::OperationReceipt)->Result<(),WorkspaceWireError>{
    name(&r.operation_ref,128)?;
    if r.state==0||proto::TerminalState::try_from(r.state).is_err(){
        return Err(WorkspaceWireError::Invalid("terminal_state"));
    }
    code(&r.reason_code)?;
    if !r.durable_receipt_sha256.is_empty(){sha(&r.durable_receipt_sha256)?}
    if let Some(output)=&r.output{typed(output)?}
    if r.output_refs.len()>MAX_OBJECT_REFS{
        return Err(WorkspaceWireError::Invalid("output_refs"));
    }
    for item in &r.output_refs{object(item)?}
    Ok(())
}

/// Decoding checks Protobuf canonical bytes, size, version, type declaration
/// shape and cheap message invariants. It cannot authenticate the sender.
/// Transport/session/permission admission remains a separate Lu Core gate.
pub fn validate(frame:&proto::WorkspaceFrame)->Result<(),WorkspaceWireError>{
    use proto::workspace_frame::Body;
    if frame.protocol_major!=WIRE_MAJOR||frame.protocol_minor!=WIRE_MINOR{
        return Err(WorkspaceWireError::UnsupportedVersion);
    }
    name(&frame.request_id,128)?;
    let body=frame.body.as_ref().ok_or(WorkspaceWireError::Invalid("body"))?;
    let handshake=matches!(body,Body::Hello(_)|Body::Welcome(_));
    if handshake {
        if !frame.session_ref.is_empty() && !matches!(body,Body::Welcome(_)){
            return Err(WorkspaceWireError::Invalid("handshake_session"))
        }
    }else{name(&frame.session_ref,128)?}
    let stream=matches!(body,Body::OperationEvent(_)|Body::InferenceEvent(_));
    if (stream&&frame.sequence==0)||(!stream&&frame.sequence!=0){
        return Err(WorkspaceWireError::Invalid("event_sequence"));
    }
    match body {
        Body::Hello(v)=>{
            name(&v.claimed_module_id,128)?;
            sha(&v.claimed_package_sha256)?;
            if v.claimed_role==0 || proto::ModuleRole::try_from(v.claimed_role).is_err()||
                v.minimum_major>WIRE_MAJOR || v.maximum_major<WIRE_MAJOR ||
                v.maximum_minor<WIRE_MINOR ||v.client_nonce.len()!=32 ||
                v.requested_features.len()>32 {
                return Err(WorkspaceWireError::Invalid("hello"));
            }
            for f in &v.requested_features {code(f)?}
        }
        Body::Welcome(v)=>{
            if v.selected_major!=WIRE_MAJOR||v.selected_minor!=WIRE_MINOR{
                return Err(WorkspaceWireError::UnsupportedVersion);
            }
            name(&frame.session_ref,128)?;
            name(&v.core_epoch_ref,128)?;
            if v.enabled_features.len()>32{
                return Err(WorkspaceWireError::Invalid("features"));
            }
            for f in &v.enabled_features{code(f)?}
        }
        Body::Register(v)=>{
            name(&v.module_id,128)?;
            sha(&v.manifest_sha256)?;
            if v.kind==0||proto::ModuleRole::try_from(v.kind).is_err()||
                v.manifest_revision==0||v.capabilities.len()>MAX_CAPABILITIES {
                return Err(WorkspaceWireError::Invalid("registration"));
            }
            let mut ids=std::collections::BTreeSet::new();
            for c in &v.capabilities {
                desc(c)?;
                if !ids.insert(&c.capability_id){
                    return Err(WorkspaceWireError::Invalid("duplicate_capability"))
                }
            }
        }
        Body::Discover(v)=>{
            name(&v.workspace_ref,128)?;
            optional_name(&v.namespace_prefix,96)?;
        }
        Body::Catalog(v)=>{
            if v.capabilities.len()>MAX_CAPABILITIES{
                return Err(WorkspaceWireError::Invalid("catalog_limit"));
            }
            for c in &v.capabilities{desc(c)?}
        }
        Body::Invoke(v)=>{
            origin(v.origin.as_ref().ok_or(WorkspaceWireError::Invalid("origin"))?)?;
            name(&v.capability_id,128)?;
            if v.capability_version==0||v.deadline_unix_ms==0{
                return Err(WorkspaceWireError::Invalid("invoke_bounds"));
            }
            typed(v.input.as_ref().ok_or(WorkspaceWireError::Invalid("input"))?)?;
            if v.input_refs.len()>MAX_OBJECT_REFS{
                return Err(WorkspaceWireError::Invalid("input_refs"));
            }
            for o in &v.input_refs{object(o)?}
        }
        Body::OperationEvent(v)=>{
            name(&v.operation_ref,128)?;
            let data=v.data.as_ref().ok_or(WorkspaceWireError::Invalid("event_data"))?;
            match data {
                proto::operation_event::Data::Progress(p)=>{
                    code(&p.stage_code)?;
                    if p.percent.is_some_and(|n|n>100){
                        return Err(WorkspaceWireError::Invalid("progress_percent"));
                    }
                }
                proto::operation_event::Data::Terminal(t)=>{
                    receipt(t)?;
                    if t.operation_ref!=v.operation_ref{
                        return Err(WorkspaceWireError::Invalid("terminal_origin"))
                    }
                }
            }
        }
        Body::Cancel(v)=>{name(&v.operation_ref,128)?;code(&v.reason_code)?}
        Body::CancelAck(v)=>name(&v.operation_ref,128)?,
        Body::Reconcile(v)=>{
            name(&v.operation_ref,128)?;
            origin(v.expected_origin.as_ref().ok_or(WorkspaceWireError::Invalid("reconcile_origin"))?)?;
            if v.expected_origin.as_ref().is_some_and(|o|o.operation_ref!=v.operation_ref){
                return Err(WorkspaceWireError::Invalid("reconcile_mismatch"))
            }
        }
        Body::Reconciled(v)=>{
            if v.evidence_available {
                receipt(v.observed.as_ref().ok_or(WorkspaceWireError::Invalid("receipt"))?)?;
            } else if v.observed.is_some(){
                return Err(WorkspaceWireError::Invalid("unverified_receipt"))
            }
        }
        Body::ResourceRequest(v)=>{
            origin(v.origin.as_ref().ok_or(WorkspaceWireError::Invalid("resource_origin"))?)?;
            if v.action==0||proto::ResourceAction::try_from(v.action).is_err(){
                return Err(WorkspaceWireError::Invalid("resource_action"));
            }
        }
        Body::ResourceDecision(v)=>{
            name(&v.operation_ref,128)?;
            if v.decision==0||proto::AdmissionDecision::try_from(v.decision).is_err(){
                return Err(WorkspaceWireError::Invalid("resource_decision"));
            }
            code(&v.reason_code)?;
        }
        Body::Focus(v)=>{name(&v.module_id,128)?;name(&v.workspace_ref,128)?}
        Body::FocusOutcome(v)=>code(&v.status_code)?,
        Body::Health(_)=>{}
        Body::HealthReport(v)=>{
            name(&v.module_id,128)?;
            code(&v.status_code)?;
            if v.epoch==0{return Err(WorkspaceWireError::Invalid("health_epoch"))}
        }
        Body::ModelStatus(_)=>{}
        Body::ModelStatusReport(v)=>{
            if v.phase==0||proto::SharedModelPhase::try_from(v.phase).is_err(){
                return Err(WorkspaceWireError::Invalid("model_phase"));
            }
            optional_name(&v.host_model_ref,128)?;
            code(&v.measurement_code)?;
        }
        Body::AcquireModelLease(v)=>{
            origin(v.origin.as_ref().ok_or(WorkspaceWireError::Invalid("lease_origin"))?)?;
            if !(1..=262144).contains(&v.requested_context_tokens)||
                !(1..=3600000).contains(&v.requested_duration_ms){
                return Err(WorkspaceWireError::Invalid("lease_limits"));
            }
        }
        Body::ModelLeaseOutcome(v)=>{
            name(&v.operation_ref,128)?;
            if v.decision==0||proto::AdmissionDecision::try_from(v.decision).is_err(){
                return Err(WorkspaceWireError::Invalid("lease_decision"));
            }
            optional_name(&v.lease_ref,128)?;
            code(&v.reason_code)?;
            if v.decision!=proto::AdmissionDecision::Allowed as i32
                && !v.lease_ref.is_empty(){
                return Err(WorkspaceWireError::Invalid("denied_lease_ref"));
            }
            if !v.lease_ref.is_empty()&&v.expires_at_unix_ms==0{
                return Err(WorkspaceWireError::Invalid("lease_expiry"));
            }
        }
        Body::ReleaseModelLease(v)=>{
            origin(v.origin.as_ref().ok_or(WorkspaceWireError::Invalid("release_origin"))?)?;
            name(&v.lease_ref,128)?;
        }
        Body::Inference(v)=>{
            origin(v.origin.as_ref().ok_or(WorkspaceWireError::Invalid("inference_origin"))?)?;
            if v.user_intent_text.trim().is_empty()||v.user_intent_text.len()>16*1024||
                !(1..=4096).contains(&v.max_new_tokens)||
                v.context_refs.len()>MAX_OBJECT_REFS {
                return Err(WorkspaceWireError::Invalid("inference_bounds"));
            }
            for o in &v.context_refs{object(o)?}
        }
        Body::InferenceEvent(v)=>{
            name(&v.operation_ref,128)?;
            match v.data.as_ref().ok_or(WorkspaceWireError::Invalid("inference_event"))?{
                proto::core_inference_event::Data::Text(t)=>{
                    if t.text.is_empty()||t.text.len()>MAX_TYPED_BYTES{
                        return Err(WorkspaceWireError::Invalid("text_chunk"));
                    }
                }
                proto::core_inference_event::Data::Terminal(t)=>{
                    receipt(t)?;
                    if t.operation_ref!=v.operation_ref{
                        return Err(WorkspaceWireError::Invalid("inference_terminal_origin"))
                    }
                }
            }
        }
        Body::StreamCredit(v)=>{
            name(&v.operation_ref,128)?;
            if v.additional_events==0||v.additional_events>MAX_STREAM_CREDIT{
                return Err(WorkspaceWireError::Invalid("stream_credit"))
            }
        }
        Body::Failure(v)=>{
            code(&v.code)?;
            if v.explanation.len()>256{
                return Err(WorkspaceWireError::Invalid("error_explanation"));
            }
        }
    }
    if frame.encoded_len()>MAX_FRAME_BYTES{
        return Err(WorkspaceWireError::Oversized)
    }
    Ok(())
}

pub fn encode_frame(frame:&proto::WorkspaceFrame)->Result<Vec<u8>,WorkspaceWireError>{
    validate(frame)?;
    Ok(frame.encode_to_vec())
}
pub fn decode_frame(bytes:&[u8])->Result<proto::WorkspaceFrame,WorkspaceWireError>{
    if bytes.is_empty(){return Err(WorkspaceWireError::Empty)}
    if bytes.len()>MAX_FRAME_BYTES{return Err(WorkspaceWireError::Oversized)}
    let frame=proto::WorkspaceFrame::decode(bytes).map_err(|_|WorkspaceWireError::Decode)?;
    validate(&frame)?;
    // Reject duplicate fields, unrecognized tags and noncanonical wire forms.
    if frame.encode_to_vec()!=bytes{return Err(WorkspaceWireError::NonCanonical)}
    Ok(frame)
}
