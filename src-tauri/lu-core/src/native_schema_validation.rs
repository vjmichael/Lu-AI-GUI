//! TOOLS-05: Lu's model-visible, read-only schema validation tool.
//!
//! This corresponds to the original Jan-Mod-Design-Document.md §5.2
//! "validate_schema", not to TOOLS-02's *internal* JSON syntax/MBRL repair.
//! It validates caller-provided, already-complete JSON data against an
//! explicitly supplied JSON Schema 2020-12. It performs NO file I/O, NO
//! data modification, NO remote schema fetching and NO arbitrary code.
//!
//! The established jsonschema crate is used for actual JSON Schema semantics.
//! We deliberately disable its filesystem/HTTP resolvers at the Cargo level.
//! The current native effect taxonomy lacks "PureComputation"; ReadWorkspace
//! is a conservative scoped read-only admission class until that migration.
//! Its descriptor cannot authorize anything on its own.

use std::sync::Arc;

use prost::Message;
use regex::RegexBuilder;
use serde_json::{json, Map, Value};

use crate::{
    contracts::{
        ModuleKind, ModuleManifest, NativeTool, ProtoPayload, ToolDescriptor, ToolEffect,
        ToolFailure, ToolFuture, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION,
    },
    model::{expect_fields, ModelToolCatalog, ModelToolCodec, ModelToolDefinition, ModelToolError},
    registry::NativeToolRegistry,
    wire::proto,
};

const MODULE: &str = "lu.validate";
const TOOL: &str = "lu.validate.schema";
const INPUT: &str = "type.googleapis.com/lu.v1.SchemaValidateRequest";
const OUTPUT: &str = "type.googleapis.com/lu.v1.SchemaValidateResult";
const MAX_SCHEMA_BYTES: usize = 16 * 1024;
const MAX_INSTANCE_BYTES: usize = 32 * 1024;
const MAX_SCHEMA_NODES: usize = 1024;
const MAX_INSTANCE_NODES: usize = 4096;
const MAX_TREE_DEPTH: usize = 32;
const MAX_PATTERN_LENGTH: usize = 128;
const MAX_PATTERNS: usize = 32;
const MAX_REF_LENGTH: usize = 256;
const MAX_REFS: usize = 32;
const MAX_ISSUES: usize = 8;

fn rejected(code: &str, message: &str) -> ToolFailure {
    ToolFailure::new(code, message, false)
}

fn value_size(value: &Value, max: usize) -> Result<Vec<u8>, ModelToolError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| ModelToolError::InvalidArguments("cannot serialize JSON value".into()))?;
    if bytes.len() > max {
        return Err(ModelToolError::InvalidArguments(
            "schema/data exceeds the bounded validation input size".into(),
        ));
    }
    Ok(bytes)
}

/// Native dispatch must reject ad-hoc Protobuf callers that skip the model
/// codec, including alternate JSON whitespace/duplicate-key forms.
fn canonical_value(bytes: &[u8], max: usize) -> Result<Value, ToolFailure> {
    if bytes.is_empty() || bytes.len() > max {
        return Err(rejected("VALIDATION_INPUT_LIMIT", "validation JSON bytes exceed budget"));
    }
    let data: Value = serde_json::from_slice(bytes)
        .map_err(|_| rejected("VALIDATION_BAD_JSON", "invalid validation JSON"))?;
    let canonical = serde_json::to_vec(&data)
        .map_err(|_| rejected("VALIDATION_BAD_JSON", "invalid validation JSON encoding"))?;
    if canonical != bytes {
        return Err(rejected("VALIDATION_BAD_JSON", "noncanonical or ambiguous validation JSON"));
    }
    Ok(data)
}

fn bounded_tree(value: &Value, limit: usize) -> Result<(), ToolFailure> {
    fn visit(v: &Value, depth: usize, count: &mut usize, limit: usize)
        -> Result<(), ToolFailure>
    {
        if depth > MAX_TREE_DEPTH {
            return Err(rejected("VALIDATION_DEPTH_LIMIT", "validation JSON nesting limit exceeded"));
        }
        *count += 1;
        if *count > limit {
            return Err(rejected("VALIDATION_NODE_LIMIT", "validation JSON node limit exceeded"));
        }
        match v {
            Value::Array(items) => {
                for item in items { visit(item, depth + 1, count, limit)?; }
            }
            Value::Object(items) => {
                for item in items.values() { visit(item, depth + 1, count, limit)?; }
            }
            _ => {}
        }
        Ok(())
    }
    visit(value, 0, &mut 0, limit)
}

fn safe_regex(pattern: &str) -> Result<(), ToolFailure> {
    if pattern.len() > MAX_PATTERN_LENGTH {
        return Err(rejected("SCHEMA_PATTERN_LIMIT", "schema regex is too long"));
    }
    // This linear-time Rust engine intentionally rejects lookaround,
    // backreferences and other fancy-regex/backtracking-only constructs.
    RegexBuilder::new(pattern)
        .size_limit(512 * 1024)
        .dfa_size_limit(512 * 1024)
        .build()
        .map_err(|_| rejected("SCHEMA_PATTERN_UNSUPPORTED", "unsafe or unsupported regex pattern"))?;
    Ok(())
}

/// V1 policy: local static JSON Pointer refs only, never remote/file/URI refs.
/// In particular this prevents schema input from becoming a network fetch or
/// a covert file read. The jsonschema crate's resolver features are disabled.
fn schema_policy(value: &Value) -> Result<(), ToolFailure> {
    // Only traverse positions that JSON Schema actually treats as schemas.
    // "const", "enum", "default" and "examples" contain ordinary user data:
    // a JSON object there may legitimately contain a "$ref" string that is
    // NOT a reference. Treating every nested object as a schema is incorrect.
    fn check(schema: &Value, patterns: &mut usize, refs: &mut usize)
        -> Result<(), ToolFailure>
    {
        let Value::Object(map) = schema else {
            return Ok(()); // boolean schemas are valid; other types fail the meta-schema
        };
        if let Some(draft) = map.get("$schema") {
            let permitted = [
                "https://json-schema.org/draft/2020-12/schema",
                "https://json-schema.org/draft/2020-12/schema#",
                "http://json-schema.org/draft/2020-12/schema",
                "http://json-schema.org/draft/2020-12/schema#",
            ];
            if !draft.as_str().is_some_and(|s| permitted.contains(&s)) {
                return Err(rejected("SCHEMA_DRAFT_UNSUPPORTED",
                    "only Draft 2020-12 JSON Schema is admitted"));
            }
        }
        for keyword in ["$id", "$anchor", "$dynamicAnchor", "$dynamicRef",
            "$recursiveRef"] {
            if map.contains_key(keyword) {
                return Err(rejected("SCHEMA_EXTERNAL_REF",
                    "identified/dynamic schema references are not supported"));
            }
        }
        if let Some(reference) = map.get("$ref") {
            *refs += 1;
            if *refs > MAX_REFS
                || !reference.as_str().is_some_and(|s| {
                    s.len() <= MAX_REF_LENGTH && s.starts_with("#/")
                        && !s.contains('%')
                })
            {
                return Err(rejected("SCHEMA_EXTERNAL_REF",
                    "only bounded local JSON Pointer references are admitted"));
            }
        }
        if let Some(pattern) = map.get("pattern") {
            *patterns += 1;
            if *patterns > MAX_PATTERNS {
                return Err(rejected("SCHEMA_PATTERN_LIMIT", "too many schema patterns"));
            }
            safe_regex(pattern.as_str().ok_or_else(|| rejected(
                "SCHEMA_INVALID", "schema pattern must be a string"))?)?;
        }

        for keyword in ["properties", "patternProperties", "$defs", "dependentSchemas"] {
            if let Some(Value::Object(definitions)) = map.get(keyword) {
                for (name, sub_schema) in definitions {
                    if keyword == "patternProperties" {
                        *patterns += 1;
                        if *patterns > MAX_PATTERNS {
                            return Err(rejected("SCHEMA_PATTERN_LIMIT", "too many schema patterns"));
                        }
                        safe_regex(name)?;
                    }
                    check(sub_schema, patterns, refs)?;
                }
            }
        }
        for keyword in ["allOf", "anyOf", "oneOf", "prefixItems"] {
            if let Some(Value::Array(items)) = map.get(keyword) {
                for item in items {
                    check(item, patterns, refs)?;
                }
            }
        }
        for keyword in ["items", "additionalProperties", "unevaluatedProperties",
            "unevaluatedItems", "contains", "not", "if", "then", "else",
            "propertyNames", "contentSchema"] {
            if let Some(item) = map.get(keyword) {
                check(item, patterns, refs)?;
            }
        }
        Ok(())
    }
    check(value, &mut 0, &mut 0)
}

fn validate(input: proto::SchemaValidateRequest) -> Result<ToolOutput, ToolFailure> {
    let schema = canonical_value(&input.schema_json, MAX_SCHEMA_BYTES)?;
    let instance = canonical_value(&input.instance_json, MAX_INSTANCE_BYTES)?;
    bounded_tree(&schema, MAX_SCHEMA_NODES)?;
    bounded_tree(&instance, MAX_INSTANCE_NODES)?;
    if !schema.is_object() && !schema.is_boolean() {
        return Err(rejected("SCHEMA_INVALID", "schema must be a JSON object or boolean"));
    }
    schema_policy(&schema)?;
    if !jsonschema::draft202012::meta::is_valid(&schema) {
        return Err(rejected("SCHEMA_INVALID", "input is not a valid Draft 2020-12 schema"));
    }
    let compiled = jsonschema::draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|_| rejected("SCHEMA_INVALID", "schema could not be compiled"))?;

    let mut all = compiled.iter_errors(&instance);
    let mut issues = Vec::new();
    for error in all.by_ref().take(MAX_ISSUES) {
        // Do not reflect unbounded user data or model instructions in output.
        let instance_path: String = error.instance_path().to_string().chars().take(256).collect();
        let message: String = error.to_string().chars().take(240).collect();
        issues.push(proto::SchemaValidationIssue { instance_path, message });
    }
    let truncated = all.next().is_some();
    let result = proto::SchemaValidateResult {
        valid: issues.is_empty(),
        issues, truncated,
    };
    Ok(ToolOutput { payload: ProtoPayload {
        type_url: OUTPUT.into(), bytes: result.encode_to_vec(),
    } })
}

struct SchemaValidator {
    descriptor: ToolDescriptor,
}
impl NativeTool for SchemaValidator {
    fn descriptor(&self) -> &ToolDescriptor { &self.descriptor }
    fn invoke(&self, call: ToolInvocation) -> ToolFuture<'_> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let request = proto::SchemaValidateRequest::decode(call.input.bytes.as_slice())
                    .map_err(|_| rejected("BAD_PROTOBUF", "invalid native schema request"))?;
                if request.encode_to_vec() != call.input.bytes {
                    return Err(rejected("BAD_PROTOBUF", "noncanonical native schema request"));
                }
                validate(request)
            }).await.map_err(|_| rejected("SCHEMA_WORKER", "validation worker stopped"))?
        })
    }
}

struct SchemaCodec;
impl ModelToolCodec for SchemaCodec {
    fn input_type_url(&self) -> &'static str { INPUT }
    fn output_type_url(&self) -> &'static str { OUTPUT }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["schema", "data"], &["schema", "data"])?;
        let schema = args.get("schema").expect("fields checked");
        if !schema.is_object() && !schema.is_boolean() {
            return Err(ModelToolError::InvalidArguments(
                "schema must be an object or boolean".into()));
        }
        let bytes = proto::SchemaValidateRequest {
            schema_json: value_size(schema, MAX_SCHEMA_BYTES)?,
            instance_json: value_size(args.get("data").expect("fields checked"), MAX_INSTANCE_BYTES)?,
        }.encode_to_vec();
        Ok(ProtoPayload { type_url: INPUT.into(), bytes })
    }
    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        let r = proto::SchemaValidateResult::decode(output.bytes.as_slice())
            .map_err(|_| ModelToolError::InvalidResult("bad validation output".into()))?;
        if r.issues.len() > MAX_ISSUES || (r.valid && !r.issues.is_empty())
            || (!r.valid && r.issues.is_empty())
        {
            return Err(ModelToolError::InvalidResult("inconsistent validation result".into()));
        }
        Ok(json!({
            "valid": r.valid,
            "issues": r.issues.into_iter().map(|i| json!({
                "instance_path": i.instance_path, "message": i.message
            })).collect::<Vec<_>>(),
            "truncated": r.truncated,
            "draft": "2020-12",
            "note": "Validation only; no data was repaired or written. The caller supplied the schema."
        }))
    }
}

/// A trusted host must separately select and authorize this schema validation
/// capability for the current persisted user turn. Native registry and model
/// schemas are inert metadata; this call does not touch disk or the network.
pub fn register_schema_validation_tool(
    registry: &NativeToolRegistry,
    catalog: &mut ModelToolCatalog,
) -> Result<(), String> {
    let descriptor = ToolDescriptor {
        id: TOOL.into(),
        display_name: "Validate supplied JSON data against JSON Schema".into(),
        // Conservative scoped classification until Lu has an explicit Pure effect.
        effect: ToolEffect::ReadWorkspace,
        input_type_url: INPUT.into(), output_type_url: OUTPUT.into(),
    };
    registry.register(
        ModuleManifest {
            id: MODULE.into(),
            display_name: "Lu Core standard schema validation".into(),
            contract_version: CORE_CONTRACT_VERSION,
            kind: ModuleKind::Core,
            tools: vec![descriptor.clone()],
            panels: vec![], workflows: vec![],
        },
        vec![Arc::new(SchemaValidator { descriptor })],
    ).map_err(|e| format!("native schema registration refused: {e:?}"))?;
    catalog.insert(registry, ModelToolDefinition {
        name: "lu_validate_schema".into(),
        native_tool_id: TOOL.into(),
        description: "Check JSON data against a supplied Draft 2020-12 JSON Schema; never repair.".into(),
        parameters: json!({
            "type":"object",
            "properties":{
                "schema":{"description":"Complete JSON Schema 2020-12; embedded, offline only"},
                "data":{"description":"Complete JSON value to validate (any JSON type)"}
            },
            "required":["schema","data"],
            "additionalProperties":false
        }),
        codec: Arc::new(SchemaCodec),
    }).map_err(|e| format!("model schema registration refused: {e:?}"))?;
    Ok(())
}
