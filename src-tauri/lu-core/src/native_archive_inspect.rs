//! Lu Core TOOLS-04: read-only ZIP manifest inspection.
//!
//! Original Jan tool design §5.2: one unified native domain and narrow
//! model-visible schema. This is a host-authorized read capability, not a
//! user-facing archive editor, shell command or extraction path.
//!
//! No member data is decompressed or extracted. Declared sizes and CRCs are
//! explicitly untrusted metadata; validation does not certify archive safety
//! for a later, separate extraction operation.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use cap_std::{ambient_authority, fs::Dir};
use prost::Message;
use serde_json::{json, Map, Value};
use zip::ZipArchive;

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

const MODULE: &str = "lu.archive";
const LIST: &str = "lu.archive.list";
const INPUT: &str = "type.googleapis.com/lu.v1.ArchiveListRequest";
const OUTPUT: &str = "type.googleapis.com/lu.v1.ArchiveListResult";
const DEFAULT_ENTRIES: u32 = 80;
const MAX_RETURNED_ENTRIES: u32 = 256;
const MAX_ARCHIVE_ENTRIES: usize = 4096;
const MAX_ARCHIVE_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_MEMBER_DECLARED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DECLARED_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const MAX_MEMBER_NAME_BYTES: usize = 1024;
const MAX_EXPANSION_RATIO: u64 = 1000;

fn refuse(code: &str, message: &str) -> ToolFailure {
    ToolFailure::new(code, message, false)
}

fn safe_relative_name(value: &str, archive_member: bool) -> Result<PathBuf, ToolFailure> {
    let max_len = if archive_member { MAX_MEMBER_NAME_BYTES } else { 4096 };
    if value.is_empty() || value.len() > max_len
        || value.chars().any(|c| c.is_control() || matches!(c, '\\' | ':')) {
        return Err(refuse("INVALID_ARCHIVE_PATH", "archive path contains unsafe characters"));
    }
    let value = if archive_member {
        value.strip_suffix('/').unwrap_or(value)
    } else {
        value
    };
    if value.starts_with('/') || Path::new(value).is_absolute()
        || value.split('/').any(|part| part.is_empty()
            || part == "." || part == ".." || part == ".git"
            || part.starts_with('-') || part.ends_with('.')
            || part.ends_with(' '))
    {
        return Err(refuse("INVALID_ARCHIVE_PATH", "archive path escapes or aliases an entry"));
    }
    Ok(PathBuf::from(value))
}

fn inspect_zip(root: &Dir, request: proto::ArchiveListRequest)
    -> Result<ToolOutput, ToolFailure>
{
    let max_entries = request.max_entries.unwrap_or(DEFAULT_ENTRIES);
    if !(1..=MAX_RETURNED_ENTRIES).contains(&max_entries) {
        return Err(refuse("ARCHIVE_LIMIT", "invalid archive listing entry budget"));
    }
    let path = safe_relative_name(&request.path, false)?;
    if !request.path.to_ascii_lowercase().ends_with(".zip") {
        return Err(refuse("ARCHIVE_FORMAT_UNSUPPORTED", "only ZIP manifests are supported"));
    }
    let meta = root.symlink_metadata(&path)
        .map_err(|_| refuse("ARCHIVE_UNAVAILABLE", "approved archive file not found"))?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(refuse("ARCHIVE_NOT_FILE", "archive must be a regular file, not a link"));
    }
    if meta.len() > MAX_ARCHIVE_FILE_BYTES {
        return Err(refuse("ARCHIVE_TOO_LARGE", "archive exceeds bounded input size"));
    }
    let file = root.open(&path)
        .map_err(|_| refuse("ARCHIVE_UNAVAILABLE", "cannot open approved archive"))?;
    let handle_meta = file.metadata()
        .map_err(|_| refuse("ARCHIVE_UNAVAILABLE", "cannot verify archive handle"))?;
    if !handle_meta.is_file() || handle_meta.len() > MAX_ARCHIVE_FILE_BYTES {
        return Err(refuse("ARCHIVE_TOO_LARGE", "approved archive handle is not bounded"));
    }
    // ZIP central directory is parsed by the maintained zip-rs library.
    // by_index_raw reads metadata without using an extractor or allocating a
    // decompressed member buffer.
    let mut archive = ZipArchive::new(file)
        .map_err(|_| refuse("ARCHIVE_INVALID", "ZIP central directory could not be parsed"))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(refuse("ARCHIVE_TOO_MANY_ENTRIES", "archive member count exceeds hard cap"));
    }
    if archive.has_overlapping_entries()
        .map_err(|_| refuse("ARCHIVE_INVALID", "ZIP layout is invalid"))?
    {
        return Err(refuse("ARCHIVE_UNSAFE_ENTRY", "archive entries overlap"));
    }
    archive.validate_header_consistency()
        .map_err(|_| refuse("ARCHIVE_INVALID", "ZIP local and central directory metadata differ"))?;
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    let mut entries = Vec::new();
    let mut truncated = false;
    for i in 0..archive.len() {
        let member = archive.by_index_raw(i)
            .map_err(|_| refuse("ARCHIVE_INVALID", "ZIP member metadata is invalid"))?;
        let name = member.name()
            .map_err(|_| refuse("ARCHIVE_INVALID", "ZIP member name is not decodable"))?
            .to_owned();
        safe_relative_name(&name, true)?;
        // Fold path case to prevent Windows aliases; normalize trailing slash
        // so "a" and "a/" cannot target the same object in later pipelines.
        let key = name.trim_end_matches('/').to_lowercase();
        if !seen.insert(key) {
            return Err(refuse("ARCHIVE_DUPLICATE_ENTRY", "archive repeats an entry name"));
        }
        let mode = member.unix_mode().unwrap_or(0);
        let file_kind = mode & 0o170000;
        if member.is_symlink() || file_kind == 0o120000
            || (file_kind != 0 && file_kind != 0o100000 && file_kind != 0o040000)
        {
            return Err(refuse("ARCHIVE_UNSAFE_ENTRY", "archive contains a link or special file"));
        }
        let directory = member.is_dir();
        if (directory && file_kind == 0o100000)
            || (!directory && file_kind == 0o040000)
        {
            return Err(refuse("ARCHIVE_UNSAFE_ENTRY", "archive member type conflicts"));
        }
        let compressed = member.compressed_size();
        let expanded = member.size();
        if expanded > MAX_MEMBER_DECLARED_BYTES
            || (expanded > 0 && compressed == 0)
            || (compressed > 0 && expanded / compressed > MAX_EXPANSION_RATIO)
        {
            return Err(refuse("ARCHIVE_BOMB_RISK", "archive member declares unsafe expansion"));
        }
        total = total.checked_add(expanded)
            .ok_or_else(|| refuse("ARCHIVE_BOMB_RISK", "archive declared size overflow"))?;
        if total > MAX_DECLARED_TOTAL_BYTES {
            return Err(refuse("ARCHIVE_BOMB_RISK", "archive declared total exceeds limit"));
        }
        if entries.len() >= max_entries as usize {
            truncated = true;
            continue;
        }
        entries.push(proto::ArchiveEntry {
            path: name.into_owned(),
            kind: if directory { "directory" } else { "file" }.into(),
            compressed_bytes: compressed,
            declared_uncompressed_bytes: expanded,
            crc32: member.crc32(),
        });
    }
    let result = proto::ArchiveListResult {
        entries,
        truncated,
        total_entries: archive.len() as u64,
        total_declared_uncompressed_bytes: total,
        archive_bytes: handle_meta.len(),
    };
    Ok(ToolOutput {
        payload: ProtoPayload { type_url: OUTPUT.into(), bytes: result.encode_to_vec() },
    })
}


fn inspect(root:&Dir,request:proto::ArchiveListRequest)
    ->Result<ToolOutput,ToolFailure>
{
    if request.path.to_ascii_lowercase().ends_with(".zip"){
        return inspect_zip(root,request)
    }
    if crate::native_cpio::is_cpio_name(&request.path)||
        crate::native_ar_deb::is_ar_name(&request.path){
        let max=request.max_entries.unwrap_or(DEFAULT_ENTRIES);
        if !(1..=MAX_RETURNED_ENTRIES).contains(&max){
            return Err(refuse("ARCHIVE_LIMIT","Invalid package listing budget"))
        }
        let path=safe_relative_name(&request.path,false)?;
        if path.components().count()!=1{
            return Err(refuse("INVALID_ARCHIVE_PATH","Package must be in approved directory"))
        }
        let stat=root.symlink_metadata(&path)
            .map_err(|_|refuse("ARCHIVE_UNAVAILABLE","Package file unavailable"))?;
        if !stat.is_file()||stat.file_type().is_symlink()
            ||stat.len()>128*1024*1024{
            return Err(refuse("ARCHIVE_TOO_LARGE","Package must be a bounded regular file"))
        }
        let members=if crate::native_cpio::is_cpio_name(&request.path){
            crate::native_cpio::cpio_tree_members(root,&request.path).map_err(|_|
                refuse("ARCHIVE_INVALID","CPIO package failed safety validation"))?
        }else{
            crate::native_ar_deb::ar_tree_members(root,&request.path).map_err(|_|
                refuse("ARCHIVE_INVALID","Unix ar/Debian package rejected"))?
        };
        if members.len()>MAX_ARCHIVE_ENTRIES{
            return Err(refuse("ARCHIVE_TOO_LARGE","Too many package entries"))
        }
        let total=members.iter().try_fold(0u64,|sum,(_,_,b)|
            sum.checked_add(b.len() as u64))
            .ok_or_else(||refuse("ARCHIVE_TOO_LARGE","Package content over limit"))?;
        let total_entries=members.len();
        let entries=members.into_iter().take(max as usize).map(|(path,directory,bytes)|
            proto::ArchiveEntry{
                path,kind:if directory{"directory".into()}else{"file".into()},
                compressed_bytes:0,
                declared_uncompressed_bytes:bytes.len() as u64,
                crc32:0,
            }).collect();
        return Ok(ToolOutput{payload:ProtoPayload{
            type_url:OUTPUT.into(),
            bytes:proto::ArchiveListResult{
                entries,truncated:total_entries>max as usize,
                total_entries:total_entries as u64,total_declared_uncompressed_bytes:total,
                archive_bytes:stat.len(),
            }.encode_to_vec(),
        }})
    }
    if crate::native_linux_archive::detect_linux_archive(&request.path).is_some(){
        let max=request.max_entries.unwrap_or(DEFAULT_ENTRIES);
        if !(1..=MAX_RETURNED_ENTRIES).contains(&max){
            return Err(refuse("ARCHIVE_LIMIT","Invalid archive listing limit"))
        }
        let path=safe_relative_name(&request.path,false)?;
        if path.components().count()!=1 {
            return Err(refuse("INVALID_ARCHIVE_PATH","Linux archive must be in approved directory"))
        }
        let manifest=crate::native_linux_archive::inspect_linux_archive(
            root,&request.path,max as usize
        ).map_err(|_|refuse("ARCHIVE_INVALID","Linux archive inspection failed validation"))?;
        return Ok(ToolOutput{payload:ProtoPayload{
            type_url:OUTPUT.into(),
            bytes:proto::ArchiveListResult {
                entries:manifest.entries.into_iter().map(|e|proto::ArchiveEntry {
                    path:e.path,
                    kind:if e.is_directory{"directory".into()}else{"file".into()},
                    compressed_bytes:0, // TAR has no per-file compressed size.
                    declared_uncompressed_bytes:e.uncompressed_bytes,
                    crc32:0, // No per-file CRC32 in these formats.
                }).collect(),
                truncated:manifest.truncated,
                total_entries:manifest.total_entries as u64,
                total_declared_uncompressed_bytes:manifest.total_uncompressed_bytes,
                archive_bytes:manifest.input_bytes,
            }.encode_to_vec(),
        }})
    }
    if !request.path.to_ascii_lowercase().ends_with(".iso"){
        return Err(refuse("ARCHIVE_FORMAT_UNSUPPORTED",
            "Supported: ZIP, ISO9660, TAR/compressed TAR, GZ/XZ/BZ2/ZST, CPIO, Unix ar and Debian DEB"))
    }
    let max=request.max_entries.unwrap_or(DEFAULT_ENTRIES);
    if !(1..=MAX_RETURNED_ENTRIES).contains(&max){
        return Err(refuse("ARCHIVE_LIMIT","Invalid archive listing limit"))
    }
    let path=safe_relative_name(&request.path,false)?;
    // ISO parser takes a flat, explicitly approved image name; no model
    // supplied directory is followed through this adapter.
    if path.components().count()!=1 {
        return Err(refuse("INVALID_ARCHIVE_PATH","ISO image must be in approved directory"))
    }
    let iso=crate::native_iso9660::inspect_iso(root,&request.path,max as usize)
        .map_err(|_|refuse("ARCHIVE_INVALID","ISO image rejected or exceeds safety limits"))?;
    Ok(ToolOutput{payload:ProtoPayload{
        type_url:OUTPUT.into(),
        bytes:proto::ArchiveListResult {
            entries:iso.entries.into_iter().map(|e|proto::ArchiveEntry {
                path:e.path,
                kind:if e.is_directory{"directory".into()}else{"file".into()},
                compressed_bytes:e.bytes, // ISO9660 members are stored, not compressed.
                declared_uncompressed_bytes:e.bytes,
                crc32:0, // ISO9660 has no per-file CRC32; never infer one.
            }).collect(),
            truncated:iso.truncated,
            total_entries:iso.total_entries as u64,
            total_declared_uncompressed_bytes:iso.declared_bytes,
            archive_bytes:iso.image_bytes,
        }.encode_to_vec(),
    }})
}

struct ArchiveListing {
    descriptor: ToolDescriptor,
    root: Arc<Dir>,
}

impl NativeTool for ArchiveListing {
    fn descriptor(&self) -> &ToolDescriptor { &self.descriptor }
    fn invoke(&self, call: ToolInvocation) -> ToolFuture<'_> {
        let root = Arc::clone(&self.root);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let input = proto::ArchiveListRequest::decode(call.input.bytes.as_slice())
                    .map_err(|_| refuse("BAD_PROTOBUF", "bad archive listing request"))?;
                if input.encode_to_vec() != call.input.bytes {
                    return Err(refuse("BAD_PROTOBUF", "noncanonical archive request"));
                }
                inspect(&root, input)
            }).await.map_err(|_| refuse("ARCHIVE_WORKER", "archive metadata worker failed"))?
        })
    }
}

struct ArchiveCodec;
impl ModelToolCodec for ArchiveCodec {
    fn input_type_url(&self) -> &'static str { INPUT }
    fn output_type_url(&self) -> &'static str { OUTPUT }

    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["path", "max_entries"], &["path"])?;
        let path = args.get("path").and_then(Value::as_str)
            .ok_or_else(|| ModelToolError::InvalidArguments("path must be text".into()))?;
        safe_relative_name(path, false)
            .map_err(|_| ModelToolError::InvalidArguments("unsafe archive path".into()))?;
        if !path.to_ascii_lowercase().ends_with(".zip") && !path.to_ascii_lowercase().ends_with(".iso") && crate::native_linux_archive::detect_linux_archive(path).is_none() && !crate::native_cpio::is_cpio_name(path) && !crate::native_ar_deb::is_ar_name(path) {
            return Err(ModelToolError::InvalidArguments("Unsupported archive format".into()));
        }
        let max_entries = optional_uint(args, "max_entries", DEFAULT_ENTRIES, MAX_RETURNED_ENTRIES)?;
        let message = proto::ArchiveListRequest {
            path: path.into(),
            max_entries: Some(max_entries),
        };
        Ok(ProtoPayload { type_url: INPUT.into(), bytes: message.encode_to_vec() })
    }

    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        let data = proto::ArchiveListResult::decode(output.bytes.as_slice())
            .map_err(|_| ModelToolError::InvalidResult("invalid archive listing result".into()))?;
        Ok(json!({
            "entries": data.entries.into_iter().map(|entry| json!({
                "path": entry.path, "kind": entry.kind,
                "compressed_bytes": entry.compressed_bytes,
                "declared_uncompressed_bytes": entry.declared_uncompressed_bytes,
                "crc32": entry.crc32
            })).collect::<Vec<_>>(),
            "truncated": data.truncated,
            "total_entries": data.total_entries,
            "total_declared_uncompressed_bytes": data.total_declared_uncompressed_bytes,
            "archive_bytes": data.archive_bytes,
            "note": "Archive metadata only. TAR, ISO and compressed streams have no per-member CRC32; zero is not proof of integrity. No extraction occurred."
        }))
    }
}

/// Register a narrow, read-only ZIP listing tool. The trusted host selects
/// the allowed root, and separately authorizes the exact tool/effect/target
/// against the persisted user turn. Registration is NEVER a model grant.
pub fn register_archive_inspection_tool(
    registry: &NativeToolRegistry,
    catalog: &mut ModelToolCatalog,
    authorized_root: &Path,
) -> Result<(), String> {
    let root = authorized_root.canonicalize()
        .map_err(|_| "trusted archive workspace root unavailable")?;
    if !root.is_dir() {
        return Err("trusted archive root must be a directory".into());
    }
    let root = Arc::new(Dir::open_ambient_dir(&root, ambient_authority())
        .map_err(|_| "trusted archive workspace root cannot be opened")?);
    let descriptor = ToolDescriptor {
        id: LIST.into(),
        display_name: "List ZIP / ISO9660 contents (metadata only)".into(),
        effect: ToolEffect::ReadWorkspace,
        input_type_url: INPUT.into(),
        output_type_url: OUTPUT.into(),
    };
    registry.register(
        ModuleManifest {
            id: MODULE.into(),
            display_name: "Lu Core archive inspection".into(),
            contract_version: CORE_CONTRACT_VERSION,
            kind: ModuleKind::Core,
            tools: vec![descriptor.clone()],
            panels: vec![],
            workflows: vec![],
        },
        vec![Arc::new(ArchiveListing { descriptor, root })],
    ).map_err(|_| "archive registry refused native tool")?;
    catalog.insert(registry, ModelToolDefinition {
        name: "lu_archive_list".into(),
        native_tool_id: LIST.into(),
        description: "Read-only inspect ZIP, ISO9660, TAR and compressed streams, CPIO newc/crc, Unix ar and Debian DEB packages; no execution or extraction.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {"type":"string","description":"Archive filename relative to the approved workspace"},
                "max_entries":{"type":"integer","minimum":1,"maximum":MAX_RETURNED_ENTRIES}
            },
            "required":["path"],
            "additionalProperties":false
        }),
        codec: Arc::new(ArchiveCodec),
    }).map_err(|_| "archive JSON/Protobuf schema registration refused")?;
    Ok(())
}
