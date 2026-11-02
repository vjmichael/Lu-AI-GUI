//! Additional Lu-owned read-only native tools. No Jan Cowork or MCP transport.
//! Model strings are validated as workspace-relative paths and executed through
//! cap-std's host-established directory handles. This module is deliberately
//! separate so the catalog can grow without an agent-loop rewrite.

use crate::{
    contracts::{
        ModuleKind, ModuleManifest, NativeTool, ProtoPayload, ToolDescriptor, ToolEffect,
        ToolFailure, ToolFuture, ToolInvocation, ToolOutput, CORE_CONTRACT_VERSION,
    },
    model::{
        expect_fields, optional_uint, ModelToolCatalog, ModelToolCodec, ModelToolDefinition,
        ModelToolError,
    },
    registry::NativeToolRegistry,
    wire::proto,
};
use cap_std::{ambient_authority, fs::Dir};
use prost::Message;
use serde_json::{json, Map, Value};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

const MODULE: &str = "lu.inspect";
const STAT: &str = "lu.inspect.stat";
const SEARCH: &str = "lu.inspect.search_text";
const STAT_IN: &str = "type.googleapis.com/lu.v1.FileStatRequest";
const STAT_OUT: &str = "type.googleapis.com/lu.v1.FileStatResult";
const SEARCH_IN: &str = "type.googleapis.com/lu.v1.SearchTextRequest";
const SEARCH_OUT: &str = "type.googleapis.com/lu.v1.SearchTextResult";
const MAX_FILES: u32 = 64;
const MAX_MATCHES: u32 = 128;
const MAX_ENTRIES: usize = 256;
const MAX_FILE_BYTES: u64 = 64 * 1024;

fn bad(code: &str, message: impl Into<String>) -> ToolFailure {
    ToolFailure::new(code, message, false)
}

fn validate_path(value: &str, empty_ok: bool) -> Result<PathBuf, String> {
    if value.is_empty() && empty_ok {
        return Ok(PathBuf::new());
    }
    if value.is_empty()
        || value.len() > 4096
        || value.chars().any(|c| matches!(c, '\\' | ':' | '\0'))
        || value
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..")
        || Path::new(value).is_absolute()
    {
        return Err("expected a simple, workspace-relative path".into());
    }
    Ok(PathBuf::from(value))
}

fn stat(root: &Dir, path: String) -> Result<ToolOutput, ToolFailure> {
    let relative = validate_path(&path, false).map_err(|e| bad("INVALID_PATH", e))?;
    let meta = root
        .symlink_metadata(&relative)
        .map_err(|_| bad("FILE_ACCESS", "cannot inspect path"))?;
    let kind = if meta.file_type().is_symlink() {
        "symlink"
    } else if meta.is_file() {
        "file"
    } else if meta.is_dir() {
        "directory"
    } else {
        "other"
    };
    let reply = proto::FileStatResult {
        kind: kind.into(),
        size_bytes: meta.len(),
        read_only: meta.permissions().readonly(),
    };
    Ok(ToolOutput {
        payload: ProtoPayload {
            type_url: STAT_OUT.into(),
            bytes: reply.encode_to_vec(),
        },
    })
}

fn search(root: &Dir, req: proto::SearchTextRequest) -> Result<ToolOutput, ToolFailure> {
    let count = req.max_files.unwrap_or(32);
    let max_hits = req.max_matches.unwrap_or(40);
    if !(1..=MAX_FILES).contains(&count) || !(1..=MAX_MATCHES).contains(&max_hits) {
        return Err(bad("INVALID_LIMIT", "invalid search size limit"));
    }
    if req.query.is_empty() || req.query.len() > 256 || req.query.chars().any(char::is_control) {
        return Err(bad(
            "INVALID_QUERY",
            "expected a bounded, literal UTF-8 phrase",
        ));
    }
    let folder = validate_path(&req.path, true).map_err(|e| bad("INVALID_PATH", e))?;
    let directory = if folder.as_os_str().is_empty() {
        Path::new(".")
    } else {
        folder.as_path()
    };
    let mut candidates = Vec::new();
    let mut truncated = false;
    for (visited_entries, entry) in root
        .read_dir(directory)
        .map_err(|_| bad("FILE_ACCESS", "cannot open search directory"))?
        .enumerate()
    {
        if visited_entries >= MAX_ENTRIES {
            truncated = true;
            break;
        }
        let entry = entry.map_err(|_| bad("FILE_ACCESS", "cannot read directory entry"))?;
        if !entry
            .file_type()
            .map_err(|_| bad("FILE_ACCESS", "cannot inspect entry"))?
            .is_file()
        {
            continue; // Never traverse directories or symlinks.
        }
        let filename = entry.file_name();
        let Some(name) = filename.to_str() else {
            continue;
        };
        let relative = if req.path.is_empty() {
            name.to_owned()
        } else {
            format!("{}/{}", req.path, name)
        };
        if validate_path(&relative, false).is_ok() {
            candidates.push(relative);
        }
    }
    candidates.sort();
    let mut hits = Vec::new();
    let mut scanned_files = 0_u32;
    for name in candidates {
        if scanned_files >= count {
            truncated = true;
            break;
        }
        let path = validate_path(&name, false).map_err(|e| bad("INVALID_PATH", e))?;
        let Ok(meta) = root.symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_file() || meta.file_type().is_symlink() {
            continue;
        }
        let Ok(mut file) = root.open(&path) else {
            continue;
        };
        if !file.metadata().is_ok_and(|m| m.is_file()) {
            continue;
        }
        scanned_files += 1;
        let mut bytes = Vec::new();
        file.by_ref()
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| bad("FILE_ACCESS", "cannot read search target"))?;
        if bytes.len() > MAX_FILE_BYTES as usize {
            bytes.truncate(MAX_FILE_BYTES as usize);
            truncated = true;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        for (line, content) in text.lines().enumerate() {
            if content.contains(&req.query) {
                if hits.len() == max_hits as usize {
                    truncated = true;
                    break;
                }
                hits.push(proto::SearchTextHit {
                    path: name.clone(),
                    line_number: (line + 1) as u32,
                    excerpt: content.chars().take(240).collect(),
                });
            }
        }
        if hits.len() >= max_hits as usize {
            truncated = true;
            break;
        }
    }
    let reply = proto::SearchTextResult {
        hits,
        scanned_files,
        truncated,
    };
    Ok(ToolOutput {
        payload: ProtoPayload {
            type_url: SEARCH_OUT.into(),
            bytes: reply.encode_to_vec(),
        },
    })
}

struct InspectTool {
    descriptor: ToolDescriptor,
    root: Arc<Dir>,
}
impl NativeTool for InspectTool {
    fn descriptor(&self) -> &ToolDescriptor {
        &self.descriptor
    }
    fn invoke(&self, call: ToolInvocation) -> ToolFuture<'_> {
        let root = Arc::clone(&self.root);
        let is_stat = self.descriptor.id == STAT;
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                if is_stat {
                    let args = proto::FileStatRequest::decode(call.input.bytes.as_slice())
                        .map_err(|_| bad("BAD_PROTOBUF", "invalid stat arguments"))?;
                    stat(&root, args.path)
                } else {
                    let args = proto::SearchTextRequest::decode(call.input.bytes.as_slice())
                        .map_err(|_| bad("BAD_PROTOBUF", "invalid search arguments"))?;
                    search(&root, args)
                }
            })
            .await
            .map_err(|_| bad("WORKER_FAILED", "inspection worker stopped"))?
        })
    }
}

struct StatCodec;
impl ModelToolCodec for StatCodec {
    fn input_type_url(&self) -> &'static str {
        STAT_IN
    }
    fn output_type_url(&self) -> &'static str {
        STAT_OUT
    }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["path"], &["path"])?;
        let path = args
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| ModelToolError::InvalidArguments("path must be a string".into()))?;
        validate_path(path, false).map_err(ModelToolError::InvalidArguments)?;
        let reply = proto::FileStatRequest { path: path.into() };
        Ok(ProtoPayload {
            type_url: STAT_IN.into(),
            bytes: reply.encode_to_vec(),
        })
    }
    fn decode(&self, value: &ProtoPayload) -> Result<Value, ModelToolError> {
        let result = proto::FileStatResult::decode(value.bytes.as_slice())
            .map_err(|_| ModelToolError::InvalidResult("bad stat reply".into()))?;
        Ok(json!({"kind":result.kind,"size_bytes":result.size_bytes,"read_only":result.read_only}))
    }
}

struct SearchCodec;
impl ModelToolCodec for SearchCodec {
    fn input_type_url(&self) -> &'static str {
        SEARCH_IN
    }
    fn output_type_url(&self) -> &'static str {
        SEARCH_OUT
    }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(
            args,
            &["path", "query", "max_files", "max_matches"],
            &["query"],
        )?;
        let path = match args.get("path") {
            None => "",
            Some(Value::String(s)) => s.as_str(),
            _ => {
                return Err(ModelToolError::InvalidArguments(
                    "path must be a string".into(),
                ))
            }
        };
        validate_path(path, true).map_err(ModelToolError::InvalidArguments)?;
        let query = args
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| ModelToolError::InvalidArguments("query must be a string".into()))?;
        if query.is_empty() || query.len() > 256 || query.chars().any(char::is_control) {
            return Err(ModelToolError::InvalidArguments(
                "invalid search query".into(),
            ));
        }
        let input = proto::SearchTextRequest {
            path: path.into(),
            query: query.into(),
            max_files: Some(optional_uint(args, "max_files", 32, MAX_FILES)?),
            max_matches: Some(optional_uint(args, "max_matches", 40, MAX_MATCHES)?),
        };
        Ok(ProtoPayload {
            type_url: SEARCH_IN.into(),
            bytes: input.encode_to_vec(),
        })
    }
    fn decode(&self, value: &ProtoPayload) -> Result<Value, ModelToolError> {
        let result = proto::SearchTextResult::decode(value.bytes.as_slice())
            .map_err(|_| ModelToolError::InvalidResult("bad search reply".into()))?;
        let hits: Vec<_> = result
            .hits
            .into_iter()
            .map(|hit| {
                json!({
                    "path":hit.path,"line_number":hit.line_number,"excerpt":hit.excerpt,
                })
            })
            .collect();
        Ok(json!({"hits":hits,"scanned_files":result.scanned_files,"truncated":result.truncated}))
    }
}

/// Extend a host-selected catalog with more real native implementations.
/// A model does not enroll tools: the Rust host owns this registration.
pub fn register_inspection_tools(
    registry: &NativeToolRegistry,
    catalog: &mut ModelToolCatalog,
    authorized_root: &Path,
) -> Result<(), String> {
    let canonical = authorized_root
        .canonicalize()
        .map_err(|_| "workspace root unavailable")?;
    if !canonical.is_dir() {
        return Err("workspace root must be a directory".into());
    }
    let root = Arc::new(
        Dir::open_ambient_dir(&canonical, ambient_authority())
            .map_err(|_| "cannot open authorized workspace")?,
    );
    let descriptor = |id: &str, title: &str, input: &str, output: &str| ToolDescriptor {
        id: id.into(),
        display_name: title.into(),
        effect: ToolEffect::ReadWorkspace,
        input_type_url: input.into(),
        output_type_url: output.into(),
    };
    let stat = descriptor(STAT, "Inspect file metadata", STAT_IN, STAT_OUT);
    let search = descriptor(SEARCH, "Search workspace text", SEARCH_IN, SEARCH_OUT);
    registry
        .register(
            ModuleManifest {
                id: MODULE.into(),
                display_name: "Lu native file inspection".into(),
                contract_version: CORE_CONTRACT_VERSION,
                kind: ModuleKind::Core,
                tools: vec![stat.clone(), search.clone()],
                panels: vec![],
                workflows: vec![],
            },
            vec![
                Arc::new(InspectTool {
                    descriptor: stat,
                    root: Arc::clone(&root),
                }),
                Arc::new(InspectTool {
                    descriptor: search,
                    root,
                }),
            ],
        )
        .map_err(|e| format!("cannot register inspection tools: {e:?}"))?;

    catalog
        .insert(
            registry,
            ModelToolDefinition {
                name: "lu_files_stat".into(),
                native_tool_id: STAT.into(),
                description: "Inspect a file, directory or symlink in the scoped workspace.".into(),
                parameters: json!({"type":"object",
            "properties":{"path":{"type":"string"}},
            "required":["path"],"additionalProperties":false}),
                codec: Arc::new(StatCodec),
            },
        )
        .map_err(|e| format!("cannot publish stat schema: {e:?}"))?;
    catalog.insert(registry, ModelToolDefinition {
        name: "lu_files_search_text".into(), native_tool_id: SEARCH.into(),
        description: "Search literal UTF-8 text within one directory (nonrecursive, maximum 64 files and 128 matches).".into(),
        parameters: json!({"type":"object","properties":{
            "path":{"type":"string","description":"Workspace directory; omit for root"},
            "query":{"type":"string","description":"Literal phrase, up to 256 UTF-8 bytes"},
            "max_files":{"type":"integer","minimum":1,"maximum":MAX_FILES},
            "max_matches":{"type":"integer","minimum":1,"maximum":MAX_MATCHES}
        },"required":["query"],"additionalProperties":false}),
        codec: Arc::new(SearchCodec),
    }).map_err(|e| format!("cannot publish search schema: {e:?}"))?;
    Ok(())
}
