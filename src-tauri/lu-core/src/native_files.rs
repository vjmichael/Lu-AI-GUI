//! First-party native filesystem tools: no MCP, shell, or sidecar required.
//!
//! These are strictly read-only example capabilities. The trusted host chooses
//! a canonical allowed root and must also supply per-invocation authorization.
//! The file handles are opened through cap-std's capability-based directory
//! interface, which keeps model-chosen paths beneath the user-approved root.
//! A full multi-platform adversarial filesystem test is still required before
//! shipping these tools in the general desktop.

use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use cap_std::{ambient_authority, fs::Dir};

use prost::Message;
use serde_json::{json, Map, Value};

use crate::{
    contracts::{
        ModuleKind, ModuleManifest, NativeTool, ProtoPayload, ToolDescriptor, ToolEffect,
        ToolFailure, ToolFuture, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION,
    },
    model::{
        expect_fields, optional_uint, ModelToolCatalog, ModelToolCodec, ModelToolDefinition,
        ModelToolError,
    },
    registry::{NativeToolRegistry, RegistryError},
    wire::proto,
};

const MODULE_ID: &str = "lu.files";
const READ_ID: &str = "lu.files.read_text";
const LIST_ID: &str = "lu.files.list_directory";
const READ_INPUT: &str = "type.googleapis.com/lu.v1.ReadTextFileRequest";
const READ_OUTPUT: &str = "type.googleapis.com/lu.v1.ReadTextFileResult";
const LIST_INPUT: &str = "type.googleapis.com/lu.v1.ListDirectoryRequest";
const LIST_OUTPUT: &str = "type.googleapis.com/lu.v1.ListDirectoryResult";

const DEFAULT_READ_BYTES: u32 = 256 * 1024;
const MAX_READ_BYTES: u32 = 1024 * 1024;
const DEFAULT_LIST_LIMIT: u32 = 100;
const MAX_LIST_LIMIT: u32 = 1000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileBundleError {
    InvalidRoot(String),
    Registry(RegistryError),
    Model(ModelToolError),
}

fn failure(code: &str, message: impl Into<String>) -> ToolFailure {
    ToolFailure::new(code, message, false)
}

fn descriptor(id: &str, name: &str, input: &str, output: &str) -> ToolDescriptor {
    ToolDescriptor {
        id: id.into(),
        display_name: name.into(),
        effect: ToolEffect::ReadWorkspace,
        input_type_url: input.into(),
        output_type_url: output.into(),
    }
}

struct FileRead {
    descriptor: ToolDescriptor,
    root: Arc<Dir>,
}

struct DirectoryList {
    descriptor: ToolDescriptor,
    root: Arc<Dir>,
}

/// Intentionally small lexical input surface, identical on Unix and Windows.
/// An empty path is allowed only for listing the selected workspace root.
fn relative_path(raw: &str, allow_empty: bool) -> Result<PathBuf, ToolFailure> {
    if raw.is_empty() && allow_empty {
        return Ok(PathBuf::new());
    }
    if raw.is_empty()
        || raw.len() > 4096
        || raw.chars().any(|c| matches!(c, '\\' | ':' | '\0'))
        || raw
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(failure(
            "INVALID_PATH",
            "expected a simple, workspace-relative path",
        ));
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(failure("INVALID_PATH", "absolute paths are not allowed"));
    }
    Ok(path.to_path_buf())
}

fn perform_read(
    root: &Dir,
    request: proto::ReadTextFileRequest,
) -> Result<ToolOutput, ToolFailure> {
    let cap = request.max_bytes.unwrap_or(DEFAULT_READ_BYTES);
    if cap == 0 || cap > MAX_READ_BYTES {
        return Err(failure(
            "INVALID_LIMIT",
            "read limit outside permitted range",
        ));
    }
    let path = relative_path(&request.path, false)?;
    let metadata = root
        .symlink_metadata(&path)
        .map_err(|e| failure("FILE_ACCESS", format!("cannot inspect file: {e}")))?;
    if metadata.file_type().is_symlink() {
        return Err(failure(
            "SYMLINK_REFUSED",
            "symlink files are not supported",
        ));
    }
    let mut file = root
        .open(&path)
        .map_err(|e| failure("FILE_ACCESS", format!("cannot open file: {e}")))?;
    if !file
        .metadata()
        .map_err(|e| failure("FILE_ACCESS", e.to_string()))?
        .is_file()
    {
        return Err(failure("NOT_FILE", "path must identify a regular file"));
    }
    let mut data = Vec::new();
    file.by_ref()
        .take(u64::from(cap) + 1)
        .read_to_end(&mut data)
        .map_err(|e| failure("FILE_ACCESS", format!("cannot read file: {e}")))?;
    let truncated = data.len() > cap as usize;
    if truncated {
        data.truncate(cap as usize);
    }
    let bytes_read = data.len() as u64;
    let content = String::from_utf8(data)
        .map_err(|_| failure("NOT_UTF8", "file content is not UTF-8 text at this limit"))?;
    let output = proto::ReadTextFileResult {
        content,
        bytes_read,
        truncated,
    };
    Ok(ToolOutput {
        payload: ProtoPayload {
            type_url: READ_OUTPUT.into(),
            bytes: output.encode_to_vec(),
        },
    })
}

fn perform_list(
    root: &Dir,
    request: proto::ListDirectoryRequest,
) -> Result<ToolOutput, ToolFailure> {
    let limit = request.limit.unwrap_or(DEFAULT_LIST_LIMIT);
    if limit == 0 || limit > MAX_LIST_LIMIT {
        return Err(failure(
            "INVALID_LIMIT",
            "listing limit outside permitted range",
        ));
    }
    let path = relative_path(&request.path, true)?;
    let path = if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        &path
    };
    let mut entries = Vec::new();
    for item in root
        .read_dir(path)
        .map_err(|e| failure("FILE_ACCESS", format!("cannot list directory: {e}")))?
    {
        let item = item.map_err(|e| failure("FILE_ACCESS", e.to_string()))?;
        let metadata = item
            .file_type()
            .map_err(|e| failure("FILE_ACCESS", e.to_string()))?;
        let kind = if metadata.is_symlink() {
            "symlink"
        } else if metadata.is_dir() {
            "directory"
        } else if metadata.is_file() {
            "file"
        } else {
            "other"
        };
        let size_bytes = if metadata.is_file() {
            item.metadata()
                .map_err(|e| failure("FILE_ACCESS", e.to_string()))?
                .len()
        } else {
            0
        };
        entries.push(proto::FileEntry {
            name: item.file_name().to_string_lossy().to_string(),
            kind: kind.to_string(),
            size_bytes,
        });
        if entries.len() > limit as usize {
            break;
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let truncated = entries.len() > limit as usize;
    entries.truncate(limit as usize);
    let output = proto::ListDirectoryResult { entries, truncated };
    Ok(ToolOutput {
        payload: ProtoPayload {
            type_url: LIST_OUTPUT.into(),
            bytes: output.encode_to_vec(),
        },
    })
}

impl NativeTool for FileRead {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }

    fn invoke(&self, invocation: ToolInvocation) -> ToolFuture<'_> {
        let root = Arc::clone(&self.root);
        Box::pin(async move {
            let input = proto::ReadTextFileRequest::decode(invocation.input.bytes.as_slice())
                .map_err(|e| failure("BAD_PROTOBUF", format!("invalid read request: {e}")))?;
            tokio::task::spawn_blocking(move || perform_read(&root, input))
                .await
                .map_err(|e| failure("WORKER_FAILED", e.to_string()))?
        })
    }
}

impl NativeTool for DirectoryList {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }

    fn invoke(&self, invocation: ToolInvocation) -> ToolFuture<'_> {
        let root = Arc::clone(&self.root);
        Box::pin(async move {
            let input = proto::ListDirectoryRequest::decode(invocation.input.bytes.as_slice())
                .map_err(|e| failure("BAD_PROTOBUF", format!("invalid list request: {e}")))?;
            tokio::task::spawn_blocking(move || perform_list(&root, input))
                .await
                .map_err(|e| failure("WORKER_FAILED", e.to_string()))?
        })
    }
}

struct ReadCodec;
impl ModelToolCodec for ReadCodec {
    fn input_type_url(&self) -> &'static str {
        READ_INPUT
    }
    fn output_type_url(&self) -> &'static str {
        READ_OUTPUT
    }

    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        let result = proto::ReadTextFileResult::decode(output.bytes.as_slice())
            .map_err(|e| ModelToolError::InvalidResult(format!("bad read result: {e}")))?;
        Ok(json!({
            "content": result.content,
            "bytes_read": result.bytes_read,
            "truncated": result.truncated
        }))
    }

    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["path", "max_bytes"], &["path"])?;
        let path = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| ModelToolError::InvalidArguments("path must be a string".into()))?;
        relative_path(path, false).map_err(|e| ModelToolError::InvalidArguments(e.message))?;
        let max_bytes = optional_uint(args, "max_bytes", DEFAULT_READ_BYTES, MAX_READ_BYTES)?;
        let msg = proto::ReadTextFileRequest {
            path: path.into(),
            max_bytes: Some(max_bytes),
        };
        Ok(ProtoPayload {
            type_url: READ_INPUT.into(),
            bytes: msg.encode_to_vec(),
        })
    }
}

struct ListCodec;
impl ModelToolCodec for ListCodec {
    fn input_type_url(&self) -> &'static str {
        LIST_INPUT
    }
    fn output_type_url(&self) -> &'static str {
        LIST_OUTPUT
    }

    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        let result = proto::ListDirectoryResult::decode(output.bytes.as_slice())
            .map_err(|e| ModelToolError::InvalidResult(format!("bad list result: {e}")))?;
        let entries: Vec<_> = result
            .entries
            .into_iter()
            .map(|entry| {
                json!({
                    "name": entry.name,
                    "kind": entry.kind,
                    "size_bytes": entry.size_bytes
                })
            })
            .collect();
        Ok(json!({"entries": entries, "truncated": result.truncated}))
    }

    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["path", "limit"], &[])?;
        let path = match args.get("path") {
            Some(v) => v
                .as_str()
                .ok_or_else(|| ModelToolError::InvalidArguments("path must be a string".into()))?,
            None => "",
        };
        relative_path(path, true).map_err(|e| ModelToolError::InvalidArguments(e.message))?;
        let limit = optional_uint(args, "limit", DEFAULT_LIST_LIMIT, MAX_LIST_LIMIT)?;
        let msg = proto::ListDirectoryRequest {
            path: path.into(),
            limit: Some(limit),
        };
        Ok(ProtoPayload {
            type_url: LIST_INPUT.into(),
            bytes: msg.encode_to_vec(),
        })
    }
}

/// Trusted-host installation of the first two Lu-owned default tools.
/// Returns the model-visible schema/codecs; none is advertised until the host
/// selects a per-conversation allow-list and passes an authorizer to dispatch.
pub fn register_bundled_file_tools(
    registry: &NativeToolRegistry,
    authorized_root: &Path,
) -> Result<ModelToolCatalog, FileBundleError> {
    let root = authorized_root
        .canonicalize()
        .map_err(|e| FileBundleError::InvalidRoot(e.to_string()))?;
    if !root.is_dir() {
        return Err(FileBundleError::InvalidRoot(
            "root must be a directory".into(),
        ));
    }
    // This is the only ambient-authority operation: opening the trusted
    // workspace root supplied by the host. All model-controlled paths below
    // use capability-relative methods on this handle.
    let root = Dir::open_ambient_dir(&root, ambient_authority())
        .map_err(|e| FileBundleError::InvalidRoot(e.to_string()))?;
    let root = Arc::new(root);
    let read = descriptor(READ_ID, "Read UTF-8 file", READ_INPUT, READ_OUTPUT);
    let list = descriptor(LIST_ID, "List directory entries", LIST_INPUT, LIST_OUTPUT);
    registry
        .register(
            ModuleManifest {
                id: MODULE_ID.into(),
                display_name: "Lu bundled filesystem tools".into(),
                contract_version: CORE_CONTRACT_VERSION,
                kind: ModuleKind::Core,
                tools: vec![read.clone(), list.clone()],
                panels: vec![],
                workflows: vec![],
            },
            vec![
                Arc::new(FileRead {
                    descriptor: read,
                    root: Arc::clone(&root),
                }),
                Arc::new(DirectoryList {
                    descriptor: list,
                    root,
                }),
            ],
        )
        .map_err(FileBundleError::Registry)?;

    let mut catalog = ModelToolCatalog::new();
    catalog
        .insert(
            registry,
            ModelToolDefinition {
                name: "lu_files_read_text".into(),
                native_tool_id: READ_ID.into(),
                description: "Read a UTF-8 text file within the authorized workspace.".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Workspace-relative file path" },
                        "max_bytes": { "type": "integer", "minimum": 1, "maximum": MAX_READ_BYTES }
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }),
                codec: Arc::new(ReadCodec),
            },
        )
        .map_err(FileBundleError::Model)?;
    catalog.insert(registry, ModelToolDefinition {
        name: "lu_files_list_directory".into(),
        native_tool_id: LIST_ID.into(),
        description: "List files and directories within the authorized workspace.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Workspace-relative directory path; omit for workspace root" },
                "limit": { "type": "integer", "minimum": 1, "maximum": MAX_LIST_LIMIT }
            },
            "required": [],
            "additionalProperties": false
        }),
        codec: Arc::new(ListCodec),
    }).map_err(FileBundleError::Model)?;
    Ok(catalog)
}
