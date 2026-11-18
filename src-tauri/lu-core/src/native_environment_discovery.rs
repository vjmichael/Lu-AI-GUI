//! TOOLS-06 — host-owned environment capability discovery for Lu Core.
//!
//! Original Jan-Mod Design §6A: basic OS + installed-runtime facts should
//! be gathered by the host and injected with the permitted tool inventory,
//! NOT rediscovered by the model on each turn. Original §5.2 "check_env" is
//! a narrow *on-demand* refresh operation in the unified exec domain.
//!
//! This module NEVER executes a program, searches arbitrary PATH directories,
//! reads an .env file, returns an executable path, reveals credentials, or
//! infers run-time compatibility from mere file presence. The trusted host
//! chooses every candidate executable location. A model only chooses a
//! registered, preapproved program identifier, inside existing task authority.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use prost::Message;
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

const MODULE: &str = "lu.exec";
const TOOL_ID: &str = "lu.exec.check_env";
const MODEL_NAME: &str = "lu_check_env";
const INPUT: &str = "type.googleapis.com/lu.v1.EnvironmentCheckRequest";
const OUTPUT: &str = "type.googleapis.com/lu.v1.EnvironmentCheckResult";
const MAX_PROGRAMS: usize = 12;
const MAX_CANDIDATES: usize = 6;

/// Fixed Core runtime inventory. 'gh' is intentionally excluded: GitHub CLI
/// service/authentication workflows belong to the future IDE DLC.
const PROGRAM_IDS: [&str; 12] = [
    "python", "node", "rustc", "cargo", "git", "ffmpeg",
    "tesseract", "cmake", "ninja", "gcc", "clang", "7zip",
];

fn permitted_id(id: &str) -> bool {
    PROGRAM_IDS.contains(&id)
}

fn refused(code: &str, message: &str) -> ToolFailure {
    ToolFailure::new(code, message, false)
}

/// The host supplies exact candidate filenames, NOT untrusted model paths,
/// PATH strings, shell commands, file glob patterns or registry searches.
#[derive(Debug, Clone)]
pub struct HostProgramSpec {
    pub id: String,
    pub candidate_files: Vec<PathBuf>,
}

/// Safe, nonsecret availability observation, suitable for the §6A ambient
/// summary. 'candidate_present' is not 'installed and working'.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramObservation {
    pub program_id: String,
    pub candidate_present: bool,
    pub evidence: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbientProgramInventory {
    pub operating_system: &'static str,
    pub architecture: &'static str,
    pub programs: Vec<ProgramObservation>,
}

#[derive(Debug)]
pub struct HostProgramInventory {
    selected: BTreeMap<String, Vec<PathBuf>>,
}

impl HostProgramInventory {
    pub fn new(specs: Vec<HostProgramSpec>) -> Result<Self, String> {
        if specs.is_empty() || specs.len() > MAX_PROGRAMS {
            return Err("host program catalog must contain 1–12 approved IDs".into());
        }
        let mut selected = BTreeMap::new();
        for spec in specs {
            if !permitted_id(&spec.id)
                || spec.candidate_files.is_empty()
                || spec.candidate_files.len() > MAX_CANDIDATES
            {
                return Err("unsupported or empty host program specification".into());
            }
            for file in &spec.candidate_files {
                if !file.is_absolute()
                    || file.as_os_str().len() > 4096
                    || file.to_string_lossy().chars().any(|c| c.is_control())
                    || file.components().any(|comp| matches!(
                        comp, std::path::Component::ParentDir | std::path::Component::CurDir
                    ))
                {
                    return Err("program candidate must be an exact host-selected absolute file".into());
                }
                let Some(name) = file.file_name().and_then(|s| s.to_str()) else {
                    return Err("invalid program candidate filename".into());
                };
                if name.starts_with('-') {
                    return Err("invalid program candidate filename".into());
                }
                // Reject script/batch entrypoints that could spawn an
                // interpreter if a later host accidentally reused this list.
                if cfg!(windows) && !name.to_ascii_lowercase().ends_with(".exe") {
                    return Err("only native .exe candidates may be observed on Windows".into());
                }
            }
            if selected.insert(spec.id, spec.candidate_files).is_some() {
                return Err("duplicate host program ID".into());
            }
        }
        Ok(Self { selected })
    }

    fn is_candidate_present(file: &Path) -> bool {
        let Ok(metadata) = fs::symlink_metadata(file) else {
            return false;
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return false;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return false; // reparse points are never considered candidates
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return false;
            }
        }
        // This proves only that the host-named executable file exists and
        // looks executable. It does not start it or validate interpreter,
        // dynamic libraries, ABI, version, driver or tool behavior.
        true
    }

    pub fn check(&self, id: &str) -> Result<ProgramObservation, String> {
        let files = self.selected.get(id).ok_or("program not in approved host inventory")?;
        let present = files.iter().any(|p| Self::is_candidate_present(p));
        Ok(ProgramObservation {
            program_id: id.to_owned(),
            candidate_present: present,
            evidence: if present { "file_present_only" } else { "not_found" },
        })
    }

    /// The trusted host can refresh and inject this at startup/per-turn as
    /// §6A context without consuming a model tool call or exposing local paths.
    /// It must also filter by the actually authorized tool inventory; this
    /// function is an observation, NOT a permission/token or a prompt builder.
    pub fn ambient_snapshot(&self) -> AmbientProgramInventory {
        AmbientProgramInventory {
            operating_system: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            programs: self.selected.keys()
                .filter_map(|id| self.check(id).ok())
                .collect(),
        }
    }
}

fn handle(inventory: &HostProgramInventory, req: proto::EnvironmentCheckRequest)
    -> Result<ToolOutput, ToolFailure>
{
    let observation = inventory.check(&req.program_id)
        .map_err(|_| refused("PROGRAM_NOT_ADVERTISED", "program not in approved inventory"))?;
    let result = proto::EnvironmentCheckResult {
        program_id: observation.program_id,
        candidate_present: observation.candidate_present,
        evidence: observation.evidence.into(),
        operating_system: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
    };
    Ok(ToolOutput { payload: ProtoPayload {
        type_url: OUTPUT.into(), bytes: result.encode_to_vec(),
    }})
}

struct EnvironmentCheck {
    descriptor: ToolDescriptor,
    inventory: Arc<HostProgramInventory>,
}

impl NativeTool for EnvironmentCheck {
    fn descriptor(&self) -> &ToolDescriptor { &self.descriptor }
    fn invoke(&self, call: ToolInvocation) -> ToolFuture<'_> {
        let inventory = Arc::clone(&self.inventory);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                let request = proto::EnvironmentCheckRequest::decode(call.input.bytes.as_slice())
                    .map_err(|_| refused("BAD_PROTOBUF", "invalid environment request"))?;
                if request.encode_to_vec() != call.input.bytes {
                    return Err(refused("BAD_PROTOBUF", "noncanonical environment request"));
                }
                handle(&inventory, request)
            }).await.map_err(|_| refused("ENV_WORKER_FAILED", "environment inspection worker failed"))?
        })
    }
}

struct EnvironmentCodec { allowed: BTreeSet<String> }
impl ModelToolCodec for EnvironmentCodec {
    fn input_type_url(&self) -> &'static str { INPUT }
    fn output_type_url(&self) -> &'static str { OUTPUT }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["program"], &["program"])?;
        let id = args.get("program").and_then(Value::as_str)
            .ok_or_else(|| ModelToolError::InvalidArguments("program must be a string".into()))?;
        if !self.allowed.contains(id) {
            return Err(ModelToolError::InvalidArguments(
                "program not selected in this host inventory".into(),
            ));
        }
        Ok(ProtoPayload {
            type_url: INPUT.into(),
            bytes: proto::EnvironmentCheckRequest { program_id: id.into() }.encode_to_vec(),
        })
    }
    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        let result = proto::EnvironmentCheckResult::decode(output.bytes.as_slice())
            .map_err(|_| ModelToolError::InvalidResult("invalid environment result".into()))?;
        if !self.allowed.contains(&result.program_id)
            || !matches!(result.evidence.as_str(), "file_present_only" | "not_found")
            || (result.candidate_present != (result.evidence == "file_present_only"))
            || result.operating_system != std::env::consts::OS
            || result.architecture != std::env::consts::ARCH
        {
            return Err(ModelToolError::InvalidResult("inconsistent host environment evidence".into()));
        }
        Ok(json!({
            "program": result.program_id,
            "candidate_present": result.candidate_present,
            "evidence": result.evidence,
            "os": result.operating_system,
            "arch": result.architecture,
            "version": null,
            "execution_verified": false,
            "note": "Host-named file presence only. Not proof of installation, functionality or execution permission."
        }))
    }
}

/// Register a read-only environment check in the **future unified exec domain**.
/// Host approves candidate filenames before registration and independently
/// grants this task's CheckEnv use; schema discovery never grants execution.
/// The returned inventory is also usable by the host to build ambient context.
pub fn register_environment_discovery_tool(
    registry: &NativeToolRegistry,
    catalog: &mut ModelToolCatalog,
    specs: Vec<HostProgramSpec>,
) -> Result<Arc<HostProgramInventory>, String> {
    let inventory = Arc::new(HostProgramInventory::new(specs)?);
    let allowed: BTreeSet<_> = inventory.selected.keys().cloned().collect();
    let descriptor = ToolDescriptor {
        id: TOOL_ID.into(),
        display_name: "Inspect host-approved runtime availability".into(),
        // The native effect enum has no pure environment query category yet.
        // Admission is conservative and READ-ONLY, despite no workspace I/O.
        effect: ToolEffect::ReadWorkspace,
        input_type_url: INPUT.into(), output_type_url: OUTPUT.into(),
    };
    registry.register(
        ModuleManifest {
            id: MODULE.into(), display_name: "Lu native execution preflight".into(),
            contract_version: CORE_CONTRACT_VERSION, kind: ModuleKind::Core,
            tools: vec![descriptor.clone()], panels: vec![], workflows: vec![],
        },
        vec![Arc::new(EnvironmentCheck {
            descriptor, inventory: Arc::clone(&inventory),
        })],
    ).map_err(|e| format!("environment registry admission refused: {e:?}"))?;
    let ids = allowed.iter().cloned().collect::<Vec<_>>();
    catalog.insert(registry, ModelToolDefinition {
        name: MODEL_NAME.into(),
        native_tool_id: TOOL_ID.into(),
        description: "Refresh host-approved runtime file presence only; does not execute programs.".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "program": {"type":"string", "enum":ids,
                    "description":"Host-approved runtime identifier, not a path or shell command"}
            },
            "required":["program"],
            "additionalProperties":false
        }),
        codec: Arc::new(EnvironmentCodec { allowed }),
    }).map_err(|e| format!("environment model schema admission refused: {e:?}"))?;
    Ok(inventory)
}
