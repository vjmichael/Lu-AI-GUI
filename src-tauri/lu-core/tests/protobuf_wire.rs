use lu_core::{
    contracts::{ProtoPayload, ToolFailure, ToolInvocation, ToolOutput},
    registry::DispatchError,
    wire::{self, proto, WireError},
};
use prost::Message;

const INPUT: &str = "type.googleapis.com/lu.v1.LegacyJsonArguments";
const OUTPUT: &str = "type.googleapis.com/lu.v1.LegacyToolResult";

#[test]
fn tool_call_roundtrips_through_generated_protobuf() {
    let args = proto::LegacyJsonArguments {
        json: r#"{"path":"notes.md"}"#.into(),
    };
    let original = ToolInvocation {
        call_id: "call-1".into(),
        tool_id: "lu.core.read".into(),
        workspace_id: Some("workspace-1".into()),
        input: ProtoPayload {
            type_url: INPUT.into(),
            bytes: args.encode_to_vec(),
        },
    };
    let bytes = wire::encode_invocation(&original).unwrap();
    let decoded = wire::decode_invocation(&bytes).unwrap();
    assert_eq!(original, decoded);
    let parsed = proto::LegacyJsonArguments::decode(decoded.input.bytes.as_slice()).unwrap();
    assert!(parsed.json.contains("notes.md"));
}

#[test]
fn unsupported_contract_versions_are_refused() {
    let value = proto::ToolCall {
        contract_version: 99,
        call_id: "x".into(),
        tool_id: "lu.core.read".into(),
        workspace_id: None,
        input_type_url: INPUT.into(),
        input: vec![],
    };
    assert_eq!(
        wire::decode_invocation(&value.encode_to_vec()),
        Err(WireError::UnsupportedContractVersion(99)),
    );
}

#[test]
fn missing_or_malformed_calls_are_rejected() {
    assert!(matches!(
        wire::decode_invocation(&[0xff]),
        Err(WireError::Decode(_))
    ));
    assert_eq!(
        wire::decode_invocation(&vec![0u8; wire::MAX_CALL_BYTES + 1]),
        Err(WireError::Oversized),
    );
}

#[test]
fn success_and_error_replies_use_separate_oneof_branches() {
    let ok = Ok(ToolOutput {
        payload: ProtoPayload {
            type_url: OUTPUT.into(),
            bytes: vec![1, 2],
        },
    });
    let frame = wire::encode_reply("c-1", &ok).unwrap();
    let reply = wire::decode_reply(&frame).unwrap();
    assert!(matches!(
        reply.outcome,
        Some(proto::tool_reply::Outcome::Output(_))
    ));

    let denied = Err(DispatchError::Denied("no permission".into()));
    let frame = wire::encode_reply("c-2", &denied).unwrap();
    let reply = wire::decode_reply(&frame).unwrap();
    assert!(matches!(
        reply.outcome,
        Some(proto::tool_reply::Outcome::Failure(_))
    ));

    let draining = Err(DispatchError::ModuleDraining("lu.reader".into()));
    let frame = wire::encode_reply("c-draining", &draining).unwrap();
    let reply = wire::decode_reply(&frame).unwrap();
    assert!(matches!(
        reply.outcome,
        Some(proto::tool_reply::Outcome::Failure(proto::ToolError {
            code,
            retryable: true,
            ..
        })) if code == "EXTENSION_DRAINING"
    ));

    let failed = Err(DispatchError::Execution(ToolFailure::new(
        "IO_ERROR",
        "device unavailable",
        true,
    )));
    let frame = wire::encode_reply("c-3", &failed).unwrap();
    let reply = wire::decode_reply(&frame).unwrap();
    assert!(matches!(
        reply.outcome,
        Some(proto::tool_reply::Outcome::Failure(proto::ToolError {
            retryable: true,
            ..
        }))
    ));
}
