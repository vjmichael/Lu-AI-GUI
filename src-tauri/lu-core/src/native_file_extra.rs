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
const MAX_ENTRIES: usize = 512;
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
        return Err(bad("INVALID_QUERY", "expected a bounded literal UTF-8 query"));
    }
    let suffix = req.file_suffix.as_deref().unwrap_or("");
    if suffix.len() > 64 || suffix.chars().any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))) {
        return Err(bad("INVALID_FILTER", "invalid literal filename suffix"));
    }
    let folder = validate_path(&req.path, true).map_err(|e| bad("INVALID_PATH", e))?;
    let directory = if folder.as_os_str().is_empty() { Path::new(".") } else { folder.as_path() };
    if !folder.as_os_str().is_empty() {
        let meta = root.symlink_metadata(directory).map_err(|_| bad("FILE_ACCESS", "search directory unavailable"))?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(bad("INVALID_PATH", "search directory must be a plain directory"));
        }
    }
    let first = root.open_dir(directory).map_err(|_| bad("FILE_ACCESS", "cannot open search directory"))?;
    let mut queue = std::collections::VecDeque::from([(req.path.clone(), first)]);
    let mut hits = Vec::new();
    let mut scanned_files = 0_u32;
    let mut scanned_directories = 0_u32;
    let mut visited_entries = 0_usize;
    let mut read_bytes = 0_u64;
    let mut truncated = false;
    'walk: while let Some((prefix, dir)) = queue.pop_front() {
        if scanned_directories >= 32 { truncated = true; break; }
        scanned_directories += 1;
        let mut entries = Vec::new();
        for entry in dir.read_dir(".").map_err(|_| bad("FILE_ACCESS", "cannot enumerate directory"))? {
            if visited_entries >= MAX_ENTRIES { truncated = true; break 'walk; }
            visited_entries += 1;
            let entry = entry.map_err(|_| bad("FILE_ACCESS", "cannot read directory entry"))?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else { truncated = true; continue; };
            if name.starts_with('.') || validate_path(&name, false).is_err() { continue; }
            entries.push(name);
        }
        entries.sort();
        for name in entries {
            let relative = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
            if validate_path(&relative, false).is_err() { continue; }
            let Ok(meta) = dir.symlink_metadata(&name) else { truncated = true; continue; };
            if meta.file_type().is_symlink() { continue; }
            if meta.is_dir() {
                if req.recursive.unwrap_or(false) {
                    if queue.len() + scanned_directories as usize >= 32 { truncated = true; }
                    else {
                        match dir.open_dir(&name) {
                            Ok(child) => queue.push_back((relative, child)),
                            Err(_) => truncated = true,
                        }
                    }
                }
                continue;
            }
            if !meta.is_file() || (!suffix.is_empty() && !name.ends_with(suffix)) { continue; }
            if scanned_files >= count || read_bytes >= 4 * 1024 * 1024 {
                truncated = true;
                break 'walk;
            }
            let Ok(mut file) = dir.open(&name) else { truncated = true; continue; };
            if !file.metadata().is_ok_and(|m| m.is_file()) { truncated = true; continue; }
            scanned_files += 1;
            let limit = MAX_FILE_BYTES.min(4 * 1024 * 1024 - read_bytes);
            let mut bytes = Vec::new();
            file.by_ref().take(limit + 1).read_to_end(&mut bytes)
                .map_err(|_| bad("FILE_ACCESS", "cannot read search target"))?;
            if bytes.len() as u64 > limit { bytes.truncate(limit as usize); truncated = true; }
            read_bytes += bytes.len() as u64;
            let Ok(text) = String::from_utf8(bytes) else { continue; };
            for (line, content) in text.lines().enumerate() {
                if content.contains(&req.query) {
                    if hits.len() == max_hits as usize { truncated = true; break 'walk; }
                    hits.push(proto::SearchTextHit { path: relative.clone(), line_number: (line + 1) as u32, excerpt: content.chars().take(240).collect() });
                }
            }
            if hits.len() >= max_hits as usize { truncated = true; break 'walk; }
        }
    }
    if !queue.is_empty() { truncated = true; }
    let reply = proto::SearchTextResult { hits, scanned_files, truncated, scanned_directories };
    Ok(ToolOutput { payload: ProtoPayload { type_url: SEARCH_OUT.into(), bytes: reply.encode_to_vec() } })
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
            &["path", "query", "max_files", "max_matches", "recursive", "file_suffix"],
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
        let recursive = match args.get("recursive") {
            None => None,
            Some(Value::Bool(v)) => Some(*v),
            _ => return Err(ModelToolError::InvalidArguments("recursive must be boolean".into())),
        };
        let file_suffix = match args.get("file_suffix") {
            None => None,
            Some(Value::String(v)) if v.len() <= 64 && v.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) => Some(v.clone()),
            _ => return Err(ModelToolError::InvalidArguments("invalid filename suffix".into())),
        };
        let input = proto::SearchTextRequest {
            path: path.into(),
            query: query.into(),
            max_files: Some(optional_uint(args, "max_files", 32, MAX_FILES)?),
            max_matches: Some(optional_uint(args, "max_matches", 40, MAX_MATCHES)?),
            recursive,
            file_suffix,
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
        Ok(json!({"hits":hits,"scanned_files":result.scanned_files,"scanned_directories":result.scanned_directories,"truncated":result.truncated}))
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
        description: "Search scoped literal UTF-8 text; optional bounded recursion and filename suffix filter. Hidden entries are excluded.".into(),
        parameters: json!({"type":"object","properties":{
            "path":{"type":"string","description":"Workspace directory; omit for root"},
            "query":{"type":"string","description":"Literal phrase, up to 256 UTF-8 bytes"},
            "max_files":{"type":"integer","minimum":1,"maximum":MAX_FILES},
            "max_matches":{"type":"integer","minimum":1,"maximum":MAX_MATCHES},
            "recursive":{"type":"boolean"},
            "file_suffix":{"type":"string","maxLength":64}
        },"required":["query"],"additionalProperties":false}),
        codec: Arc::new(SearchCodec),
    }).map_err(|e| format!("cannot publish search schema: {e:?}"))?;
    Ok(())
}


#[cfg(test)]
mod recursive_search_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> (PathBuf, Dir) {
        let path = std::env::temp_dir().join(format!(
            "lu-recursive-search-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(path.join("nested").join("deeper")).unwrap();
        std::fs::write(path.join("top.rs"), "needle first\n").unwrap();
        std::fs::write(path.join("nested").join("middle.txt"), "needle second\n").unwrap();
        std::fs::write(path.join("nested").join("deeper").join("last.rs"), "needle third\n").unwrap();
        std::fs::write(path.join(".secret"), "needle secret\n").unwrap();
        let dir = Dir::open_ambient_dir(&path, ambient_authority()).unwrap();
        (path, dir)
    }

    fn request(recursive: bool, suffix: Option<&str>) -> proto::SearchTextRequest {
        proto::SearchTextRequest {
            path: String::new(),
            query: "needle".into(),
            max_files: Some(16),
            max_matches: Some(16),
            recursive: Some(recursive),
            file_suffix: suffix.map(str::to_owned),
        }
    }

    #[test]
    fn recursive_search_remains_opt_in_and_scoped() {
        let (path, dir) = fixture();
        let plain = search(&dir, request(false, None)).unwrap();
        let plain = proto::SearchTextResult::decode(plain.payload.bytes.as_slice()).unwrap();
        assert_eq!(plain.scanned_directories, 1);
        assert_eq!(plain.hits.len(), 1);
        assert_eq!(plain.hits[0].path, "top.rs");
        let recursive = search(&dir, request(true, None)).unwrap();
        let recursive = proto::SearchTextResult::decode(recursive.payload.bytes.as_slice()).unwrap();
        assert_eq!(recursive.scanned_directories, 3);
        assert_eq!(recursive.hits.len(), 3);
        assert!(!recursive.truncated);
        let filtered = search(&dir, request(true, Some(".rs"))).unwrap();
        let filtered = proto::SearchTextResult::decode(filtered.payload.bytes.as_slice()).unwrap();
        assert_eq!(filtered.hits.len(), 2);
        assert!(filtered.hits.iter().all(|h| h.path.ends_with(".rs")));
        drop(dir);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn recursion_limits_and_invalid_filters_do_not_silently_succeed() {
        let (path, dir) = fixture();
        let mut req = request(true, None);
        req.max_matches = Some(1);
        let result = search(&dir, req).unwrap();
        let result = proto::SearchTextResult::decode(result.payload.bytes.as_slice()).unwrap();
        assert!(result.truncated);
        let mut req = request(true, Some("../secret"));
        assert!(search(&dir, req.clone()).is_err());
        req.file_suffix = None;
        req.path = "../outside".into();
        assert!(search(&dir, req).is_err());
        drop(dir);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_recursed_or_listed() {
        use std::os::unix::fs::symlink;
        let (path, dir) = fixture();
        symlink("nested", path.join("alias")).unwrap();
        symlink("top.rs", path.join("alias.rs")).unwrap();
        let result = search(&dir, request(true, None)).unwrap();
        let result = proto::SearchTextResult::decode(result.payload.bytes.as_slice()).unwrap();
        assert_eq!(result.hits.len(), 3);
        assert!(result.hits.iter().all(|h| !h.path.starts_with("alias")));
        drop(dir);
        std::fs::remove_dir_all(path).unwrap();
    }
}
