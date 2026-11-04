//! TOOLS-05 integration: real Draft 2020-12 semantics and host admission.
//! No external network or filesystem access is needed for the validator.
use std::collections::BTreeSet;

use lu_core::{
    agent_tool_loop::{NativeToolTurn, ToolTurnError, ToolTurnOutcome},
    contracts::{
        AuthorizationError, ToolAuthorizer, ToolDescriptor, ToolEffect, ToolInvocation,
    },
    conversation_authority::{
        AuthorizedCapability, BoundCallOrigin, ConversationAuthority, TaskAuthority,
        TrustedUserTurnVerifier,
    },
    model::ModelToolCatalog,
    native_schema_validation::register_schema_validation_tool,
    registry::{DispatchError, NativeToolRegistry},
    wire::proto,
};
use prost::Message;
use serde_json::{json, Value};

struct Verified;
impl TrustedUserTurnVerifier for Verified {
    fn verify_user_turn(&self, conversation: &str, turn: &str) -> bool {
        conversation == "validate-chat" && turn == "validate-user"
    }
}
struct Allow;
impl ToolAuthorizer for Allow {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor)
        -> Result<(), AuthorizationError> { Ok(()) }
}
struct Deny;
impl ToolAuthorizer for Deny {
    fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor)
        -> Result<(), AuthorizationError> {
        Err(AuthorizationError::new("user did not authorize validation"))
    }
}
fn setup() -> (NativeToolRegistry, ModelToolCatalog) {
    let r = NativeToolRegistry::new();
    let mut c = ModelToolCatalog::new();
    register_schema_validation_tool(&r, &mut c).unwrap();
    (r, c)
}
fn authority() -> (ConversationAuthority, BoundCallOrigin, BTreeSet<String>) {
    let a = ConversationAuthority::new();
    let origin = BoundCallOrigin {
        conversation_id: "validate-chat".into(),
        user_turn_id: "validate-user".into(),
        task_id: "validate-task".into(),
    };
    a.open_user_task(TaskAuthority {
        conversation_id: origin.conversation_id.clone(),
        user_turn_id: origin.user_turn_id.clone(),
        task_id: origin.task_id.clone(),
        execution_scope_id: "validate-scope".into(),
        capabilities: vec![AuthorizedCapability {
            tool_id: "lu.validate.schema".into(),
            effect: ToolEffect::ReadWorkspace,
        }],
        expires_at: None,
    }, &Verified).unwrap();
    (a, origin, ["lu_validate_schema".to_string()].into_iter().collect())
}
async fn call(schema: Value, instance: Value) -> Value {
    let (r, c) = setup();
    let (a, origin, allowed) = authority();
    let mut turn = NativeToolTurn::new_read_only(
        &c, &r, &a, origin, "validate-scope".into(), &allowed,
    ).unwrap();
    let frame = json!({"kind":"tool","name":"lu_validate_schema",
        "arguments":{"schema":schema,"data":instance}}).to_string();
    let out = turn.handle_model_reply(&frame, &c, &r, &a, &Allow).await.unwrap();
    let ToolTurnOutcome::ToolResult {data,..} = out else { panic!("must be typed tool result") };
    assert_eq!(turn.step_count(),1);
    data
}

#[tokio::test]
async fn validates_actual_json_schema_semantics_and_issues() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["name", "age"],
        "additionalProperties": false,
        "properties": {
            "name": {"type":"string", "minLength":2},
            "age":{"type":"integer", "minimum":0}
        }
    });
    let success = call(schema.clone(), json!({"name":"Taylor","age":32})).await;
    assert_eq!(success["ok"], true);
    assert_eq!(success["result"]["valid"], true);
    assert_eq!(success["result"]["draft"], "2020-12");
    assert_eq!(success["result"]["issues"].as_array().unwrap().len(),0);
    let fail = call(schema, json!({"name":"A","age":-2,"extra":9})).await;
    assert_eq!(fail["ok"],true,"data is invalid, tool execution is not");
    assert_eq!(fail["result"]["valid"],false);
    assert!(!fail["result"]["issues"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn supports_nested_local_refs_and_safe_regex() {
    let s=json!({"type":"object","$defs":{"name":{"type":"string",
        "pattern":"^[A-Za-z]{2,10}$"}},"properties":{
        "name":{"$ref":"#/$defs/name"}},"required":["name"]});
    assert_eq!(call(s.clone(),json!({"name":"Ada"})).await["result"]["valid"],true);
    assert_eq!(call(s,json!({"name":"a1"})).await["result"]["valid"],false);
    assert_eq!(call(json!(false),json!(null)).await["result"]["valid"],false);
    assert_eq!(call(json!(true),json!(null)).await["result"]["valid"],true);
    // A "$ref" string under JSON Schema "const" is literal *data*, not an
    // instruction to fetch a remote schema: the policy must distinguish them.
    let literal=json!({"const":{"$ref":"https://example.invalid/just-data"}});
    assert_eq!(call(literal.clone(),json!({"$ref":"https://example.invalid/just-data"}))
        .await["result"]["valid"],true);
    assert_eq!(call(literal,json!({"$ref":"https://another.invalid"}))
        .await["result"]["valid"],false);
}

#[tokio::test]
async fn no_network_filesystem_schema_fetch_or_unsafe_regex() {
    for schema in [
        json!({"$ref":"https://attacker.example.invalid/sneak.json"}),
        json!({"$ref":"file:///etc/passwd"}),
        json!({"$ref":"../schema.json"}),
        json!({"$id":"https://attacker.example.invalid/schema.json", "type":"string"}),
        json!({"$dynamicRef":"#name"}),
        json!({"type":"string","pattern":"(?=a)a"}),
        json!({"patternProperties":{"(?=malicious)": {"type":"string"}}}),
        json!({"$schema":"http://json-schema.org/draft-07/schema#","type":"string"}),
        json!({"type":"made-up-type"}),
    ] {
        let outcome=call(schema,json!("test")).await;
        assert_eq!(outcome["ok"],false,"must reject unsupported schema: {outcome}");
        assert_eq!(outcome["error"]["retryable"],false);
    }
}

#[tokio::test]
async fn rejects_ambiguous_model_frames_and_noncanonical_native_proto() {
    let (r,c)=setup();
    let (a, origin, allowed)=authority();
    let schemas=c.schemas(&allowed).unwrap();
    assert_eq!(schemas.len(),1);
    let arguments=[
        r#"{"schema":{"type":"string","type":"integer"},"data":42}"#,
        r#"{"schema":{"type":"object"},"data":{},"data":[]}"#,
        r#"{"schema":{"type":"string"},"data":42,}"#,
        r#"{"schema":42,"data":"x"}"#,
        r#"{"schema":{},"unknown":"value"}"#,
        r#"{"data":"x"}"#,
    ];
    for raw in arguments {
        assert!(c.encode_call(&allowed,"lu_validate_schema","reject",None,raw).is_err(),
            "ambiguous tool argument admitted: {raw}");
    }
    let normal=c.encode_call(&allowed,"lu_validate_schema","native",
        None,r#"{"schema":{"type":"integer"},"data":42}"#).unwrap();
    assert!(matches!(r.dispatch(normal.clone(),&Deny).await,Err(DispatchError::Denied(_))));
    let mut unknown=normal.clone();
    unknown.input.bytes.extend_from_slice(&[0x78,0x01]);
    assert!(matches!(r.dispatch(unknown,&Allow).await,Err(DispatchError::Execution(_))));
    let mut wrong_type=normal.clone();
    wrong_type.input.type_url="type.googleapis.com/lu.v1.ReadTextFileRequest".into();
    assert!(r.dispatch(wrong_type,&Allow).await.is_err());
    // Direct Protobuf spoofing cannot bypass the canonical-data parser.
    let forged=proto::SchemaValidateRequest {
        schema_json:br#"{"type":"string","type":"integer"}"#.to_vec(),
        instance_json:b"42".to_vec(),
    };
    let mut bad=normal;
    bad.input.bytes=forged.encode_to_vec();
    assert!(matches!(r.dispatch(bad,&Allow).await,Err(DispatchError::Execution(_))));
    let mut turn=NativeToolTurn::new_read_only(
        &c,&r,&a,origin,"validate-scope".into(),&allowed,
    ).unwrap();
    a.revoke_task("validate-task").unwrap();
    assert_eq!(turn.handle_model_reply(
        r#"{"kind":"tool","name":"lu_validate_schema",
            "arguments":{"schema":{"type":"integer"},"data":42}}"#,
        &c,&r,&a,&Allow,
    ).await,Err(ToolTurnError::Revoked));
    assert_eq!(turn.step_count(),0);
}

#[test]
fn model_tool_rejects_unbounded_schema_and_instance_before_native_dispatch() {
    let (_r,c)=setup();
    let allowed: BTreeSet<String>=["lu_validate_schema".into()].into_iter().collect();
    let big_schema=json!({"description":"x".repeat(17000)});
    let oversized=json!({"schema":big_schema,"data":1}).to_string();
    assert!(c.encode_call(&allowed,"lu_validate_schema","schema-large",None,&oversized).is_err());
    let big_data="d".repeat(33000);
    let oversized=json!({"schema":{"type":"string"},"data":big_data}).to_string();
    assert!(c.encode_call(&allowed,"lu_validate_schema","data-large",None,&oversized).is_err());
}

#[tokio::test]
async fn reports_bounded_validation_diagnostics_without_payload_repair() {
    let schema=json!({"type":"array","items":{"type":"integer"}});
    let input=json!(["a","b","c","d","e","f","g","h","i","j","k","l"]);
    let response=call(schema,input).await;
    assert_eq!(response["ok"],true);
    assert_eq!(response["result"]["valid"],false);
    assert_eq!(response["result"]["issues"].as_array().unwrap().len(),8);
    assert_eq!(response["result"]["truncated"],true);
    assert_eq!(response["result"]["note"].as_str().unwrap().contains("never"),false);
}
