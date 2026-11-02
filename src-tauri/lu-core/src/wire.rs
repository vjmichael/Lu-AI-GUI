//! Protobuf serialization at Lu-internal boundaries.
//!
//! Typed Protobuf frames are encoded and decoded here. Incoming message type
//! names are checked before tool dispatch; full per-tool payload verification
//! happens in each generated-schema tool adapter. This is deliberately not a
//! converter from arbitrary JSON into arbitrary Protobuf bytes.

use prost::Message;

use crate::contracts::{ProtoPayload, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION};
use crate::registry::{DispatchError, ToolResponse};

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/lu.v1.rs"));
}

pub const MAX_CALL_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_REPLY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    Oversized,
    Decode(String),
    MissingField(&'static str),
    UnsupportedContractVersion(u32),
    InvalidTypeUrl,
}

fn check_type(s: &str) -> Result<(), WireError> {
    if !s.starts_with("type.googleapis.com/") || s.len() > 512 {
        return Err(WireError::InvalidTypeUrl);
    }
    Ok(())
}

pub fn encode_invocation(inv: &ToolInvocation) -> Result<Vec<u8>, WireError> {
    check_type(&inv.input.type_url)?;
    if inv.call_id.trim().is_empty() {
        return Err(WireError::MissingField("call_id"));
    }
    if inv.tool_id.trim().is_empty() {
        return Err(WireError::MissingField("tool_id"));
    }
    let call = proto::ToolCall {
        contract_version: u32::from(CORE_CONTRACT_VERSION),
        call_id: inv.call_id.clone(),
        tool_id: inv.tool_id.clone(),
        workspace_id: inv.workspace_id.clone(),
        input_type_url: inv.input.type_url.clone(),
        input: inv.input.bytes.clone(),
    };
    if call.encoded_len() > MAX_CALL_BYTES {
        return Err(WireError::Oversized);
    }
    Ok(call.encode_to_vec())
}

pub fn decode_invocation(frame: &[u8]) -> Result<ToolInvocation, WireError> {
    if frame.len() > MAX_CALL_BYTES {
        return Err(WireError::Oversized);
    }
    let call = proto::ToolCall::decode(frame).map_err(|e| WireError::Decode(e.to_string()))?;
    if call.contract_version != u32::from(CORE_CONTRACT_VERSION) {
        return Err(WireError::UnsupportedContractVersion(call.contract_version));
    }
    if call.call_id.trim().is_empty() {
        return Err(WireError::MissingField("call_id"));
    }
    if call.tool_id.trim().is_empty() {
        return Err(WireError::MissingField("tool_id"));
    }
    check_type(&call.input_type_url)?;
    Ok(ToolInvocation {
        call_id: call.call_id,
        tool_id: call.tool_id,
        workspace_id: call.workspace_id,
        input: ProtoPayload {
            type_url: call.input_type_url,
            bytes: call.input,
        },
    })
}

fn dispatch_error(error: &DispatchError) -> proto::ToolError {
    let (code, retryable) = match error {
        DispatchError::UnknownTool(_) => ("UNKNOWN_TOOL", false),
        DispatchError::ModuleDraining(_) => ("EXTENSION_DRAINING", true),
        DispatchError::InvalidCallId => ("INVALID_CALL", false),
        DispatchError::WorkspaceRequired(_) => ("WORKSPACE_REQUIRED", false),
        DispatchError::InputTypeMismatch { .. } => ("INPUT_TYPE_MISMATCH", false),
        DispatchError::Denied(_) => ("DENIED", false),
        DispatchError::ProtectedEffect(_) => ("PRIVILEGE_BLOCKED", false),
        DispatchError::Execution(e) => (&*e.code, e.retryable),
        DispatchError::Poisoned => ("REGISTRY_UNAVAILABLE", true),
    };
    proto::ToolError {
        code: code.to_string(),
        message: format!("{error:?}"),
        retryable,
    }
}

pub fn encode_reply(call_id: &str, result: &ToolResponse) -> Result<Vec<u8>, WireError> {
    if call_id.trim().is_empty() {
        return Err(WireError::MissingField("call_id"));
    }
    let (output_type_url, outcome) = match result {
        Ok(output) => {
            check_type(&output.payload.type_url)?;
            (
                output.payload.type_url.clone(),
                Some(proto::tool_reply::Outcome::Output(
                    output.payload.bytes.clone(),
                )),
            )
        }
        Err(error) => (
            String::new(),
            Some(proto::tool_reply::Outcome::Failure(dispatch_error(error))),
        ),
    };
    let reply = proto::ToolReply {
        call_id: call_id.to_string(),
        output_type_url,
        outcome,
    };
    if reply.encoded_len() > MAX_REPLY_BYTES {
        return Err(WireError::Oversized);
    }
    Ok(reply.encode_to_vec())
}

pub fn decode_reply(frame: &[u8]) -> Result<proto::ToolReply, WireError> {
    if frame.len() > MAX_REPLY_BYTES {
        return Err(WireError::Oversized);
    }
    let reply = proto::ToolReply::decode(frame).map_err(|e| WireError::Decode(e.to_string()))?;
    if reply.call_id.trim().is_empty() {
        return Err(WireError::MissingField("call_id"));
    }
    match &reply.outcome {
        Some(proto::tool_reply::Outcome::Output(_)) => check_type(&reply.output_type_url)?,
        Some(proto::tool_reply::Outcome::Failure(err)) if !err.code.is_empty() => {}
        _ => return Err(WireError::MissingField("outcome")),
    }
    Ok(reply)
}

pub fn typed_result<T: Message>(output_type_url: impl Into<String>, value: &T) -> ToolOutput {
    ToolOutput {
        payload: ProtoPayload {
            type_url: output_type_url.into(),
            bytes: value.encode_to_vec(),
        },
    }
}

pub const CE_WORKER_PROTOCOL_VERSION: u32 = 1;
pub const MAX_CE_WORKER_FRAME_BYTES: usize = 128 * 1024;
pub const MAX_CE_WORKER_PAYLOAD_BYTES: usize = 64 * 1024;

/// Strictly validate the untrusted worker's generated Protobuf envelope.
/// Reject duplicate or unknown fields and noncanonical serializations, not
/// merely schema-incompatible values. Host-owned descriptors determine effects.
fn validate_ce_worker_frame(frame: &proto::CeWorkerFrame) -> Result<(), WireError> {
    use proto::ce_worker_frame::Body;
    if frame.protocol_version != CE_WORKER_PROTOCOL_VERSION {
        return Err(WireError::UnsupportedContractVersion(
            frame.protocol_version,
        ));
    }
    match frame.body.as_ref().ok_or(WireError::MissingField("body"))? {
        Body::Invoke(request) => {
            if request.call_id.trim().is_empty() || request.tool_id.trim().is_empty() {
                return Err(WireError::MissingField("invocation identity"));
            }
            check_type(&request.input_type_url)?;
            if request.input.len() > MAX_CE_WORKER_PAYLOAD_BYTES {
                return Err(WireError::Oversized);
            }
        }
        Body::Read(request) => {
            if request.request_id == 0
                || request.tool_id.trim().is_empty()
                || request.relative_path.is_empty()
            {
                return Err(WireError::MissingField("read fields"));
            }
        }
        Body::Write(request) => {
            if request.request_id == 0
                || request.tool_id.trim().is_empty()
                || request.relative_path.is_empty()
            {
                return Err(WireError::MissingField("write fields"));
            }
            if request.data.len() > MAX_CE_WORKER_PAYLOAD_BYTES {
                return Err(WireError::Oversized);
            }
        }
        Body::BrokerResult(reply) => {
            if reply.request_id == 0 {
                return Err(WireError::MissingField("broker request ID"));
            }
            if reply.data.len() > MAX_CE_WORKER_PAYLOAD_BYTES
                || (!reply.ok && !reply.data.is_empty())
            {
                return Err(WireError::Oversized);
            }
        }
        Body::Result(result) => {
            check_type(&result.output_type_url)?;
            if result.output.len() > MAX_CE_WORKER_PAYLOAD_BYTES {
                return Err(WireError::Oversized);
            }
        }
    }
    if frame.encoded_len() > MAX_CE_WORKER_FRAME_BYTES {
        return Err(WireError::Oversized);
    }
    Ok(())
}

pub fn encode_ce_worker_frame(frame: &proto::CeWorkerFrame) -> Result<Vec<u8>, WireError> {
    validate_ce_worker_frame(frame)?;
    Ok(frame.encode_to_vec())
}

pub fn decode_ce_worker_frame(bytes: &[u8]) -> Result<proto::CeWorkerFrame, WireError> {
    if bytes.is_empty() || bytes.len() > MAX_CE_WORKER_FRAME_BYTES {
        return Err(WireError::Oversized);
    }
    let frame = proto::CeWorkerFrame::decode(bytes)
        .map_err(|error| WireError::Decode(error.to_string()))?;
    validate_ce_worker_frame(&frame)?;
    // Prost normally ignores unknown fields. For an untrusted CE boundary,
    // require canonical round-tripping to reject ambiguous envelopes.
    if frame.encode_to_vec() != bytes {
        return Err(WireError::Decode("noncanonical worker frame".into()));
    }
    Ok(frame)
}

#[cfg(test)]
mod ce_worker_tests {
    use super::*;
    use proto::ce_worker_frame::Body;

    #[test]
    fn worker_proto_is_versioned_canonical_and_bounded() {
        let frame = proto::CeWorkerFrame {
            protocol_version: CE_WORKER_PROTOCOL_VERSION,
            body: Some(Body::Read(proto::CeWorkerRead {
                request_id: 7,
                tool_id: "acme.worker.read".into(),
                relative_path: "relative.txt".into(),
            })),
        };
        let encoded = encode_ce_worker_frame(&frame).unwrap();
        assert_eq!(decode_ce_worker_frame(&encoded).unwrap(), frame);
        let mut with_unknown = encoded.clone();
        with_unknown.extend_from_slice(&[0x78, 0x01]);
        assert!(decode_ce_worker_frame(&with_unknown).is_err());
        let mut with_duplicate = encoded.clone();
        with_duplicate.extend_from_slice(&[0x08, 0x01]);
        assert!(decode_ce_worker_frame(&with_duplicate).is_err());
        let mut bad_version = frame.clone();
        bad_version.protocol_version = 99;
        assert!(encode_ce_worker_frame(&bad_version).is_err());
        let mut bad_request = frame.clone();
        bad_request.body = Some(Body::Read(proto::CeWorkerRead {
            request_id: 0,
            tool_id: "acme.worker.read".into(),
            relative_path: "input".into(),
        }));
        assert!(encode_ce_worker_frame(&bad_request).is_err());
        let oversized = proto::CeWorkerFrame {
            protocol_version: CE_WORKER_PROTOCOL_VERSION,
            body: Some(Body::Result(proto::CeWorkerResult {
                output_type_url: "type.googleapis.com/lu.v1.CeOutput".into(),
                output: vec![0; MAX_CE_WORKER_PAYLOAD_BYTES + 1],
            })),
        };
        assert!(encode_ce_worker_frame(&oversized).is_err());
        assert!(decode_ce_worker_frame(&vec![0; MAX_CE_WORKER_FRAME_BYTES + 1]).is_err());
    }
}
