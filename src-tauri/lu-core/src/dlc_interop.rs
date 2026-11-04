//! DLC-IPC-01: portable, fail-closed *contract* for Lu-owned Workspace IPC.
//!
//! This module creates NO listener, authenticates NO OS peer, and does NOT
//! grant model/tool authority. It is deliberately separate from the internal
//! AF_UNIX ToolCall carrier and from llama.cpp's private HTTP endpoint.
//! A future trusted Windows named-pipe / Linux AF_UNIX adapter must verify
//! peer identity and obtain a host-approved session BEFORE using this gate.
//! Never serialize a DlcAdmission as if it were a wire bearer credential.

use std::collections::BTreeSet;

use prost::Message;

use crate::inference_wire::{validate_frame as validate_inference_frame, InferenceWireError};
use crate::wire::proto;

pub const DLC_PROTOCOL_VERSION: u32 = 1;
pub const MAX_DLC_FRAME_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_DLC_REQUEST_IDS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DlcPermission {
    ObserveStatus,
    GenerateText,
    GenerateImages,
    CancelOwnGeneration,
}

impl DlcPermission {
    pub fn wire_value(self) -> i32 {
        match self {
            Self::ObserveStatus => 1,
            Self::GenerateText => 2,
            Self::GenerateImages => 3,
            Self::CancelOwnGeneration => 4,
        }
    }

    fn from_wire(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::ObserveStatus),
            2 => Some(Self::GenerateText),
            3 => Some(Self::GenerateImages),
            4 => Some(Self::CancelOwnGeneration),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DlcInteropError {
    EmptyOrOversized,
    UnsupportedVersion(u32),
    InvalidField(&'static str),
    Noncanonical,
    Decode(String),
    Inference(InferenceWireError),
    WrongDirection,
    NotNegotiated,
    AlreadyNegotiated,
    NotHostApproved,
    SessionMismatch,
    RequestReplay,
    GenerationReplay,
    GenerationNotOwned,
    GenerationBusy,
    SessionExhausted,
    Revoked,
}

fn valid_ref(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= 128
        && reference.as_bytes()[0].is_ascii_alphanumeric()
        && reference.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
        })
}

/// Decode is strict: prost ordinarily discards unknown fields and duplicate
/// singular fields, so canonical re-encoding is required at this boundary.
pub fn decode_dlc_frame(data: &[u8]) -> Result<proto::DlcFrame, DlcInteropError> {
    if data.is_empty() || data.len() > MAX_DLC_FRAME_BYTES {
        return Err(DlcInteropError::EmptyOrOversized);
    }
    let frame = proto::DlcFrame::decode(data)
        .map_err(|e| DlcInteropError::Decode(e.to_string()))?;
    validate_dlc_frame(&frame)?;
    if frame.encode_to_vec() != data {
        return Err(DlcInteropError::Noncanonical);
    }
    Ok(frame)
}

pub fn encode_dlc_frame(frame: &proto::DlcFrame) -> Result<Vec<u8>, DlcInteropError> {
    validate_dlc_frame(frame)?;
    Ok(frame.encode_to_vec())
}

pub fn validate_dlc_frame(frame: &proto::DlcFrame) -> Result<(), DlcInteropError> {
    use proto::dlc_frame::Body;

    if frame.protocol_version != DLC_PROTOCOL_VERSION {
        return Err(DlcInteropError::UnsupportedVersion(frame.protocol_version));
    }
    if !valid_ref(&frame.request_id) {
        return Err(DlcInteropError::InvalidField("request_id"));
    }
    let body = frame.body.as_ref().ok_or(DlcInteropError::InvalidField("body"))?;
    match body {
        Body::Hello(hello) => {
            if !valid_ref(&hello.module_id) || !valid_ref(&hello.instance_id) {
                return Err(DlcInteropError::InvalidField("module_or_instance_id"));
            }
            if hello.requested_capabilities.is_empty()
                || hello.requested_capabilities.len() > 4
            {
                return Err(DlcInteropError::InvalidField("capability_count"));
            }
            let mut seen = BTreeSet::new();
            for &operation in &hello.requested_capabilities {
                let permission = DlcPermission::from_wire(operation)
                    .ok_or(DlcInteropError::InvalidField("capability"))?;
                if !seen.insert(permission) {
                    return Err(DlcInteropError::InvalidField("duplicate_capability"));
                }
            }
        }
        Body::Status(status) => validate_inference_frame(&proto::InferenceFrame {
            protocol_version: crate::inference_wire::INFERENCE_PROTOCOL_VERSION,
            request_id: frame.request_id.clone(),
            body: Some(proto::inference_frame::Body::Status(status.clone())),
        })
        .map_err(DlcInteropError::Inference)?,
        Body::Generate(generate) => validate_inference_frame(&proto::InferenceFrame {
            protocol_version: crate::inference_wire::INFERENCE_PROTOCOL_VERSION,
            request_id: frame.request_id.clone(),
            body: Some(proto::inference_frame::Body::Generate(generate.clone())),
        })
        .map_err(DlcInteropError::Inference)?,
        Body::Cancel(cancel) => validate_inference_frame(&proto::InferenceFrame {
            protocol_version: crate::inference_wire::INFERENCE_PROTOCOL_VERSION,
            request_id: frame.request_id.clone(),
            body: Some(proto::inference_frame::Body::Cancel(cancel.clone())),
        })
        .map_err(DlcInteropError::Inference)?,
        Body::Event(event) => validate_inference_frame(&proto::InferenceFrame {
            protocol_version: crate::inference_wire::INFERENCE_PROTOCOL_VERSION,
            request_id: frame.request_id.clone(),
            body: Some(proto::inference_frame::Body::Event(event.clone())),
        })
        .map_err(DlcInteropError::Inference)?,
    }
    if frame.encoded_len() > MAX_DLC_FRAME_BYTES {
        return Err(DlcInteropError::EmptyOrOversized);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DlcAdmittedRequest {
    ObserveStatus,
    Generate { generation_id: String },
    CancelOwnGeneration { generation_id: String },
}

/// A host-held, non-serializable approval fence. Only a trusted Lu host may
/// construct it after OS peer attestation, WorkspaceHost instance ownership,
/// user/project scope and inference resource admission have been verified.
///
/// NOTE: This method alone does not perform those checks; those integration
/// points are deliberately unimplemented until the native carrier exists.
/// Never bind this constructor directly to a WebView, IPC request or model.
pub struct DlcAdmission {
    module_id: String,
    instance_id: String,
    session_id: String,
    host_approved: BTreeSet<DlcPermission>,
    negotiated: Option<BTreeSet<DlcPermission>>,
    seen_request_ids: BTreeSet<String>,
    seen_generations: BTreeSet<String>,
    active_generation: Option<String>,
    revoked: bool,
}

impl DlcAdmission {
    pub fn host_bind_trusted(
        module_id: &str,
        instance_id: &str,
        session_id: &str,
        approved: &[DlcPermission],
    ) -> Result<Self, DlcInteropError> {
        if !valid_ref(module_id) || !valid_ref(instance_id) || !valid_ref(session_id) {
            return Err(DlcInteropError::InvalidField("host_binding"));
        }
        let approved: BTreeSet<_> = approved.iter().copied().collect();
        if approved.is_empty() {
            return Err(DlcInteropError::NotHostApproved);
        }
        Ok(Self {
            module_id: module_id.to_owned(),
            instance_id: instance_id.to_owned(),
            session_id: session_id.to_owned(),
            host_approved: approved,
            negotiated: None,
            seen_request_ids: BTreeSet::new(),
            seen_generations: BTreeSet::new(),
            active_generation: None,
            revoked: false,
        })
    }

    /// Hello declares *requested* capabilities. A successful exchange is
    /// NOT a model/tool grant: it only intersects with an existing host-held
    /// approval, and never permits an unapproved optional operation.
    pub fn negotiate(&mut self, frame: &proto::DlcFrame) -> Result<(), DlcInteropError> {
        use proto::dlc_frame::Body;
        self.require_live()?;
        validate_dlc_frame(frame)?;
        if self.negotiated.is_some() {
            return Err(DlcInteropError::AlreadyNegotiated);
        }
        let Body::Hello(hello) = frame.body.as_ref().ok_or(
            DlcInteropError::InvalidField("body")
        )? else {
            return Err(DlcInteropError::WrongDirection);
        };
        if hello.module_id != self.module_id || hello.instance_id != self.instance_id {
            return Err(DlcInteropError::SessionMismatch);
        }
        let requested = hello.requested_capabilities.iter()
            .filter_map(|&value| DlcPermission::from_wire(value))
            .collect::<BTreeSet<_>>();
        if !requested.is_subset(&self.host_approved) {
            return Err(DlcInteropError::NotHostApproved);
        }
        self.seen_request_ids.insert(frame.request_id.clone());
        self.negotiated = Some(requested);
        Ok(())
    }

    /// Returns an inert decision, NOT an invocation of Lu's inference backend.
    /// The host must still bind the generation to the authenticated user turn,
    /// resident model, scheduler, resource limits, and its durable journal.
    pub fn admit_request(
        &mut self,
        frame: &proto::DlcFrame,
    ) -> Result<DlcAdmittedRequest, DlcInteropError> {
        use proto::dlc_frame::Body;
        self.require_live()?;
        validate_dlc_frame(frame)?;
        let negotiated = self.negotiated.as_ref()
            .ok_or(DlcInteropError::NotNegotiated)?;
        if self.seen_request_ids.contains(&frame.request_id) {
            return Err(DlcInteropError::RequestReplay);
        }
        if self.seen_request_ids.len() >= MAX_DLC_REQUEST_IDS {
            return Err(DlcInteropError::SessionExhausted);
        }
        let action = match frame.body.as_ref() {
            Some(Body::Status(status)) => {
                if !negotiated.contains(&DlcPermission::ObserveStatus) {
                    return Err(DlcInteropError::NotHostApproved);
                }
                self.require_session(&status.session_id)?;
                DlcAdmittedRequest::ObserveStatus
            }
            Some(Body::Generate(generate)) => {
                if !negotiated.contains(&DlcPermission::GenerateText)
                    || (!generate.images.is_empty()
                        && !negotiated.contains(&DlcPermission::GenerateImages))
                {
                    return Err(DlcInteropError::NotHostApproved);
                }
                self.require_session(&generate.session_id)?;
                if self.seen_generations.contains(&generate.generation_id) {
                    return Err(DlcInteropError::GenerationReplay);
                }
                if self.active_generation.is_some() {
                    return Err(DlcInteropError::GenerationBusy);
                }
                let generation_id = generate.generation_id.clone();
                self.seen_generations.insert(generation_id.clone());
                self.active_generation = Some(generation_id.clone());
                DlcAdmittedRequest::Generate { generation_id }
            }
            Some(Body::Cancel(cancel)) => {
                if !negotiated.contains(&DlcPermission::CancelOwnGeneration) {
                    return Err(DlcInteropError::NotHostApproved);
                }
                self.require_session(&cancel.session_id)?;
                if self.active_generation.as_deref() != Some(cancel.generation_id.as_str()) {
                    return Err(DlcInteropError::GenerationNotOwned);
                }
                DlcAdmittedRequest::CancelOwnGeneration {
                    generation_id: cancel.generation_id.clone(),
                }
            }
            _ => return Err(DlcInteropError::WrongDirection),
        };
        self.seen_request_ids.insert(frame.request_id.clone());
        Ok(action)
    }

    /// Only the host's terminal inference event clears an active generation;
    /// the client's cancel request alone never declares completion.
    pub fn host_generation_finished(&mut self, generation_id: &str) -> Result<(), DlcInteropError> {
        if self.active_generation.as_deref() != Some(generation_id) {
            return Err(DlcInteropError::GenerationNotOwned);
        }
        self.active_generation = None;
        Ok(())
    }

    /// Revocation denies all subsequent DLC requests. The host must also
    /// cancel/reap an active worker through its existing supervisor.
    pub fn host_revoke(&mut self) {
        self.revoked = true;
    }

    fn require_live(&self) -> Result<(), DlcInteropError> {
        if self.revoked {
            Err(DlcInteropError::Revoked)
        } else {
            Ok(())
        }
    }

    fn require_session(&self, actual: &str) -> Result<(), DlcInteropError> {
        if actual != self.session_id {
            Err(DlcInteropError::SessionMismatch)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::dlc_frame::Body;

    fn frame(id: &str, body: Body) -> proto::DlcFrame {
        proto::DlcFrame {
            protocol_version: DLC_PROTOCOL_VERSION,
            request_id: id.into(),
            body: Some(body),
        }
    }
    fn hello(perms: &[DlcPermission]) -> proto::DlcFrame {
        frame("req-hello", Body::Hello(proto::DlcHello {
            module_id: "nvp.film".into(),
            instance_id: "nvp-1".into(),
            requested_capabilities: perms.iter().map(|x| x.wire_value()).collect(),
        }))
    }
    fn grant(perms: &[DlcPermission]) -> DlcAdmission {
        DlcAdmission::host_bind_trusted("nvp.film","nvp-1","lu-session-1",perms).unwrap()
    }
    fn status(id: &str, session: &str) -> proto::DlcFrame {
        frame(id, Body::Status(proto::InferenceStatusRequest {
            session_id: session.into(),
        }))
    }
    fn generate(id: &str, generation: &str) -> proto::DlcFrame {
        frame(id, Body::Generate(proto::InferenceGenerateRequest {
            session_id: "lu-session-1".into(),
            generation_id: generation.into(),
            prompt: "Create a storyboard outline".into(),
            max_new_tokens: 64,
            history: Vec::new(),
            images: Vec::new(),
        }))
    }
    fn cancel(id: &str, generation: &str) -> proto::DlcFrame {
        frame(id, Body::Cancel(proto::InferenceCancelRequest {
            session_id: "lu-session-1".into(),
            generation_id: generation.into(),
        }))
    }

    #[test]
    fn canonical_protobuf_envelope_rejects_unknown_duplicate_and_oversized() {
        let hello = hello(&[DlcPermission::ObserveStatus]);
        let encoded = encode_dlc_frame(&hello).unwrap();
        assert_eq!(decode_dlc_frame(&encoded).unwrap(), hello);
        let mut unknown = encoded.clone();
        unknown.extend_from_slice(&[0xf8, 0x07, 0x01]);
        assert_eq!(decode_dlc_frame(&unknown), Err(DlcInteropError::Noncanonical));
        let mut duplicate = encoded;
        duplicate.extend_from_slice(&[0x08, 0x01]);
        assert_eq!(decode_dlc_frame(&duplicate), Err(DlcInteropError::Noncanonical));
        assert_eq!(decode_dlc_frame(&[]), Err(DlcInteropError::EmptyOrOversized));
        assert_eq!(decode_dlc_frame(&vec![0; MAX_DLC_FRAME_BYTES + 1]),
            Err(DlcInteropError::EmptyOrOversized));
    }

    #[test]
    fn incompatible_versions_and_unknown_capabilities_fail_closed() {
        let mut f = hello(&[DlcPermission::ObserveStatus]);
        f.protocol_version = 2;
        assert_eq!(encode_dlc_frame(&f), Err(DlcInteropError::UnsupportedVersion(2)));
        f.protocol_version = DLC_PROTOCOL_VERSION;
        if let Some(Body::Hello(ref mut h)) = f.body {
            h.requested_capabilities.push(99);
        }
        assert_eq!(encode_dlc_frame(&f), Err(DlcInteropError::InvalidField("capability")));
        if let Some(Body::Hello(ref mut h)) = f.body {
            h.requested_capabilities = vec![1,1];
        }
        assert_eq!(encode_dlc_frame(&f), Err(DlcInteropError::InvalidField("duplicate_capability")));
    }

    #[test]
    fn no_operation_is_available_before_host_approved_hello() {
        let mut g = grant(&[DlcPermission::ObserveStatus]);
        assert_eq!(g.admit_request(&status("req-1","lu-session-1")),
            Err(DlcInteropError::NotNegotiated));
        let mut spoof = hello(&[DlcPermission::ObserveStatus]);
        if let Some(Body::Hello(ref mut h)) = spoof.body {
            h.module_id = "other.module".into();
        }
        assert_eq!(g.negotiate(&spoof), Err(DlcInteropError::SessionMismatch));
        g.negotiate(&hello(&[DlcPermission::ObserveStatus])).unwrap();
        assert_eq!(g.admit_request(&status("req-1","lu-session-1")).unwrap(),
            DlcAdmittedRequest::ObserveStatus);
        assert_eq!(g.admit_request(&status("req-1","lu-session-1")),
            Err(DlcInteropError::RequestReplay));
    }

    #[test]
    fn discovery_cannot_escalate_permissions_or_change_sessions() {
        let mut g = grant(&[DlcPermission::ObserveStatus]);
        assert_eq!(g.negotiate(&hello(&[DlcPermission::GenerateText])),
            Err(DlcInteropError::NotHostApproved));
        g.negotiate(&hello(&[DlcPermission::ObserveStatus])).unwrap();
        assert_eq!(g.admit_request(&generate("req-gen","gen-1")),
            Err(DlcInteropError::NotHostApproved));
        assert_eq!(g.admit_request(&status("req-wrong","other-session")),
            Err(DlcInteropError::SessionMismatch));
        assert_eq!(g.negotiate(&hello(&[DlcPermission::ObserveStatus])),
            Err(DlcInteropError::AlreadyNegotiated));
    }

    #[test]
    fn generation_is_single_owned_and_cancel_never_confirms_completion() {
        let ops = [DlcPermission::GenerateText, DlcPermission::CancelOwnGeneration];
        let mut g = grant(&ops);
        g.negotiate(&hello(&ops)).unwrap();
        assert_eq!(g.admit_request(&generate("req-1","gen-1")).unwrap(),
            DlcAdmittedRequest::Generate { generation_id: "gen-1".into() });
        assert_eq!(g.admit_request(&generate("req-2","gen-2")),
            Err(DlcInteropError::GenerationBusy));
        assert_eq!(g.admit_request(&cancel("req-3","gen-OTHER")),
            Err(DlcInteropError::GenerationNotOwned));
        assert_eq!(g.admit_request(&cancel("req-4","gen-1")).unwrap(),
            DlcAdmittedRequest::CancelOwnGeneration { generation_id: "gen-1".into() });
        assert_eq!(g.admit_request(&generate("req-5","gen-2")),
            Err(DlcInteropError::GenerationBusy));
        g.host_generation_finished("gen-1").unwrap();
        assert_eq!(g.admit_request(&generate("req-6","gen-1")),
            Err(DlcInteropError::GenerationReplay));
        assert!(g.admit_request(&generate("req-7","gen-2")).is_ok());
        g.host_revoke();
        assert_eq!(g.admit_request(&cancel("req-8","gen-2")), Err(DlcInteropError::Revoked));
    }

    #[test]
    fn image_requests_need_separate_image_capability_and_pinned_refs() {
        let mut req=generate("req-img","gen-img");
        if let Some(Body::Generate(ref mut generation))=req.body {
            generation.images.push(proto::InferenceImageAttachment {
                image_ref:"shot-1".into(), mime_type:"image/png".into(),
                sha256:vec![1u8;32], size_bytes:1024,
            });
        }
        let mut g=grant(&[DlcPermission::GenerateText]);
        g.negotiate(&hello(&[DlcPermission::GenerateText])).unwrap();
        assert_eq!(g.admit_request(&req), Err(DlcInteropError::NotHostApproved));
        let ops=[DlcPermission::GenerateText,DlcPermission::GenerateImages];
        let mut g=grant(&ops);
        g.negotiate(&hello(&ops)).unwrap();
        assert!(g.admit_request(&req).is_ok());
        if let Some(Body::Generate(ref mut generation))=req.body {
            generation.images[0].image_ref="/tmp/unsafe.png".into();
        }
        assert!(encode_dlc_frame(&req).is_err());
    }

    #[test]
    fn external_client_must_not_send_inference_events_or_load_unload() {
        let mut g=grant(&[DlcPermission::ObserveStatus]);
        g.negotiate(&hello(&[DlcPermission::ObserveStatus])).unwrap();
        let event=frame("req-evt", Body::Event(proto::InferenceEvent {
            session_id:"lu-session-1".into(),
            generation_id:"gen-1".into(),
            sequence:1,
            kind:Some(proto::inference_event::Kind::Finished(proto::InferenceFinished {
                cancelled:false, generated_bytes:5, output_pieces:1,
            })),
        }));
        assert_eq!(g.admit_request(&event), Err(DlcInteropError::WrongDirection));
        // DlcFrame's schema intentionally has NO Load/Unload variants.
    }
}
