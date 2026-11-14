//! Internal JSON validation at Lu's model/native-tool trust boundary.
//! No user-facing JSON file inspection or model-advertised validator tool.
//! Inbound model envelopes may receive bounded punctuation-only repair,
//! but argument payloads and outbound host feedback never receive repair.
//! A successful parse validates JSON syntax, not capabilities or execution.

use serde::{
    de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor},
    Serialize,
};
use serde_json::{Map, Number, Value};
use std::fmt;

pub const MAX_BOUNDARY_JSON_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonRepairStatus {
    Unchanged,
    RemovedFence,
    RemovedTrailingCommas,
    RemovedFenceAndTrailingCommas,
    RemovedThinkingPrefix,
    RemovedThinkingPrefixAndTrailingCommas,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonBoundaryError {
    TooLarge,
    Malformed,
    DuplicateKey,
    NonObject,
    Serialization,
    InvalidEnvelope,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModelJson {
    pub object: Map<String, Value>,
    pub repair: JsonRepairStatus,
}

struct UniqueValueSeed;

impl<'de> DeserializeSeed<'de> for UniqueValueSeed {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

struct UniqueValueVisitor;

impl<'de> Visitor<'de> for UniqueValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("nonfinite JSON number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = seq.next_element_seed(UniqueValueSeed)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON object key"));
            }
            values.insert(key, map.next_value_seed(UniqueValueSeed)?);
        }
        Ok(Value::Object(values))
    }
}

fn strict_parse(raw: &str) -> Result<Value, JsonBoundaryError> {
    let mut de = serde_json::Deserializer::from_str(raw);
    let value = UniqueValueSeed.deserialize(&mut de).map_err(|error| {
        if error.to_string().contains("duplicate JSON object key") {
            JsonBoundaryError::DuplicateKey
        } else {
            JsonBoundaryError::Malformed
        }
    })?;
    de.end().map_err(|_| JsonBoundaryError::Malformed)?;
    Ok(value)
}

fn strip_exact_json_fence(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    let content = trimmed
        .strip_prefix("```json\n")
        .or_else(|| trimmed.strip_prefix("```\n"))?;
    content.strip_suffix("\n```")
}

/// Strip only one completely closed, bounded model-owned reasoning prefix.
/// Original bytes remain in Lu's raw model audit; the result still needs
/// a strict complete JSON envelope, codec, task grant and host policy.
fn strip_closed_thinking_prefix(raw: &str) -> Result<Option<&str>, JsonBoundaryError> {
    let leading = raw.trim_start();
    let Some(reasoning) = leading.strip_prefix("<think>") else {
        return Ok(None);
    };
    let (private, rest) = reasoning.split_once("</think>")
        .ok_or(JsonBoundaryError::Malformed)?;
    if private.len() > 16 * 1024
        || private.contains("<think>")
        || private.contains("<|")
        || private.contains("</think")
    {
        return Err(JsonBoundaryError::Malformed);
    }
    let response = rest.trim();
    if response.is_empty()
        || response.starts_with("~~~")
        || response.contains("<|im_start|>")
        || response.contains("<|im_end|>")
        || response.contains("<think>")
        || response.contains("</think>")
    {
        return Err(JsonBoundaryError::Malformed);
    }
    Ok(Some(response))
}

/// Remove a comma only when followed by optional whitespace and a closing
/// object/array delimiter; string literals (including escapes) are untouched.
fn strip_trailing_commas(raw: &str) -> (String, bool) {
    let bytes = raw.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut in_string = false;
    let mut escaped = false;
    let mut changed = false;
    for (i, &byte) in bytes.iter().enumerate() {
        if in_string {
            output.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        if byte == b'"' {
            in_string = true;
        } else if byte == b',' {
            let mut next = i + 1;
            while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                next += 1;
            }
            if next < bytes.len() && matches!(bytes[next], b'}' | b']') {
                changed = true;
                continue;
            }
        }
        output.push(byte);
    }
    (
        String::from_utf8(output).expect("UTF-8 bytes were only removed"),
        changed,
    )
}

/// Parse complete model output. At most one bounded syntax-only repair is
/// proposed and then subjected to the same strict duplicate-safe parser.
/// Truncation, ambiguous escapes and missing required properties fail closed.
pub fn parse_model_boundary(raw: &str) -> Result<ParsedModelJson, JsonBoundaryError> {
    if raw.len() > MAX_BOUNDARY_JSON_BYTES {
        return Err(JsonBoundaryError::TooLarge);
    }
    let (normalized, had_reasoning) = match strip_closed_thinking_prefix(raw)? {
        Some(value) => (value, true),
        None => (raw, false),
    };
    match strict_parse(normalized) {
        Ok(Value::Object(object)) => {
            return Ok(ParsedModelJson {
                object,
                repair: if had_reasoning {
                    JsonRepairStatus::RemovedThinkingPrefix
                } else {
                    JsonRepairStatus::Unchanged
                },
            })
        }
        Ok(_) => return Err(JsonBoundaryError::NonObject),
        Err(JsonBoundaryError::DuplicateKey) => return Err(JsonBoundaryError::DuplicateKey),
        Err(_) => {}
    }
    // Do not combine a reasoning prefix and a Markdown fence into a
    // multi-stage speculative repair.
    if had_reasoning && strip_exact_json_fence(normalized).is_some() {
        return Err(JsonBoundaryError::Malformed);
    }
    let (candidate, had_fence) = match strip_exact_json_fence(normalized) {
        Some(value) => (value, true),
        None => (normalized, false),
    };
    let (repaired, had_comma) = strip_trailing_commas(candidate);
    if !had_fence && !had_comma && !had_reasoning {
        return Err(JsonBoundaryError::Malformed);
    }
    if repaired.len() > MAX_BOUNDARY_JSON_BYTES {
        return Err(JsonBoundaryError::TooLarge);
    }
    let object = match strict_parse(&repaired)? {
        Value::Object(object) => object,
        _ => return Err(JsonBoundaryError::NonObject),
    };
    let repair = match (had_reasoning, had_fence, had_comma) {
        (true, false, true) => JsonRepairStatus::RemovedThinkingPrefixAndTrailingCommas,
        (true, false, false) => JsonRepairStatus::RemovedThinkingPrefix,
        (false, true, true) => JsonRepairStatus::RemovedFenceAndTrailingCommas,
        (false, true, false) => JsonRepairStatus::RemovedFence,
        (false, false, true) => JsonRepairStatus::RemovedTrailingCommas,
        (false, false, false) => JsonRepairStatus::Unchanged,
        (true, true, _) => return Err(JsonBoundaryError::Malformed),
    };
    Ok(ParsedModelJson { object, repair })
}


/**
 * Validate an inbound JSON object whose bytes are already committed to a
 * schema-specific operation (for example a model tool's argument object).
 *
 * Unlike parse_model_boundary, this function never repairs syntax. The
 * strict parser rejects duplicate field names at every depth, invalid
 * escapes, unfinished input and any trailing tokens, before a Protobuf
 * codec or host dispatcher can receive the object.
 */
pub fn parse_exact_boundary_object(
    raw: &str,
    limit: usize,
) -> Result<Map<String, Value>, JsonBoundaryError> {
    if raw.len() > MAX_BOUNDARY_JSON_BYTES || raw.len() > limit {
        return Err(JsonBoundaryError::TooLarge);
    }
    match strict_parse(raw)? {
        Value::Object(value) => Ok(value),
        _ => Err(JsonBoundaryError::NonObject),
    }
}

/// Validate a complete tool result about to leave Lu Core toward the model.
/// The shape, tool identity and success/error alternative are host protocol
/// fields, not model-supplied claims. A valid result here does not grant
/// another tool call or approve any filesystem/network effect.
pub fn parse_host_tool_feedback_boundary(
    raw: &str,
    limit: usize,
) -> Result<Map<String, Value>, JsonBoundaryError> {
    let root = parse_exact_boundary_object(raw, limit)?;
    if root.len() != 4
        || root.get("kind").and_then(Value::as_str) != Some("tool_result")
    {
        return Err(JsonBoundaryError::InvalidEnvelope);
    }
    let call_id = root.get("call_id").and_then(Value::as_str)
        .ok_or(JsonBoundaryError::InvalidEnvelope)?;
    let tool_name = root.get("name").and_then(Value::as_str)
        .ok_or(JsonBoundaryError::InvalidEnvelope)?;
    if call_id.trim().is_empty() || call_id.len() > 128
        || tool_name.is_empty() || tool_name.len() > 64
        || !tool_name.bytes().enumerate().all(|(i,c)| {
            c.is_ascii_alphabetic() || (i > 0 && (c.is_ascii_digit() || c == b'_'))
        })
    {
        return Err(JsonBoundaryError::InvalidEnvelope);
    }
    let response = root.get("response").and_then(Value::as_object)
        .ok_or(JsonBoundaryError::InvalidEnvelope)?;
    if response.len() != 3
        || response.get("tool").and_then(Value::as_str) != Some(tool_name)
    {
        return Err(JsonBoundaryError::InvalidEnvelope);
    }
    match response.get("ok").and_then(Value::as_bool) {
        Some(true) if response.contains_key("result") && !response.contains_key("error") => {}
        Some(false) if response.contains_key("error") && !response.contains_key("result") => {
            let error = response.get("error").and_then(Value::as_object)
                .ok_or(JsonBoundaryError::InvalidEnvelope)?;
            if error.len() != 3
                || error.get("code").and_then(Value::as_str)
                    .is_none_or(|code| code.is_empty() || code.len() > 128)
                || error.get("message").and_then(Value::as_str)
                    .is_none_or(|msg| msg.len() > 4096)
                || error.get("retryable").and_then(Value::as_bool).is_none()
            {
                return Err(JsonBoundaryError::InvalidEnvelope);
            }
        }
        _ => return Err(JsonBoundaryError::InvalidEnvelope),
    }
    Ok(root)
}

/// Host-generated tool feedback has a fixed, mutually exclusive success or
/// failure envelope. Serialize and validate it BEFORE any prompt emission,
/// audit handoff or further model generation. No auto-repair is performed.
pub fn serialize_host_tool_feedback_boundary(
    call_id: &str,
    tool_name: &str,
    response: &Value,
    limit: usize,
) -> Result<String, JsonBoundaryError> {
    let raw = serialize_host_boundary(
        &serde_json::json!({
            "kind": "tool_result",
            "call_id": call_id,
            "name": tool_name,
            "response": response,
        }),
        limit,
    )?;
    parse_host_tool_feedback_boundary(&raw, limit)?;
    Ok(raw)
}

/// Host-to-model results use serialization, bounds and strict validation.
/// We do not "repair" invalid internal Protobuf or invent output data.
pub fn serialize_host_boundary<T: Serialize + ?Sized>(
    value: &T,
    limit: usize,
) -> Result<String, JsonBoundaryError> {
    let raw = serde_json::to_string(value).map_err(|_| JsonBoundaryError::Serialization)?;
    if raw.len() > limit || raw.len() > MAX_BOUNDARY_JSON_BYTES {
        return Err(JsonBoundaryError::TooLarge);
    }
    strict_parse(&raw)?;
    Ok(raw)
}
