//! Host-scoped read-only Git inspection for Lu: status, recent log, diff.
//! No shell, arbitrary command string, hooks, network, repository mutation or
//! MCP. The host, not the model, chooses the repository AND Git executable.
//! The native tool accepts only bounded typed Protobuf arguments.
//!
//! Executing a trusted Git binary is a deliberate first-party subprocess
//! exception, NOT a generic terminal capability. Every argument is fixed or
//! narrowly typed; model-supplied Git flags, refs and executables are forbidden.

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
use prost::Message;
use serde_json::{json, Map, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const MODULE: &str = "lu.git";
const STATUS: &str = "lu.git.status";
const LOG: &str = "lu.git.log";
const DIFF: &str = "lu.git.diff";
const STATUS_IN: &str = "type.googleapis.com/lu.v1.GitStatusRequest";
const STATUS_OUT: &str = "type.googleapis.com/lu.v1.GitStatusResult";
const LOG_IN: &str = "type.googleapis.com/lu.v1.GitLogRequest";
const LOG_OUT: &str = "type.googleapis.com/lu.v1.GitLogResult";
const DIFF_IN: &str = "type.googleapis.com/lu.v1.GitDiffRequest";
const DIFF_OUT: &str = "type.googleapis.com/lu.v1.GitDiffResult";
const MAX_STATUS_ENTRIES: u32 = 100;
const MAX_LOG_COMMITS: u32 = 20;
const MAX_DIFF_BYTES: u32 = 64 * 1024;
const MAX_STATUS_CAPTURE: usize = 256 * 1024;
const MAX_LOG_CAPTURE: usize = 32 * 1024;
const GIT_TIMEOUT: Duration = Duration::from_secs(8);

fn denied(code: &str, message: &str) -> ToolFailure {
    ToolFailure::new(code, message, false)
}

fn validated_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Ok(());
    }
    if path.len() > 4096
        || Path::new(path).is_absolute()
        || path.chars().any(|c| c.is_control() || matches!(c, '\\' | ':'))
        || path.split('/').any(|c| c.is_empty() || c == "." || c == ".."
            || c == ".git" || c.starts_with('-'))
    {
        return Err("expected a simple workspace-relative tracked path".into());
    }
    Ok(())
}

#[derive(Clone)]
struct ApprovedGit {
    root: PathBuf,
    git_dir: PathBuf,
    executable: PathBuf,
}

/// Git for Windows does not consistently accept Rust's verbatim Win32
/// \\?\C:\... canonical paths as --git-dir / --work-tree arguments.
/// This strips the *local disk* extended-length prefix for CLI transport
/// only; the stored approved canonical path and reparse-point checks remain.
fn git_argument_path(path: &Path) -> Result<PathBuf, ToolFailure> {
    #[cfg(windows)]
    {
        let raw=path.to_str().ok_or_else(||
            denied("REPOSITORY_CHANGED","Git path is not valid Windows Unicode"))?;
        if let Some(drive)=raw.strip_prefix(r"\\?\") {
            let b=drive.as_bytes();
            if b.len() >= 3 && b[0].is_ascii_alphabetic()
                && b[1]==b':' && b[2]==b'\\' {
                return Ok(PathBuf::from(drive));
            }
            return Err(denied("REPOSITORY_CHANGED",
                "non-local verbatim Git paths are not supported"));
        }
    }
    Ok(path.to_path_buf())
}

impl ApprovedGit {
    fn open(root: &Path, executable: &Path) -> Result<Self, String> {
        // Host-owned paths; never taken from a model, JSON argument, tool
        // description, user-supplied repository metadata or environment.
        let root = root.canonicalize().map_err(|_| "approved repository unavailable")?;
        if !root.is_dir() {
            return Err("approved repository must be a directory".into());
        }
        let git_meta = fs::symlink_metadata(root.join(".git"))
            .map_err(|_| "repository must contain a normal .git directory")?;
        if !git_meta.is_dir() || git_meta.file_type().is_symlink() {
            return Err("Git worktree indirection and linked .git metadata are unsupported".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            let root_meta = fs::symlink_metadata(&root)
                .map_err(|_| "cannot inspect approved repository")?;
            if (root_meta.file_attributes() | git_meta.file_attributes()) & 0x400 != 0 {
                return Err("repository Windows reparse point refused".into());
            }
        }
        let git_dir = root.join(".git");
        // Git alternates, common-dir indirections and replacement object refs
        // can redirect inspection to outside an approved repository.
        for name in ["commondir", "objects/info/alternates"] {
            if fs::symlink_metadata(git_dir.join(name)).is_ok() {
                return Err("external Git object-store indirection is not approved".into());
            }
        }
        if fs::symlink_metadata(git_dir.join("objects")).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("linked object store is not approved".into());
        }
        let executable = executable.canonicalize()
            .map_err(|_| "host-selected Git executable unavailable")?;
        let exe = fs::metadata(&executable).map_err(|_| "Git executable unavailable")?;
        if !exe.is_file() || executable.starts_with(&root) {
            return Err("Git executable must be an independent approved native program".into());
        }
        Ok(Self { root, git_dir, executable })
    }

    /// Run a single allowlisted read-only Git operation. No shell. The env is
    /// cleared, local Git config cannot run fsmonitor, hooks, external diff
    /// helpers, pagers, textconv filters or network credentials. We override
    /// gitdir/worktree instead of trusting core.worktree or GIT_DIR.
    fn run(&self, args: &[String], max_capture: usize) -> Result<(Vec<u8>, bool), ToolFailure> {
        let git_meta = fs::symlink_metadata(&self.git_dir)
            .map_err(|_| denied("REPOSITORY_CHANGED", "approved .git directory vanished"))?;
        if !git_meta.is_dir() || git_meta.file_type().is_symlink() {
            return Err(denied("REPOSITORY_CHANGED", "approved .git directory changed"));
        }
        let cli_gitdir=git_argument_path(&self.git_dir)?;
        let cli_worktree=git_argument_path(&self.root)?;
        let mut command = Command::new(&self.executable);
        command
            .arg("--no-optional-locks")
            .arg("--no-pager")
            .arg(format!("--git-dir={}", cli_gitdir.display()))
            .arg(format!("--work-tree={}", cli_worktree.display()))
            .arg("-c").arg("core.fsmonitor=false")
            .arg("-c").arg("core.hooksPath=/dev/null")
            .arg("-c").arg("core.pager=cat")
            .arg("-c").arg("diff.external=")
            .arg("-c").arg("diff.trustExitCode=false")
            .args(args)
            .env_clear()
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", if cfg!(windows) { "NUL" } else { "/dev/null" })
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_PAGER", "cat")
            .env("GIT_EXTERNAL_DIFF", "")
            .env("GIT_LFS_SKIP_SMUDGE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // Windows CreateProcess and Git's signed native install expect this
        // variable for system DLL discovery; it is not a command interpreter.
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
        let mut child = command.spawn()
            .map_err(|_| denied("GIT_UNAVAILABLE", "cannot start approved Git executable"))?;
        let Some(mut stdout) = child.stdout.take() else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(denied("GIT_FAILED", "Git stdout unavailable"));
        };
        let exceeded = Arc::new(AtomicBool::new(false));
        let over = Arc::clone(&exceeded);
        let reader = std::thread::spawn(move || {
            let mut result = Vec::new();
            let read = (&mut stdout).take(max_capture as u64 + 1)
                .read_to_end(&mut result);
            if result.len() > max_capture {
                over.store(true, Ordering::Release);
                result.truncate(max_capture);
            }
            read.map(|_| result)
        });
        let deadline = Instant::now() + GIT_TIMEOUT;
        let mut exit_ok = false;
        let mut capped = false;
        loop {
            if exceeded.load(Ordering::Acquire) {
                capped = true;
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            match child.try_wait() {
                Ok(Some(status)) => {
                    exit_ok = status.success();
                    break;
                }
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    return Err(denied("GIT_TIMEOUT", "Git inspection did not complete"));
                }
            }
        }
        let data = reader.join()
            .map_err(|_| denied("GIT_FAILED", "Git output capture stopped"))?
            .map_err(|_| denied("GIT_FAILED", "cannot read Git output"))?;
        let capped = capped || exceeded.load(Ordering::Acquire);
        if !exit_ok && !capped {
            return Err(denied("GIT_FAILED", "read-only Git inspection refused or failed"));
        }
        Ok((data, capped))
    }
}

fn status(repo: &ApprovedGit, max_entries: u32) -> Result<ToolOutput, ToolFailure> {
    if !(1..=MAX_STATUS_ENTRIES).contains(&max_entries) {
        return Err(denied("INVALID_LIMIT", "invalid Git status entry limit"));
    }
    let args = ["status", "--porcelain=v1", "-z", "--no-renames",
        "--untracked-files=normal", "--ignore-submodules=all"]
        .into_iter().map(str::to_owned).collect::<Vec<_>>();
    let (output, capped) = repo.run(&args, MAX_STATUS_CAPTURE)?;
    if capped {
        return Err(denied("STATUS_TOO_LARGE", "Git status exceeded bounded output"));
    }
    let mut entries = Vec::new();
    let mut truncated = false;
    for item in output.split(|byte| *byte == 0).filter(|item| !item.is_empty()) {
        if item.len() < 4 || item[2] != b' ' {
            return Err(denied("BAD_GIT_OUTPUT", "Git status record malformed"));
        }
        if entries.len() >= max_entries as usize {
            truncated = true;
            break;
        }
        entries.push(proto::GitStatusEntry {
            path: String::from_utf8_lossy(&item[3..]).into_owned(),
            state: String::from_utf8_lossy(&item[..2]).into_owned(),
        });
    }
    let payload = proto::GitStatusResult { entries, truncated };
    Ok(ToolOutput { payload: ProtoPayload {
        type_url: STATUS_OUT.into(), bytes: payload.encode_to_vec(),
    } })
}

fn log(repo: &ApprovedGit, limit: u32) -> Result<ToolOutput, ToolFailure> {
    if !(1..=MAX_LOG_COMMITS).contains(&limit) {
        return Err(denied("INVALID_LIMIT", "invalid Git history limit"));
    }
    let args = vec![
        "log".into(), "--no-show-signature".into(), "--no-decorate".into(),
        format!("-n{limit}"), "--format=%H%x00%ct%x00%s%x00".into(),
        "HEAD".into(),
    ];
    let (output, capped) = repo.run(&args, MAX_LOG_CAPTURE)?;
    if capped {
        return Err(denied("LOG_TOO_LARGE", "Git log exceeded bounded output"));
    }
    let mut commits = Vec::new();
    for line in output.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()) {
        let fields: Vec<_> = line.split(|b| *b == 0).collect();
        if fields.len() != 4 || !fields[3].is_empty() {
            return Err(denied("BAD_GIT_OUTPUT", "Git log record malformed"));
        }
        let oid = String::from_utf8(fields[0].to_vec())
            .map_err(|_| denied("BAD_GIT_OUTPUT", "invalid Git object ID"))?;
        if oid.len() != 40 || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(denied("BAD_GIT_OUTPUT", "invalid Git object ID"));
        }
        let when = std::str::from_utf8(fields[1]).ok()
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| denied("BAD_GIT_OUTPUT", "invalid Git commit time"))?;
        let subject = String::from_utf8_lossy(fields[2]).chars()
            .filter(|c| !c.is_control()).take(240).collect();
        commits.push(proto::GitCommitSummary {
            commit: oid, unix_time: when, subject,
        });
    }
    let result = proto::GitLogResult { commits, truncated: false };
    Ok(ToolOutput { payload: ProtoPayload {
        type_url: LOG_OUT.into(), bytes: result.encode_to_vec(),
    } })
}

fn diff(repo: &ApprovedGit, request: proto::GitDiffRequest) -> Result<ToolOutput, ToolFailure> {
    validated_path(&request.path).map_err(|_| denied("INVALID_PATH", "unsafe Git diff path"))?;
    let max_bytes = request.max_bytes.unwrap_or(16 * 1024);
    if !(1..=MAX_DIFF_BYTES).contains(&max_bytes) {
        return Err(denied("INVALID_LIMIT", "invalid Git diff byte limit"));
    }
    let mut args = [
        "diff", "--no-ext-diff", "--no-textconv", "--no-renames",
        "--ignore-submodules=all", "--no-color", "--unified=3", "HEAD", "--",
    ].into_iter().map(str::to_owned).collect::<Vec<_>>();
    if !request.path.is_empty() {
        args.push(request.path);
    }
    let (data, truncated) = repo.run(&args, max_bytes as usize)?;
    let observed_bytes = data.len() as u64;
    let result = proto::GitDiffResult {
        diff: String::from_utf8_lossy(&data).into_owned(),
        truncated, observed_bytes,
    };
    Ok(ToolOutput { payload: ProtoPayload {
        type_url: DIFF_OUT.into(), bytes: result.encode_to_vec(),
    } })
}

#[derive(Clone, Copy)]
enum Operation { Status, Log, Diff }

struct GitTool {
    descriptor: ToolDescriptor,
    repository: Arc<ApprovedGit>,
    operation: Operation,
}
impl NativeTool for GitTool {
    fn descriptor(&self) -> &ToolDescriptor { &self.descriptor }
    fn invoke(&self, call: ToolInvocation) -> ToolFuture<'_> {
        let repo = Arc::clone(&self.repository);
        let op = self.operation;
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                match op {
                    Operation::Status => {
                        let input = proto::GitStatusRequest::decode(call.input.bytes.as_slice())
                            .map_err(|_| denied("BAD_PROTOBUF", "invalid Git status request"))?;
                        if input.encode_to_vec() != call.input.bytes {
                            return Err(denied("BAD_PROTOBUF", "noncanonical Git status request"));
                        }
                        status(&repo, input.max_entries.unwrap_or(40))
                    }
                    Operation::Log => {
                        let input = proto::GitLogRequest::decode(call.input.bytes.as_slice())
                            .map_err(|_| denied("BAD_PROTOBUF", "invalid Git log request"))?;
                        if input.encode_to_vec() != call.input.bytes {
                            return Err(denied("BAD_PROTOBUF", "noncanonical Git log request"));
                        }
                        log(&repo, input.limit.unwrap_or(10))
                    }
                    Operation::Diff => {
                        let input = proto::GitDiffRequest::decode(call.input.bytes.as_slice())
                            .map_err(|_| denied("BAD_PROTOBUF", "invalid Git diff request"))?;
                        if input.encode_to_vec() != call.input.bytes {
                            return Err(denied("BAD_PROTOBUF", "noncanonical Git diff request"));
                        }
                        diff(&repo, input)
                    }
                }
            }).await.map_err(|_| denied("GIT_WORKER", "Git inspection worker stopped"))?
        })
    }
}

struct StatusCodec;
impl ModelToolCodec for StatusCodec {
    fn input_type_url(&self) -> &'static str { STATUS_IN }
    fn output_type_url(&self) -> &'static str { STATUS_OUT }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["max_entries"], &[])?;
        let input = proto::GitStatusRequest {
            max_entries: Some(optional_uint(args, "max_entries", 40, MAX_STATUS_ENTRIES)?),
        };
        Ok(ProtoPayload { type_url: STATUS_IN.into(), bytes: input.encode_to_vec() })
    }
    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        let reply = proto::GitStatusResult::decode(output.bytes.as_slice())
            .map_err(|_| ModelToolError::InvalidResult("bad Git status reply".into()))?;
        Ok(json!({"entries": reply.entries.into_iter().map(|e|
            json!({"path":e.path,"state":e.state})).collect::<Vec<_>>(),
            "truncated": reply.truncated}))
    }
}
struct LogCodec;
impl ModelToolCodec for LogCodec {
    fn input_type_url(&self) -> &'static str { LOG_IN }
    fn output_type_url(&self) -> &'static str { LOG_OUT }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["limit"], &[])?;
        let input = proto::GitLogRequest {
            limit: Some(optional_uint(args, "limit", 10, MAX_LOG_COMMITS)?),
        };
        Ok(ProtoPayload { type_url: LOG_IN.into(), bytes: input.encode_to_vec() })
    }
    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        let reply = proto::GitLogResult::decode(output.bytes.as_slice())
            .map_err(|_| ModelToolError::InvalidResult("bad Git log reply".into()))?;
        Ok(json!({"commits": reply.commits.into_iter().map(|e|
            json!({"commit":e.commit,"unix_time":e.unix_time,"subject":e.subject}))
                .collect::<Vec<_>>(), "truncated": reply.truncated}))
    }
}
struct DiffCodec;
impl ModelToolCodec for DiffCodec {
    fn input_type_url(&self) -> &'static str { DIFF_IN }
    fn output_type_url(&self) -> &'static str { DIFF_OUT }
    fn encode(&self, args: &Map<String, Value>) -> Result<ProtoPayload, ModelToolError> {
        expect_fields(args, &["path", "max_bytes"], &[])?;
        let path = match args.get("path") {
            None => String::new(),
            Some(Value::String(path)) => path.clone(),
            _ => return Err(ModelToolError::InvalidArguments("path must be text".into())),
        };
        validated_path(&path).map_err(ModelToolError::InvalidArguments)?;
        let input = proto::GitDiffRequest {
            path, max_bytes: Some(optional_uint(args, "max_bytes", 16*1024, MAX_DIFF_BYTES)?),
        };
        Ok(ProtoPayload { type_url: DIFF_IN.into(), bytes: input.encode_to_vec() })
    }
    fn decode(&self, output: &ProtoPayload) -> Result<Value, ModelToolError> {
        let reply = proto::GitDiffResult::decode(output.bytes.as_slice())
            .map_err(|_| ModelToolError::InvalidResult("bad Git diff reply".into()))?;
        Ok(json!({"diff":reply.diff,"truncated":reply.truncated,
            "observed_bytes":reply.observed_bytes}))
    }
}

/// The trusted Lu host passes (1) the already user-approved repository root
/// and (2) the trusted canonical Git binary location. No model can choose
/// these. This method registers schemas; discovery never implies execution
/// authorization. The caller must separately grant ReadWorkspace per turn.
pub fn register_git_inspection_tools(
    registry: &NativeToolRegistry,
    catalog: &mut ModelToolCatalog,
    authorized_repository: &Path,
    trusted_git_executable: &Path,
) -> Result<(), String> {
    let approved = Arc::new(ApprovedGit::open(
        authorized_repository, trusted_git_executable,
    )?);
    let descriptor = |id: &str, name: &str, input: &str, output: &str| ToolDescriptor {
        id: id.into(), display_name: name.into(), effect: ToolEffect::ReadWorkspace,
        input_type_url: input.into(), output_type_url: output.into(),
    };
    let st = descriptor(STATUS, "Inspect Git worktree status", STATUS_IN, STATUS_OUT);
    let lg = descriptor(LOG, "Inspect recent Git commits", LOG_IN, LOG_OUT);
    let df = descriptor(DIFF, "Inspect Git tracked diff", DIFF_IN, DIFF_OUT);
    registry.register(
        ModuleManifest {
            id: MODULE.into(), display_name: "Lu native read-only Git".into(),
            contract_version: CORE_CONTRACT_VERSION, kind: ModuleKind::Core,
            tools: vec![st.clone(), lg.clone(), df.clone()],
            panels: vec![], workflows: vec![],
        },
        vec![
            Arc::new(GitTool { descriptor: st, repository: Arc::clone(&approved),
                operation: Operation::Status }),
            Arc::new(GitTool { descriptor: lg, repository: Arc::clone(&approved),
                operation: Operation::Log }),
            Arc::new(GitTool { descriptor: df, repository: approved,
                operation: Operation::Diff }),
        ],
    ).map_err(|e| format!("Git inspection registry refused: {e:?}"))?;

    let defs: Vec<(&str, &str, &str, &str, Value, Arc<dyn ModelToolCodec>)> = vec![
        ("lu_git_status", STATUS, "Read working tree changes without modifying Git.",
            STATUS_IN, json!({"type":"object","properties":{
                "max_entries":{"type":"integer","minimum":1,"maximum":MAX_STATUS_ENTRIES}
            },"required":[],"additionalProperties":false}), Arc::new(StatusCodec)),
        ("lu_git_log", LOG, "Read the latest committed history.",
            LOG_IN, json!({"type":"object","properties":{
                "limit":{"type":"integer","minimum":1,"maximum":MAX_LOG_COMMITS}
            },"required":[],"additionalProperties":false}), Arc::new(LogCodec)),
        ("lu_git_diff", DIFF, "Read bounded tracked Git diff; optional relative path.",
            DIFF_IN, json!({"type":"object","properties":{
                "path":{"type":"string","description":"Optional repo-relative tracked file path"},
                "max_bytes":{"type":"integer","minimum":1,"maximum":MAX_DIFF_BYTES}
            },"required":[],"additionalProperties":false}), Arc::new(DiffCodec)),
    ];
    for (name, id, description, _input, parameters, codec) in defs {
        catalog.insert(registry, ModelToolDefinition {
            name:name.into(), native_tool_id:id.into(), description:description.into(),
            parameters, codec,
        }).map_err(|e| format!("Git model schema refused: {e:?}"))?;
    }
    Ok(())
}
