//! WO10-A strict data-only Protobuf codec for a future optional Lu bridge.
//! The codec is not a socket, peer authentication, human grant or permission.
use prost::Message;
use crate::wire::valid_project_id;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/lu.ide.bridge.v1.rs"));
}
pub const BRIDGE_VERSION: u32 = 1;
pub const MAX_BRIDGE_BYTES: usize = 48 * 1024;
const MAX_PROPOSAL_BYTES: usize = 24 * 1024;
const MAX_CONTEXT_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeWireError {
    Oversized,
    Noncanonical,
    Decode,
    Invalid(&'static str),
}

fn id(input: &str) -> bool {
    !input.is_empty()
        && input.len() <= 128
        && input.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn sha256(input: &str) -> bool {
    input.len() == 64
        && input.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_session(input: &str) -> bool {
    input.len() == 32
        && input.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn caps(values: &[i32]) -> bool {
    let mut seen = std::collections::BTreeSet::new();
    values.len() <= 4
        && values.iter().all(|value| (1..=4).contains(value) && seen.insert(*value))
}
fn code(input: &str) -> bool {
    !input.is_empty() && input.len() <= 64
        && input.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

pub fn validate(message: &proto::BridgeEnvelope) -> Result<(), BridgeWireError> {
    use proto::bridge_envelope::Body;
    if message.protocol_version != BRIDGE_VERSION || !id(&message.request_id) {
        return Err(BridgeWireError::Invalid("bridge header"));
    }
    match message.body.as_ref().ok_or(BridgeWireError::Invalid("bridge body"))? {
        Body::Hello(hello) => {
            if !message.session_id.is_empty() || message.sequence != 0
                || !matches!(hello.client_surface.as_str(), "lu.ide" | "lu.core")
                || hello.minimum_version != BRIDGE_VERSION
                || hello.maximum_version != BRIDGE_VERSION
                || hello.challenge_nonce.len() != 32
                || !caps(&hello.advertised_capabilities) {
                return Err(BridgeWireError::Invalid("hello"));
            }
        }
        Body::HelloAck(ack) => {
            if !valid_session(&message.session_id) || message.sequence != 0
                || ack.selected_version != BRIDGE_VERSION
                || ack.echoed_challenge_nonce.len() != 32
                || ack.responder_nonce.len() != 32
                || !caps(&ack.advertised_capabilities) {
                return Err(BridgeWireError::Invalid("hello ack"));
            }
        }
        Body::ContextOffer(offer) => {
            if !valid_session(&message.session_id) || message.sequence == 0
                || !valid_project_id(&offer.project_id)
                || !id(&offer.thread_id)
                || !sha256(&offer.source_revision)
                || offer.summary.len() > MAX_CONTEXT_BYTES
                || offer.summary.contains('\0') {
                return Err(BridgeWireError::Invalid("context offer"));
            }
        }
        Body::ProposalOffer(offer) => {
            if !valid_session(&message.session_id) || message.sequence == 0
                || !valid_project_id(&offer.project_id)
                || !id(&offer.thread_id)
                || !id(&offer.task_id)
                || offer.proposal_json.is_empty()
                || offer.proposal_json.len() > MAX_PROPOSAL_BYTES {
                return Err(BridgeWireError::Invalid("proposal offer"));
            }
        }
        Body::Failure(failure) => {
            if !valid_session(&message.session_id) || message.sequence == 0
                || !code(&failure.code) {
                return Err(BridgeWireError::Invalid("failure"));
            }
        }
        Body::Goodbye(goodbye) => {
            if !valid_session(&message.session_id) || message.sequence == 0
                || !code(&goodbye.reason_code) {
                return Err(BridgeWireError::Invalid("goodbye"));
            }
        }
        Body::ChatRequest(chat) => {
            if !valid_session(&message.session_id) || message.sequence == 0
                || !valid_project_id(&chat.project_id)
                || !id(&chat.thread_id) || !id(&chat.generation_id)
                || chat.prompt.trim().is_empty() || chat.prompt.len() > 8192
                || chat.prompt.as_bytes().contains(&0)
                || chat.max_new_tokens == 0 || chat.max_new_tokens > 2048
                || chat.history.len() > 12
                || chat.history.iter().any(|turn| !matches!(turn.role.as_str(),"user"|"assistant")
                    || turn.text.is_empty() || turn.text.len() > 8192
                    || turn.text.as_bytes().contains(&0))
                || chat.history.iter().map(|t| t.text.len()).sum::<usize>() > 24 * 1024
            { return Err(BridgeWireError::Invalid("coding chat request")); }
        }
        Body::ChatReply(reply) => {
            if !valid_session(&message.session_id) || message.sequence == 0
                || !valid_project_id(&reply.project_id) || !id(&reply.thread_id)
                || !id(&reply.generation_id) || !id(&reply.model_session_id)
                || reply.text.len() > 16384
                || reply.text.as_bytes().contains(&0)
                || !matches!(reply.finish_code.as_str(), "completed"|"cancelled"|"failed")
            { return Err(BridgeWireError::Invalid("coding chat reply")); }
        }
        Body::ChatCancel(cancel) => {
            if !valid_session(&message.session_id) || message.sequence == 0
                || !valid_project_id(&cancel.project_id)
                || !id(&cancel.thread_id) || !id(&cancel.generation_id)
            { return Err(BridgeWireError::Invalid("coding chat cancel")); }
        }
    }
    if message.encoded_len() > MAX_BRIDGE_BYTES {
        return Err(BridgeWireError::Oversized);
    }
    Ok(())
}

pub fn encode(message: &proto::BridgeEnvelope) -> Result<Vec<u8>, BridgeWireError> {
    validate(message)?;
    Ok(message.encode_to_vec())
}

pub fn decode(bytes: &[u8]) -> Result<proto::BridgeEnvelope, BridgeWireError> {
    if bytes.is_empty() || bytes.len() > MAX_BRIDGE_BYTES {
        return Err(BridgeWireError::Oversized);
    }
    let message = proto::BridgeEnvelope::decode(bytes).map_err(|_| BridgeWireError::Decode)?;
    validate(&message)?;
    if message.encode_to_vec() != bytes {
        return Err(BridgeWireError::Noncanonical);
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proto::{bridge_envelope::Body, BridgeHello, BridgeHelloAck, ProjectContextOffer, ProposedChangeOffer};
    fn hello() -> proto::BridgeEnvelope {
        proto::BridgeEnvelope {
            protocol_version: BRIDGE_VERSION, request_id: "id-1".into(),
            session_id: String::new(), sequence: 0,
            body: Some(Body::Hello(BridgeHello {
                client_surface: "lu.core".into(), minimum_version: 1, maximum_version: 1,
                challenge_nonce: vec![23; 32],
                advertised_capabilities: vec![1, 2, 3],
            })),
        }
    }
    #[test]
    fn canonical_roundtrip_without_any_authority() {
        let msg = hello();
        let frame = encode(&msg).unwrap();
        assert_eq!(decode(&frame).unwrap(), msg);
        let mut extra = frame.clone();
        extra.extend_from_slice(&[0x08, 0x01]);
        assert_eq!(decode(&extra), Err(BridgeWireError::Noncanonical));
        assert!(decode(&vec![42; MAX_BRIDGE_BYTES + 1]).is_err());
        assert!(encode(&proto::BridgeEnvelope { protocol_version: 2, ..msg }).is_err());
    }
    #[test]
    fn challenge_caps_and_session_are_strict() {
        let mut msg = hello();
        if let Some(Body::Hello(h)) = &mut msg.body {
            h.advertised_capabilities.push(3);
        }
        assert!(encode(&msg).is_err());
        if let Some(Body::Hello(h)) = &mut msg.body {
            h.advertised_capabilities = vec![99];
        }
        assert!(encode(&msg).is_err());
        let ack = proto::BridgeEnvelope {
            protocol_version: 1, request_id: "id-ack".into(),
            session_id: "a".repeat(32), sequence: 0,
            body: Some(Body::HelloAck(BridgeHelloAck {
                selected_version: 1,
                echoed_challenge_nonce: vec![1; 32],
                responder_nonce: vec![2; 32],
                advertised_capabilities: vec![2, 3],
            })),
        };
        assert_eq!(decode(&encode(&ack).unwrap()).unwrap(), ack);
        assert!(encode(&proto::BridgeEnvelope { sequence: 1, ..ack }).is_err());
    }
    #[test]
    fn coding_chat_is_bounded_separate_from_native_lu_chats_and_unable_to_grant_tools() {
        let body = proto::CodingChatRequest {
            project_id: format!("ide-{}", "a".repeat(32)),
            thread_id: "ide-thread-1".into(), generation_id: "gen-1".into(),
            prompt: "Review this Rust code. No commands.".into(),
            history: vec![proto::CodingChatTurn {
                role: "user".into(), text: "Question about source".into()
            }],
            max_new_tokens: 512,
        };
        let mut msg = proto::BridgeEnvelope {
            protocol_version: 1, request_id: "coding-1".into(),
            session_id: "b".repeat(32), sequence: 1,
            body: Some(Body::ChatRequest(body)),
        };
        assert_eq!(decode(&encode(&msg).unwrap()).unwrap(), msg);
        if let Some(Body::ChatRequest(chat)) = &mut msg.body {
            chat.prompt = "x".repeat(8193);
        }
        assert!(encode(&msg).is_err());
        if let Some(Body::ChatRequest(chat)) = &mut msg.body {
            chat.prompt = "What is wrong here?".into();
            chat.history[0].role = "system".into();
        }
        assert!(encode(&msg).is_err());
        let schema = include_str!("../../proto/lu/ide/v1/bridge.proto");
        for forbidden in ["absolute_path =","shell_command =","tool_grant =",
            "access_token =","native_chat_id =","executable_path ="] {
            assert!(!schema.contains(forbidden));
        }
    }
    #[test]
    fn source_context_and_proposals_are_bounded_and_project_scoped() {
        let id = format!("ide-{}", "a".repeat(32));
        let base = proto::BridgeEnvelope {
            protocol_version: 1, request_id: "task-1".into(),
            session_id: "b".repeat(32), sequence: 1,
            body: Some(Body::ContextOffer(ProjectContextOffer {
                project_id: id.clone(), thread_id: "ide-thread-1".into(),
                source_revision: "c".repeat(64), summary: "selected summary".into(),
            })),
        };
        assert!(encode(&base).is_ok());
        let mut invalid = base.clone();
        if let Some(Body::ContextOffer(c)) = &mut invalid.body {
            c.project_id = "../../etc".into();
        }
        assert!(encode(&invalid).is_err());
        let bad_proposal = proto::BridgeEnvelope {
            body: Some(Body::ProposalOffer(ProposedChangeOffer {
                project_id: id, thread_id: "thread".into(), task_id: "t-1".into(),
                proposal_json: vec![b'x'; MAX_PROPOSAL_BYTES + 1],
            })),
            ..base
        };
        assert!(encode(&bad_proposal).is_err());
    }
}
