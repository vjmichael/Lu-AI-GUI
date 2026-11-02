//! JSON-to-Protobuf trust boundary for Lu's model-generated tool calls.
//! Repair syntax only; never infer names, paths, missing values or permissions.

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonBoundaryError {
    TooLarge,
    Malformed,
    DuplicateKey,
    NonObject,
    Serialization,
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
    match strict_parse(raw) {
        Ok(Value::Object(object)) => {
            return Ok(ParsedModelJson {
                object,
                repair: JsonRepairStatus::Unchanged,
            })
        }
        Ok(_) => return Err(JsonBoundaryError::NonObject),
        Err(JsonBoundaryError::DuplicateKey) => return Err(JsonBoundaryError::DuplicateKey),
        Err(_) => {}
    }
    let (candidate, had_fence) = match strip_exact_json_fence(raw) {
        Some(value) => (value, true),
        None => (raw, false),
    };
    let (repaired, had_comma) = strip_trailing_commas(candidate);
    if !had_fence && !had_comma {
        return Err(JsonBoundaryError::Malformed);
    }
    if repaired.len() > MAX_BOUNDARY_JSON_BYTES {
        return Err(JsonBoundaryError::TooLarge);
    }
    let object = match strict_parse(&repaired)? {
        Value::Object(object) => object,
        _ => return Err(JsonBoundaryError::NonObject),
    };
    let repair = match (had_fence, had_comma) {
        (true, true) => JsonRepairStatus::RemovedFenceAndTrailingCommas,
        (true, false) => JsonRepairStatus::RemovedFence,
        (false, true) => JsonRepairStatus::RemovedTrailingCommas,
        (false, false) => JsonRepairStatus::Unchanged,
    };
    Ok(ParsedModelJson { object, repair })
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
