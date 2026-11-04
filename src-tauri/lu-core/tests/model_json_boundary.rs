//! Untrusted local model JSON must be parsed, optionally repaired with
//! punctuation-only rules, then validated by the Protobuf/native tool layer.
//! Outbound tool results are serialized, not heuristically repaired.

use lu_core::model_json_boundary::{
    parse_exact_boundary_object, parse_host_tool_feedback_boundary,
    parse_model_boundary, serialize_host_boundary, serialize_host_tool_feedback_boundary,
    JsonBoundaryError, JsonRepairStatus,
    MAX_BOUNDARY_JSON_BYTES,
};
use serde_json::json;

#[test]
fn valid_tool_call_is_untouched() {
    let raw = r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt"}}"#;
    let parsed = parse_model_boundary(raw).unwrap();
    assert_eq!(parsed.repair, JsonRepairStatus::Unchanged);
    assert_eq!(parsed.object["name"], "lu_files_read_text");
    assert_eq!(parsed.object["arguments"]["path"], "notes.txt");
}

#[test]
fn exact_markdown_json_fence_is_repaired_without_touching_values() {
    let fenced = "```json\n{\"kind\":\"tool\",\"name\":\"lu_files_read_text\",\"arguments\":{\"path\":\"notes.txt\"}}\n```";
    let parsed = parse_model_boundary(fenced).unwrap();
    assert_eq!(parsed.repair, JsonRepairStatus::RemovedFence);
    assert_eq!(parsed.object["arguments"]["path"], "notes.txt");
}

#[test]
fn nested_trailing_commas_are_repaired_only_outside_strings() {
    let raw = r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"a,}.txt",},"data":["x,]",],}"#;
    let parsed = parse_model_boundary(raw).unwrap();
    assert_eq!(parsed.repair, JsonRepairStatus::RemovedTrailingCommas);
    assert_eq!(parsed.object["arguments"]["path"], "a,}.txt");
    assert_eq!(parsed.object["data"][0], "x,]");
}

#[test]
fn fences_and_trailing_comma_can_be_repaired_in_one_pass() {
    let raw = "```json\n{\"kind\":\"tool\",\"name\":\"lu_files_read_text\",\"arguments\":{\"path\":\"notes.txt\",},}\n```";
    let parsed = parse_model_boundary(raw).unwrap();
    assert_eq!(
        parsed.repair,
        JsonRepairStatus::RemovedFenceAndTrailingCommas
    );
    assert_eq!(parsed.object["arguments"]["path"], "notes.txt");
}

#[test]
fn duplicate_keys_are_rejected_at_every_depth() {
    let root = r#"{"kind":"final","kind":"tool","name":"lu_files_read_text","arguments":{}}"#;
    let args = r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"safe.txt","path":"secret.txt"}}"#;
    assert_eq!(
        parse_model_boundary(root),
        Err(JsonBoundaryError::DuplicateKey)
    );
    assert_eq!(
        parse_model_boundary(args),
        Err(JsonBoundaryError::DuplicateKey)
    );
    let with_trailing = r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"safe.txt","path":"secret.txt",},}"#;
    assert_eq!(
        parse_model_boundary(with_trailing),
        Err(JsonBoundaryError::DuplicateKey)
    );
}

#[test]
fn no_guessing_truncated_output_or_invalid_escapes() {
    let unfinished =
        r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"notes.txt""#;
    let wrong_escape =
        r#"{"kind":"tool","name":"lu_files_read_text","arguments":{"path":"C:\Users\file.txt"}}"#;
    let missing_key = r#"{"kind":"tool","arguments":{"path":"notes.txt",},}"#;
    assert_eq!(
        parse_model_boundary(unfinished),
        Err(JsonBoundaryError::Malformed)
    );
    assert_eq!(
        parse_model_boundary(wrong_escape),
        Err(JsonBoundaryError::Malformed)
    );
    // Formatting repair doesn't invent a missing tool name.
    let parsed = parse_model_boundary(missing_key).unwrap();
    assert!(!parsed.object.contains_key("name"));
}

#[test]
fn rejects_extra_prose_arrays_and_oversized_output() {
    let prose = "Please run this: {\"kind\":\"tool\"}";
    assert_eq!(
        parse_model_boundary(prose),
        Err(JsonBoundaryError::Malformed)
    );
    assert_eq!(
        parse_model_boundary("[]"),
        Err(JsonBoundaryError::NonObject)
    );
    let too_large = " ".repeat(MAX_BOUNDARY_JSON_BYTES + 1);
    assert_eq!(
        parse_model_boundary(&too_large),
        Err(JsonBoundaryError::TooLarge)
    );
}

#[test]
fn outgoing_host_feedback_is_strict_serialized_json() {
    let value = json!({
        "kind":"tool_result",
        "name":"lu_files_read_text",
        "response":{"ok":true,"result":{"content":"Lu\n\"evidence\""}}
    });
    let raw = serialize_host_boundary(&value, MAX_BOUNDARY_JSON_BYTES).unwrap();
    let parsed = parse_model_boundary(&raw).unwrap();
    assert_eq!(parsed.repair, JsonRepairStatus::Unchanged);
    assert_eq!(
        parsed.object["response"]["result"]["content"],
        "Lu\n\"evidence\""
    );
    assert_eq!(
        serialize_host_boundary(&value, 8),
        Err(JsonBoundaryError::TooLarge)
    );
}

#[test]
fn strict_tool_arguments_cannot_be_repaired_or_reinterpreted() {
    let valid = r#"{"path":"report.json","metadata":{"nested":true}}"#;
    assert_eq!(
        parse_exact_boundary_object(valid, MAX_BOUNDARY_JSON_BYTES).unwrap()["path"],
        "report.json"
    );
    for bad in [
        r#"{"path":"safe.txt","path":"evil.txt"}"#,
        r#"{"path":"safe.txt","metadata":{"k":1,"k":2}}"#,
        r#"{"path":"safe.txt",}"#,
        "&#123;not json}",
        r#"{"path":"broken\"#,
        r#"[{"path":"safe.txt"}]"#,
    ] {
        assert!(
            parse_exact_boundary_object(bad, MAX_BOUNDARY_JSON_BYTES).is_err(),
            "unsafe JSON arguments accepted: {bad:?}"
        );
    }
    assert_eq!(
        parse_exact_boundary_object(valid, 5),
        Err(JsonBoundaryError::TooLarge)
    );
}

#[test]
fn host_feedback_must_have_exact_identity_success_or_failure_semantics() {
    let success = json!({
        "ok":true,
        "tool":"lu_files_read_text",
        "result":{"content":"A string with escaped \"quotes\" and unicode ✓"}
    });
    let raw = serialize_host_tool_feedback_boundary(
        "lu-call-1", "lu_files_read_text", &success, MAX_BOUNDARY_JSON_BYTES,
    ).unwrap();
    let parsed = parse_host_tool_feedback_boundary(&raw, MAX_BOUNDARY_JSON_BYTES).unwrap();
    assert_eq!(parsed["response"]["result"]["content"], "A string with escaped \"quotes\" and unicode ✓");

    let rejected = json!({
        "ok":false,"tool":"lu_files_read_text",
        "error":{"code":"PERMISSION_DENIED","message":"denied","retryable":false}
    });
    let raw = serialize_host_tool_feedback_boundary(
        "lu-call-2", "lu_files_read_text", &rejected, MAX_BOUNDARY_JSON_BYTES,
    ).unwrap();
    let parsed = parse_host_tool_feedback_boundary(&raw, MAX_BOUNDARY_JSON_BYTES).unwrap();
    assert_eq!(parsed["response"]["error"]["code"], "PERMISSION_DENIED");

    for corrupt in [
        r#"{"kind":"tool_result","call_id":"c","name":"lu_files_read_text","response":{"ok":true,"tool":"lu_files_read_text"}}"#,
        r#"{"kind":"tool_result","call_id":"c","name":"lu_files_read_text","response":{"ok":true,"tool":"lu_files_read_text","result":null,"error":{"code":"A","message":"","retryable":false}}}"#,
        r#"{"kind":"tool_result","call_id":"c","name":"lu_files_read_text","response":{"ok":true,"tool":"lu_files_other","result":null}}"#,
        r#"{"kind":"tool_result","call_id":"c","name":"lu_files_read_text","response":{"ok":false,"tool":"lu_files_read_text","result":null}}"#,
        r#"{"kind":"tool_result","call_id":"c","name":"lu_files_read_text","response":{"ok":true,"tool":"lu_files_read_text","result":null},"unknown":1}"#,
        r#"{"kind":"tool_result","kind":"tool_result","call_id":"c","name":"lu_files_read_text","response":{"ok":true,"tool":"lu_files_read_text","result":null}}"#,
        r#"{"kind":"tool_result","call_id":"c","name":"lu_files_read_text","response":{"ok":true,"tool":"lu_files_read_text","result":null,}}"#,
    ] {
        assert!(parse_host_tool_feedback_boundary(corrupt, MAX_BOUNDARY_JSON_BYTES).is_err(),
            "invalid host JSON escaped the boundary: {corrupt}");
    }
    assert_eq!(
        serialize_host_tool_feedback_boundary(
            "c", "lu_files_read_text", &json!({"ok":true}), MAX_BOUNDARY_JSON_BYTES,
        ),
        Err(JsonBoundaryError::InvalidEnvelope)
    );
    assert_eq!(
        serialize_host_tool_feedback_boundary(
            "c", "lu_files_read_text", &success, 20,
        ),
        Err(JsonBoundaryError::TooLarge)
    );
}

#[test]
fn outbound_host_serializer_cannot_emit_duplicate_keys_even_from_custom_serializer() {
    use serde::ser::SerializeMap;
    struct DuplicateKeys;
    impl serde::Serialize for DuplicateKeys {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok,S::Error> {
            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("grant", &"none")?;
            map.serialize_entry("grant", &"administrator")?;
            map.end()
        }
    }
    assert_eq!(
        serialize_host_boundary(&DuplicateKeys, MAX_BOUNDARY_JSON_BYTES),
        Err(JsonBoundaryError::DuplicateKey)
    );
}
