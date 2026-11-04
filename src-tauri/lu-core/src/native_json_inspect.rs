//! TOOLS-02: first-party, read-only JSON file inspection.
//!
//! Reads only a bounded file beneath an already user-authorized directory.
//! Lu owns the native tool registration and per-turn permissions. Model JSON
//! arguments are an untrusted external boundary; the executor receives typed
//! Protobuf and never accepts an ambient/absolute filename or write request.
//! This module does not "repair" broken files or silently overwrite them.

use cap_std::{ambient_authority, fs::Dir};
use prost::Message;
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{json, Map, Number, Value};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::{
    contracts::{
        ModuleKind, ModuleManifest, NativeTool, ProtoPayload, ToolDescriptor, ToolEffect,
        ToolFailure, ToolFuture, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION,
    },
    model::{expect_fields, optional_uint, ModelToolCatalog, ModelToolCodec, ModelToolDefinition, ModelToolError},
    registry::NativeToolRegistry,
    wire::proto,
};

pub const MAX_JSON_INSPECTION_BYTES: u32 = 512 * 1024;
pub const MAX_REPORTED_KEYS: usize = 32;
const ID: &str = "lu.json.inspect";
const MODULE: &str = "lu.json";
const MODEL_NAME: &str = "lu_json_inspect";
const INPUT: &str = "type.googleapis.com/lu.v1.InspectJsonFileRequest";
const OUTPUT: &str = "type.googleapis.com/lu.v1.InspectJsonFileResult";

fn failure(code: &str, message: &str) -> ToolFailure {
    ToolFailure::new(code, message, false)
}

fn relative_json_path(raw: &str) -> Result<PathBuf, &'static str> {
    if raw.is_empty() || raw.len() > 4096
        || raw.chars().any(|c| c.is_control() || matches!(c, '\\' | ':'))
        || Path::new(raw).is_absolute()
        || raw.split('/').any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("path must be relative to the authorized directory");
    }
    Ok(PathBuf::from(raw))
}

/// serde_json's normal Value deserializer accepts duplicate object keys,
/// silently keeping the last value. Inspection must refuse that ambiguity at
/// any nesting level rather than letting an AI trust overwritten JSON fields.
struct StrictJson(Value);

impl<'de> Deserialize<'de> for StrictJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictJson;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON value without duplicate object keys")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Null))
            }
            fn visit_bool<E: de::Error>(self, x: bool) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Bool(x)))
            }
            fn visit_i64<E: de::Error>(self, x: i64) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Number(x.into())))
            }
            fn visit_u64<E: de::Error>(self, x: u64) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::Number(x.into())))
            }
            fn visit_f64<E: de::Error>(self, x: f64) -> Result<Self::Value, E> {
                Number::from_f64(x)
                    .map(|n| StrictJson(Value::Number(n)))
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E: de::Error>(self, x: &str) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::String(x.to_owned())))
            }
            fn visit_string<E: de::Error>(self, x: String) -> Result<Self::Value, E> {
                Ok(StrictJson(Value::String(x)))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(StrictJson(value)) = seq.next_element::<StrictJson>()? {
                    values.push(value);
                }
                Ok(StrictJson(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some((key, StrictJson(value))) = map.next_entry::<String, StrictJson>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate object key"));
                    }
                    values.insert(key, value);
                }
                Ok(StrictJson(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(StrictVisitor)
    }
}

fn classify_json_error(error: &serde_json::Error) -> &'static str {
    if error.to_string().contains("duplicate object key") {
        return "DUPLICATE_KEY";
    }
    if error.to_string().contains("recursion limit exceeded") {
        return "DEPTH_LIMIT";
    }
    match error.classify() {
        serde_json::error::Category::Eof => "TRUNCATED_JSON",
        serde_json::error::Category::Syntax => "SYNTAX_ERROR",
        serde_json::error::Category::Data => "INVALID_DATA",
        serde_json::error::Category::Io => "PARSER_IO",
    }
}

/// A native OS picker can pin one exact approved file handle. The subsequent
/// model-visible path never selects an unapproved replacement or sibling.
struct PinnedJsonFile {
    name: String,
    handle: Mutex<File>,
}

fn inspect_json(
    root: &Dir,
    request: proto::InspectJsonFileRequest,
    pinned: Option<&PinnedJsonFile>,
) -> Result<ToolOutput, ToolFailure> {
    let max_bytes = request.max_bytes.unwrap_or(MAX_JSON_INSPECTION_BYTES);
    if max_bytes == 0 || max_bytes > MAX_JSON_INSPECTION_BYTES {
        return Err(failure("INVALID_LIMIT", "JSON inspection size outside approved range"));
    }
    let relative = relative_json_path(&request.path)
        .map_err(|message| failure("INVALID_PATH", message))?;
    let mut data = Vec::new();
    if let Some(pinned) = pinned {
        if request.path != pinned.name {
            return Err(failure("TARGET_MISMATCH", "JSON request differs from approved file"));
        }
        let mut handle = pinned.handle.lock()
            .map_err(|_| failure("FILE_ACCESS", "approved JSON handle unavailable"))?;
        let opened = handle.metadata()
            .map_err(|_| failure("FILE_ACCESS", "cannot inspect approved JSON handle"))?;
        if !opened.is_file() {
            return Err(failure("NOT_FILE", "approved JSON handle is not a file"));
        }
        if opened.len() > u64::from(max_bytes) {
            return Err(failure("TOO_LARGE", "approved JSON file exceeds inspection limit"));
        }
        handle.seek(SeekFrom::Start(0))
            .map_err(|_| failure("FILE_ACCESS", "cannot seek approved JSON handle"))?;
        (&mut *handle).take(u64::from(max_bytes) + 1).read_to_end(&mut data)
            .map_err(|_| failure("FILE_ACCESS", "cannot read approved JSON handle"))?;
    } else {
        let before = root.symlink_metadata(&relative)
            .map_err(|_| failure("FILE_ACCESS", "JSON target unavailable"))?;
        if !before.file_type().is_file() || before.file_type().is_symlink() {
            return Err(failure("NOT_FILE", "JSON target must be a regular file, not a link"));
        }
        if before.len() > u64::from(max_bytes) {
            return Err(failure("TOO_LARGE", "JSON file exceeds approved inspection limit"));
        }
        let mut handle = root.open(&relative)
            .map_err(|_| failure("FILE_ACCESS", "cannot open JSON target"))?;
        let opened = handle.metadata()
            .map_err(|_| failure("FILE_ACCESS", "cannot inspect opened JSON handle"))?;
        if !opened.is_file() || opened.len() > u64::from(max_bytes) {
            return Err(failure("NOT_FILE", "opened JSON target not approved"));
        }
        handle.by_ref().take(u64::from(max_bytes) + 1).read_to_end(&mut data)
            .map_err(|_| failure("FILE_ACCESS", "cannot read JSON target"))?;
    }
    if data.len() > max_bytes as usize {
        return Err(failure("TOO_LARGE", "JSON target grew beyond approved limit"));
    }
    // Digest ties syntax and shape results to exactly the observed bytes.
    // It is an observation fingerprint, NOT a file identity or edit approval.
    let digest = format!("{:x}", Sha256::digest(&data));
    let mut output = proto::InspectJsonFileResult {
        valid: false,
        root_kind: String::new(),
        bytes_read: data.len() as u64,
        content_sha256: digest,
        top_level_keys: Vec::new(),
        array_length: 0,
        error_kind: String::new(),
        error_line: 0,
        error_column: 0,
    };
    match serde_json::from_slice::<StrictJson>(&data) {
        Ok(StrictJson(value)) => {
            output.valid = true;
            output.root_kind = match &value {
                Value::Object(values) => {
                    // No values or raw source text are returned.
                    output.top_level_keys = values.keys()
                        .take(MAX_REPORTED_KEYS)
                        .map(|name| name.chars().take(128).collect())
                        .collect();
                    "object"
                }
                Value::Array(values) => {
                    output.array_length = values.len().min(u32::MAX as usize) as u32;
                    "array"
                }
                Value::String(_) => "string",
                Value::Number(_) => "number",
                Value::Bool(_) => "boolean",
                Value::Null => "null",
            }.into();
        }
        Err(error) => {
            output.error_kind = classify_json_error(&error).into();
            output.error_line = error.line().min(u32::MAX as usize) as u32;
            output.error_column = error.column().min(u32::MAX as usize) as u32;
        }
    }
    Ok(ToolOutput {
        payload: ProtoPayload {
            type_url: OUTPUT.into(),
            bytes: output.encode_to_vec(),
        },
    })
}

struct JsonInspectTool {
    descriptor: ToolDescriptor,
    root: Arc<Dir>,
    pinned: Option<Arc<PinnedJsonFile>>,
}
impl NativeTool for JsonInspectTool {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }
    fn invoke(&self, call: ToolInvocation) -> ToolFuture<'_> {
        let root = Arc::clone(&self.root);
        let pinned = self.pinned.clone();
        Box::pin(async move {
            let request = proto::InspectJsonFileRequest::decode(call.input.bytes.as_slice())
                .map_err(|_| failure("BAD_PROTOBUF", "malformed JSON inspect arguments"))?;
            if request.encode_to_vec() != call.input.bytes {
                return Err(failure("BAD_PROTOBUF", "noncanonical JSON inspect arguments"));
            }
            tokio::task::spawn_blocking(move || inspect_json(&root, request, pinned.as_deref()))
                .await
                .map_err(|_| failure("WORKER_FAILED", "JSON inspection task failed"))?
        })
    }
}

struct JsonCodec;
impl ModelToolCodec for JsonCodec {
    fn input_type_url(&self) -> &'static str { INPUT }
    fn output_type_url(&self) -> &'static str { OUTPUT }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["path", "max_bytes"], &["path"])?;
        let raw = args.get("path").and_then(Value::as_str)
            .ok_or_else(|| ModelToolError::InvalidArguments("path must be a string".into()))?;
        relative_json_path(raw).map_err(|e| ModelToolError::InvalidArguments(e.into()))?;
        let max_bytes = optional_uint(args, "max_bytes", MAX_JSON_INSPECTION_BYTES, MAX_JSON_INSPECTION_BYTES)?;
        Ok(ProtoPayload {
            type_url: INPUT.into(),
            bytes: proto::InspectJsonFileRequest {
                path: raw.to_owned(),
                max_bytes: Some(max_bytes),
            }.encode_to_vec(),
        })
    }
    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        let result = proto::InspectJsonFileResult::decode(output.bytes.as_slice())
            .map_err(|_| ModelToolError::InvalidResult("bad JSON inspection result".into()))?;
        if result.content_sha256.len() != 64 || !result.content_sha256.bytes().all(|c| c.is_ascii_hexdigit())
            || result.top_level_keys.len() > MAX_REPORTED_KEYS
        {
            return Err(ModelToolError::InvalidResult("JSON result not bounded".into()));
        }
        Ok(json!({
            "valid": result.valid,
            "root_kind": result.root_kind,
            "bytes_read": result.bytes_read,
            "content_sha256": result.content_sha256,
            "top_level_keys": result.top_level_keys,
            "array_length": result.array_length,
            "error_kind": result.error_kind,
            "error_line": result.error_line,
            "error_column": result.error_column,
        }))
    }
}

/// Register only with a real user-approved directory, then expose the model
/// schema only to turns whose Lu-native scope includes `lu.json.inspect`.
/// Successful registration is never a grant to read the directory.
pub fn register_json_inspection_tool(
    registry: &NativeToolRegistry,
    catalog: &mut ModelToolCatalog,
    authorized_root: &Path,
) -> Result<(), String> {
    register_json_inspection_tool_inner(registry, catalog, authorized_root, None)
}

/// Register a picker-bound inspector. The selected handle is opened and
/// identity-checked by trusted desktop code before this call; model text may
/// only name the same file, and never chooses the handle itself.
pub fn register_json_inspection_tool_pinned(
    registry: &NativeToolRegistry,
    catalog: &mut ModelToolCatalog,
    authorized_root: &Path,
    selected_name: &str,
    selected_handle: File,
) -> Result<(), String> {
    let path = relative_json_path(selected_name).map_err(str::to_owned)?;
    if path.file_name().and_then(|n| n.to_str()) != Some(selected_name)
        || !selected_handle.metadata().map_err(|_| "cannot inspect selected JSON file")?.is_file()
    {
        return Err("selected JSON handle/name is invalid".into());
    }
    register_json_inspection_tool_inner(
        registry, catalog, authorized_root,
        Some(Arc::new(PinnedJsonFile {
            name: selected_name.to_owned(),
            handle: Mutex::new(selected_handle),
        })),
    )
}

fn register_json_inspection_tool_inner(
    registry: &NativeToolRegistry,
    catalog: &mut ModelToolCatalog,
    authorized_root: &Path,
    pinned: Option<Arc<PinnedJsonFile>>,
) -> Result<(), String> {
    let canonical = authorized_root.canonicalize()
        .map_err(|_| "authorized JSON root unavailable")?;
    if !canonical.is_dir() {
        return Err("authorized JSON root is not a directory".into());
    }
    let root = Arc::new(Dir::open_ambient_dir(canonical, ambient_authority())
        .map_err(|_| "cannot bind JSON inspection directory")?);
    let descriptor = ToolDescriptor {
        id: ID.into(),
        display_name: "Inspect JSON syntax and top-level structure (read only)".into(),
        effect: ToolEffect::ReadWorkspace,
        input_type_url: INPUT.into(),
        output_type_url: OUTPUT.into(),
    };
    registry.register(ModuleManifest {
        id: MODULE.into(),
        display_name: "Lu native JSON validation".into(),
        contract_version: CORE_CONTRACT_VERSION,
        kind: ModuleKind::Core,
        tools: vec![descriptor.clone()],
        panels: vec![],
        workflows: vec![],
    }, vec![Arc::new(JsonInspectTool { descriptor, root, pinned })])
    .map_err(|_| "JSON native tool registry refused module")?;

    catalog.insert(registry, ModelToolDefinition {
        name: MODEL_NAME.into(),
        native_tool_id: ID.into(),
        description: "Read-only JSON file syntax and duplicate-key check with content digest and top-level shape; never modifies a file.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path beneath the user-approved directory" },
                "max_bytes": { "type": "integer", "minimum": 1, "maximum": MAX_JSON_INSPECTION_BYTES }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
        codec: Arc::new(JsonCodec),
    }).map_err(|_| "JSON model schema registration refused")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{AuthorizationError, ToolAuthorizer};
    use std::{collections::BTreeSet, fs, time::{SystemTime, UNIX_EPOCH}};
    fn temp() -> PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("lu-json-{}-{n}", std::process::id()));
        fs::create_dir(&path).unwrap();
        path
    }
    struct Allow;
    impl ToolAuthorizer for Allow {
        fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
            Ok(())
        }
    }
    struct Deny;
    impl ToolAuthorizer for Deny {
        fn authorize(&self, _: &ToolInvocation, _: &ToolDescriptor) -> Result<(), AuthorizationError> {
            Err(AuthorizationError::new("not user-approved"))
        }
    }
    fn prepare(root: &Path) -> (NativeToolRegistry, ModelToolCatalog) {
        let registry = NativeToolRegistry::new();
        let mut catalog = ModelToolCatalog::new();
        register_json_inspection_tool(&registry, &mut catalog, root).unwrap();
        (registry, catalog)
    }
    fn call(catalog: &ModelToolCatalog, name: &str, args: &str) -> ToolInvocation {
        catalog.encode_call(&BTreeSet::from([MODEL_NAME.into()]),
            MODEL_NAME, "json-call-1", Some("json-workspace-1"),
            &format!(r#"{{"path":"{name}",{args}}}"#)).unwrap()
    }
    #[tokio::test]
    async fn reads_valid_json_shape_hash_without_returning_content() {
        let root = temp();
        fs::write(root.join("film.json"), r#"{"shot":1,"assets":["a","b"],"private":"SECRET_VALUE"}"#).unwrap();
        let (registry, catalog) = prepare(&root);
        let invocation = call(&catalog, "film.json", r#""max_bytes":512000"#);
        let result = registry.dispatch(invocation, &Allow).await;
        let value = catalog.decode_result(&BTreeSet::from([MODEL_NAME.into()]), MODEL_NAME, &result).unwrap();
        assert_eq!(value["result"]["valid"], true);
        assert_eq!(value["result"]["root_kind"], "object");
        assert_eq!(value["result"]["top_level_keys"], json!(["assets","private","shot"]));
        assert_eq!(value["result"]["content_sha256"].as_str().unwrap().len(), 64);
        assert!(!value.to_string().contains("SECRET_VALUE"));
        drop(catalog);
        drop(registry);
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn duplicate_nested_keys_are_reported_instead_of_overwritten() {
        let root = temp();
        fs::write(root.join("ambiguous.json"), r#"{"outer":{"duplicated":1,"duplicated":2}}"#).unwrap();
        fs::write(root.join("truncated.json"), r#"{"outer":"#).unwrap();
        let (registry, catalog) = prepare(&root);
        for (name, kind) in [("ambiguous.json","DUPLICATE_KEY"),("truncated.json","TRUNCATED_JSON")] {
            let response=registry.dispatch(call(&catalog,name,r#""max_bytes":512000"#),&Allow).await;
            let data=catalog.decode_result(&BTreeSet::from([MODEL_NAME.into()]), MODEL_NAME, &response).unwrap();
            assert_eq!(data["result"]["valid"], false);
            assert_eq!(data["result"]["error_kind"],kind);
        }
        drop(catalog);
        drop(registry);
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn pinned_file_matches_approved_handle_not_replaced_path() {
        let root = temp();
        fs::write(root.join("first.json"), r#"{"source":"first"}"#).unwrap();
        fs::write(root.join("other.json"), r#"{"source":"other"}"#).unwrap();
        let handle = File::open(root.join("first.json")).unwrap();
        let registry = NativeToolRegistry::new();
        let mut catalog = ModelToolCatalog::new();
        register_json_inspection_tool_pinned(
            &registry, &mut catalog, &root, "first.json", handle,
        ).unwrap();
        let wrong = registry.dispatch(
            call(&catalog, "other.json", r#""max_bytes":512000"#), &Allow,
        ).await;
        assert!(wrong.is_err(), "approved handle cannot authorize another filename");
        let correct = registry.dispatch(
            call(&catalog, "first.json", r#""max_bytes":512000"#), &Allow,
        ).await;
        let data = catalog.decode_result(
            &BTreeSet::from([MODEL_NAME.into()]), MODEL_NAME, &correct,
        ).unwrap();
        assert_eq!(data["result"]["valid"], true);
        assert_eq!(data["result"]["bytes_read"], 18);
        drop(catalog);
        drop(registry);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn path_traversal_excessive_read_and_missing_grant_are_refused() {
        let root = temp();
        fs::write(root.join("small.json"), r#"{"ok":true}"#).unwrap();
        fs::write(root.join("large.json"), vec![b'a'; 4096]).unwrap();
        let (registry, catalog) = prepare(&root);
        let allow=BTreeSet::from([MODEL_NAME.to_owned()]);
        for value in [r#"{"path":"../private.json"}"#,
                      r#"{"path":"/etc/passwd"}"#,
                      r#"{"path":"C:\\secret.json"}"#,
                      r#"{"path":"small.json","other":true}"#] {
            assert!(catalog.encode_call(&allow, MODEL_NAME, "x", None, value).is_err());
        }
        let denied=registry.dispatch(call(&catalog,"small.json",r#""max_bytes":512000"#),&Deny).await;
        assert!(denied.is_err());
        let oversized=registry.dispatch(call(&catalog,"large.json",r#""max_bytes":100"#),&Allow).await;
        assert!(oversized.is_err());
        drop(catalog);
        drop(registry);
        fs::remove_dir_all(root).unwrap();
    }
}
