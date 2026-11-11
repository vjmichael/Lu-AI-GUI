//! E2 native Windows/Linux Tauri host for reviewed installer-packaged language servers.
//! This module is compiled ONLY with explicit native-lsp feature. The default
//! IDE alpha exposes no language-server executable or Lu bridge.
//! An OS dialog (not a UI boolean) grants one fresh D3 job for an exact source
//! snapshot after release pins, registered project and disk revision checks.
#![cfg(all(any(windows, target_os = "linux"), feature = "native-lsp"))]
#[cfg(target_os = "linux")]
use lu_ide_execution_supervisor::linux_worker::lsp_native::{
    native_lsp_bundle_digest, start_linux_lsp, LinuxLspSession,
};
use lu_ide_execution_supervisor::{snapshot::publish_lsp_input, JobPlan, Supervisor};
#[cfg(windows)]
use lu_ide_execution_supervisor::{
    windows_broker::TrustedBroker,
    windows_worker::lsp_native::{native_lsp_bundle_digest, start_windows_lsp, WindowsLspSession},
};
use lu_ide_workspace::IdeWorkspaceRegistry;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
#[cfg(windows)]
type NativeLspSession = WindowsLspSession;
#[cfg(target_os = "linux")]
type NativeLspSession = LinuxLspSession;
use lu_ide_workspace::{
    lsp::DiagnosticSource,
    lsp_session::{position_to_utf16, Completion, Definition, Diagnostic, LspEvent, Position},
};

// Pins are embedded at build time by the reviewed installer release job.
// Runtime environment variables, user selected paths, repo configs and
// language server replies cannot override these values.
const APPROVED_CLANGD_BUNDLE: Option<&str> = option_env!("LU_IDE_E2_CLANGD_BUNDLE_SHA256");
const APPROVED_RUST_ANALYZER_BUNDLE: Option<&str> =
    option_env!("LU_IDE_E2_RUST_ANALYZER_BUNDLE_SHA256");
const APPROVED_RUFF_BUNDLE: Option<&str> = option_env!("LU_IDE_E2_RUFF_BUNDLE_SHA256");
// An installer-reviewed, wholly offline standalone Pyright language-server
// executable bundle. No user Node/Python interpreter, PATH or npm download.
const APPROVED_PYRIGHT_BUNDLE: Option<&str> =
    option_env!("LU_IDE_E2_PYRIGHT_BUNDLE_SHA256");
#[cfg(windows)]
const APPROVED_SERVICE: Option<&str> = option_env!("LU_IDE_E2_D7D_SERVICE_SHA256");

fn pin(value: Option<&str>) -> Result<&str, String> {
    let text = value.ok_or("LSP_RELEASE_PINS_ABSENT")?;
    if text.len() != 64 || !text.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("LSP_RELEASE_PINS_INVALID".into());
    }
    Ok(text)
}
#[cfg(windows)]
fn hash_file(path: &Path) -> Result<String, String> {
    let mut f = File::open(path).map_err(|_| "LSP_SERVICE_MISSING")?;
    let mut hasher = Sha256::new();
    let mut bytes = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut bytes).map_err(|_| "LSP_SERVICE_UNREADABLE")?;
        if n == 0 {
            break;
        }
        hasher.update(&bytes[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
struct Installation {
    bundle: PathBuf,
    #[cfg(windows)]
    service: TrustedBroker,
    bundle_sha256: String,
}
fn installation(app: &AppHandle, server_id: &str) -> Result<Installation, String> {
    let (recipe, approved) = match server_id {
        "clangd" => ("lsp_clangd", APPROVED_CLANGD_BUNDLE),
        "rust-analyzer" => ("lsp_rust_analyzer", APPROVED_RUST_ANALYZER_BUNDLE),
        "ruff" => ("lsp_ruff", APPROVED_RUFF_BUNDLE),
        "pyright" => ("lsp_pyright", APPROVED_PYRIGHT_BUNDLE),
        _ => return Err("LSP_SERVER_NOT_APPROVED".into()),
    };
    let root = app
        .path()
        .resource_dir()
        .map_err(|_| "LSP_RESOURCE_ROOT_UNAVAILABLE")?;
    let bundle = root.join("language-servers").join(server_id);
    let expected_bundle = pin(approved)?;
    #[cfg(windows)]
    let service = root.join("recovery").join("windows_recovery_service.exe");
    #[cfg(windows)]
    let expected_service = pin(APPROVED_SERVICE)?;
    let observed = native_lsp_bundle_digest(&bundle, recipe).map_err(|_| "LSP_UNTRUSTED_BUNDLE")?;
    if observed != expected_bundle {
        return Err("LSP_BUNDLE_PIN_MISMATCH".into());
    }
    #[cfg(windows)]
    if hash_file(&service)? != expected_service {
        return Err("LSP_SERVICE_PIN_MISMATCH".into());
    }
    #[cfg(windows)]
    let broker = TrustedBroker {
        executable: service,
        expected_sha256: expected_service.to_owned(),
        private_witness_directory: root.join("recovery"),
    };
    Ok(Installation {
        bundle,
        #[cfg(windows)]
        service: broker,
        bundle_sha256: observed,
    })
}
pub fn installed_languages(app: &AppHandle) -> Vec<String> {
    let mut result = Vec::new();
    if installation(app, "clangd").is_ok() {
        result.push("c".into());
        result.push("cpp".into());
    }
    if installation(app, "rust-analyzer").is_ok() {
        result.push("rust".into());
    }
    if installation(app, "pyright").is_ok() || installation(app, "ruff").is_ok() {
        result.push("python".into());
    }
    result
}
/// Only a review-pinned standalone Pyright executable enables semantic Python
/// operations; Ruff alone cannot provide completion/hover/definitions.
pub fn installed_semantic_languages(app: &AppHandle) -> Vec<String> {
    let mut languages=Vec::new();
    if installation(app, "rust-analyzer").is_ok() { languages.push("rust".into()); }
    if installation(app, "clangd").is_ok() {
        languages.push("c".into());
        languages.push("cpp".into());
    }
    if installation(app, "pyright").is_ok() { languages.push("python".into()); }
    languages
}
pub fn installed_worker_ready(app: &AppHandle) -> bool {
    !installed_languages(app).is_empty()
}
fn language_server_for_path(path: &str, language_id: &str) -> Result<&'static str, String> {
    if path.len() > 1024
        || path.contains('\\')
        || path.contains(':')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path.chars().any(char::is_control)
    {
        return Err("LSP_INVALID_ACTIVE_SOURCE".into());
    }
    let extension = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match (extension.as_str(), language_id) {
        ("rs", "rust") => Ok("rust-analyzer"),
        ("c" | "h", "c") | ("cpp" | "cc" | "cxx" | "hpp" | "hxx", "cpp") => Ok("clangd"),
        ("py" | "pyi", "python") => Ok("ruff"),
        _ => Err("LSP_LANGUAGE_NOT_APPROVED".into()),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LspRequest {
    project_id: String,
    relative_path: String,
    expected_revision: String,
    buffer_revision: u64,
    language_id: String,
    text: String,
    kind: Option<String>,
    position: Option<Position>,
}
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SourceCoordinate {
    project_id: String,
    relative_path: String,
    disk_sha256: String,
    buffer_revision: u64,
    language_id: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionCoordinate {
    relative_path: String,
    disk_sha256: String,
    range: lu_ide_workspace::lsp_session::Span,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LspReply {
    source: SourceCoordinate,
    provider_id: &'static str,
    diagnostics: Vec<Diagnostic>,
    completions: Vec<Completion>,
    hover: Option<String>,
    definition: Option<DefinitionCoordinate>,
}

const MAX_OPEN_LSP_FILES: usize = 16;
const MAX_SESSION_LSP_BYTES: usize = 2 * 1024 * 1024;

// Strict canonical staged-relative URI reconstruction. The server's URI is
// never passed to the filesystem API; only host-minted project paths are.
fn encoded_relative(path: &str) -> Option<String> {
    if path.is_empty() || path.len() > 1024 || path.contains(':') ||
        path.contains('\\') || path.chars().any(char::is_control) ||
        path.split('/').any(|part| part.is_empty() || part == "." || part == "..") {
        return None;
    }
    let mut encoded = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() ||
            matches!(byte, b'/' | b'.' | b'-' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    Some(encoded)
}
fn staged_uri_prefix<'a>(native_uri: &'a str, relative: &str) -> Option<&'a str> {
    let suffix = encoded_relative(relative)?;
    let prefix = native_uri.strip_suffix(&suffix)?;
    (prefix.starts_with("file:///") && prefix.ends_with('/')).then_some(prefix)
}
fn staged_relative_from_uri(primary_uri: &str, primary_path: &str,
    candidate: &str) -> Option<String> {
    let prefix = staged_uri_prefix(primary_uri, primary_path)?;
    let encoded = candidate.strip_prefix(prefix)?;
    if encoded.is_empty() || encoded.len() > 2048 { return None; }
    let raw = encoded.as_bytes();
    let mut bytes = Vec::with_capacity(encoded.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            let pair = raw.get(i + 1..i + 3)?;
            let hex = |byte: u8| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'A'..=b'F' => Some(byte - b'A' + 10),
                _ => None,
            };
            bytes.push((hex(pair[0])? << 4) | hex(pair[1])?);
            i += 3;
        } else {
            if raw[i] == b'?' || raw[i] == b'#' { return None; }
            bytes.push(raw[i]);
            i += 1;
        }
    }
    let relative = String::from_utf8(bytes).ok()?;
    (encoded_relative(&relative)?.as_str() == encoded).then_some(relative)
}
fn valid_provider_path(provider: &str, path: &str, language: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match provider {
        "rust-analyzer" => language == "rust" && ext == "rs",
        "clangd" => matches!(language, "c" | "cpp") &&
            matches!(ext.as_str(), "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hxx"),
        "pyright" | "ruff" => language == "python" && matches!(ext.as_str(), "py" | "pyi"),
        _ => false,
    }
}
fn snapshot_file_sha256(path: &Path, root: &Path) -> Option<String> {
    let info = fs::symlink_metadata(path).ok()?;
    if !info.file_type().is_file() || info.len() > 512 * 1024 { return None; }
    let canonical_root = fs::canonicalize(root).ok()?;
    let canonical_file = fs::canonicalize(path).ok()?;
    if !canonical_file.starts_with(&canonical_root) { return None; }
    let mut reader = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut block = [0_u8; 32 * 1024];
    loop {
        let size = reader.read(&mut block).ok()?;
        if size == 0 { break; }
        hasher.update(&block[..size]);
    }
    Some(format!("{:x}", hasher.finalize()))
}
fn file_id(relative: &str) -> String {
    let digest = format!("file_{:x}", Sha256::digest(relative.as_bytes()));
    digest[..37].to_string()
}
#[derive(Clone)]
struct ActiveDocument {
    source: DiagnosticSource,
    relative_path: String,
    saved_revision: String,
    native_uri: String,
    text: String,
    diagnostics: Vec<Diagnostic>,
}
struct Active {
    project_id: String,
    approved_native_root: PathBuf,
    sealed_root: PathBuf,
    primary_path: String,
    primary_uri: String,
    provider: &'static str,
    documents: BTreeMap<String, ActiveDocument>,
    // This single process and its pipes are owned by the OS supervisor.
    // All operations on all registered documents hold the same host mutex.
    session: NativeLspSession,
}
fn bind_registered_definition(active: &Active, origin: &ActiveDocument,
    registry: &IdeWorkspaceRegistry, candidate: &Definition) -> Option<DefinitionCoordinate> {
    if candidate.uri == origin.native_uri {
        return Some(DefinitionCoordinate {
            relative_path: origin.relative_path.clone(),
            disk_sha256: origin.saved_revision.clone(),
            range: candidate.range.clone(),
        });
    }
    let relative = staged_relative_from_uri(
        &active.primary_uri, &active.primary_path, &candidate.uri)?;
    if !valid_provider_path(active.provider, &relative,
        match active.provider {
            "rust-analyzer" => "rust", "clangd" => "cpp",
            "ruff" | "pyright" => "python", _ => return None,
        }) { return None; }
    // A server's location becomes usable only when the native project
    // registry and the sealed read-only worker source agree on exact bytes.
    let target = registry.preview_source_file(&active.project_id, &relative).ok()?;
    if target.project_id != active.project_id ||
        target.relative_path != relative ||
        snapshot_file_sha256(&active.sealed_root.join(&relative), &active.sealed_root)?
            != target.revision { return None; }
    let from = position_to_utf16(&target.text, &candidate.range.start)?;
    let to = position_to_utf16(&target.text, &candidate.range.end)?;
    if from > to { return None; }
    Some(DefinitionCoordinate {
        relative_path: relative,
        disk_sha256: target.revision,
        range: candidate.range.clone(),
    })
}
impl Active {
    fn output(&self, path: &str) -> Result<LspReply, String> {
        let doc = self.documents.get(path).ok_or("LSP_DOCUMENT_NOT_OPEN")?;
        Ok(LspReply {
            provider_id: self.provider,
            source: SourceCoordinate {
                project_id: self.project_id.clone(),
                relative_path: doc.relative_path.clone(),
                disk_sha256: doc.saved_revision.clone(),
                buffer_revision: doc.source.buffer_revision,
                language_id: doc.source.language_id.clone(),
            },
            diagnostics: doc.diagnostics.clone(),
            completions: vec![],
            hover: None,
            definition: None,
        })
    }
    fn receive(&mut self, kind: &str, path: &str,
        registry: &IdeWorkspaceRegistry) -> Result<LspReply, String> {
        let started = Instant::now();
        let mut result = self.output(path)?;
        while started.elapsed() < Duration::from_secs(if kind == "diagnostics" { 1 } else { 8 }) {
            for event in self.session.poll().map_err(|e| e.code())? {
                let completed = matches!(
                    (&event, kind),
                    (LspEvent::Completions(_), "completion") |
                    (LspEvent::Hover(_), "hover") |
                    (LspEvent::Definition(_), "definition")
                );
                match event {
                    // The single-file legacy notification is only emitted
                    // when the protocol has exactly one open document.
                    LspEvent::Diagnostics(values) if self.documents.len() == 1 => {
                        if let Some(doc) = self.documents.values_mut().next() {
                            doc.diagnostics = values;
                        }
                    }
                    LspEvent::ScopedDiagnostics { source, diagnostics } => {
                        if let Some(doc) = self.documents.values_mut()
                            .find(|doc| doc.source.same_revision(&source)) {
                            doc.diagnostics = diagnostics;
                        }
                    }
                    LspEvent::Completions(values) if kind == "completion" => {
                        result.completions = values;
                    }
                    LspEvent::Hover(value) if kind == "hover" => {
                        result.hover = Some(value);
                    }
                    LspEvent::Definition(Some(candidate)) if kind == "definition" => {
                        if let Some(origin) = self.documents.get(path) {
                            result.definition = bind_registered_definition(
                                self, origin, registry, &candidate);
                        }
                    }
                    _ => {}
                }
                if completed {
                    result.diagnostics = self.output(path)?.diagnostics;
                    return Ok(result);
                }
            }
            thread::sleep(Duration::from_millis(30));
        }
        if kind == "diagnostics" {
            result.diagnostics = self.output(path)?.diagnostics;
            Ok(result)
        } else {
            Err("LSP_REQUEST_TIMEOUT".into())
        }
    }
}

pub struct NativeLspRuntime {
    supervisor: Arc<Supervisor>,
    stage: PathBuf,
    active: Mutex<Option<Active>>,
    // A failed OS worker close is never interpreted as an exited process.
    // INT02-F must reconcile that native journal before further admission.
    cleanup_uncertain: Mutex<bool>,
}
impl NativeLspRuntime {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let root = app
            .path()
            .app_data_dir()
            .map_err(|_| "LSP_HOST_PROFILE_UNAVAILABLE")?;
        fs::create_dir_all(&root).map_err(|_| "LSP_PROFILE_UNWRITABLE")?;
        let stage = root.join("lsp-staged-inputs");
        fs::create_dir_all(&stage).map_err(|_| "LSP_PROFILE_UNWRITABLE")?;
        let supervisor =
            Supervisor::open(root.join("lsp-job-receipts.jsonl")).map_err(|e| e.code())?;
        Ok(Self {
            supervisor: Arc::new(supervisor),
            stage,
            active: Mutex::new(None),
            cleanup_uncertain: Mutex::new(false),
        })
    }
    fn validated_registered_text(
        registry: &IdeWorkspaceRegistry,
        request: &LspRequest,
    ) -> Result<(PathBuf, String), String> {
        if language_server_for_path(&request.relative_path, &request.language_id).is_err()
            || request.text.len() > 512 * 1024
            || request.text.contains('\0')
            || request.expected_revision.len() != 64
        {
            return Err("LSP_INVALID_ACTIVE_SOURCE".into());
        }
        let root = registry
            .trusted_native_worker_root(&request.project_id)
            .map_err(|_| "LSP_PROJECT_NOT_ACTIVE")?;
        let saved = registry
            .preview_source_file(&request.project_id, &request.relative_path)
            .map_err(|_| "LSP_SOURCE_UNAVAILABLE")?;
        if saved.revision != request.expected_revision {
            return Err("LSP_FILE_CHANGED_ON_DISK".into());
        }
        Ok((root, saved.revision))
    }
    pub fn start(
        &self,
        app: &AppHandle,
        registry: &IdeWorkspaceRegistry,
        request: LspRequest,
    ) -> Result<Value, String> {
        // Reserve the native owner for the full admission and OS consent.
        // A concurrent window cannot spawn a second worker before publish.
        let mut holder = self.active.lock().map_err(|_| "LSP_HOST_UNAVAILABLE")?;
        if *self.cleanup_uncertain.lock().map_err(|_| "LSP_HOST_UNAVAILABLE")? {
            return Err("LSP_NATIVE_CLEANUP_UNVERIFIED".into());
        }
        if holder.is_some() {
            return Err("LSP_SESSION_ALREADY_ACTIVE".into());
        }
        let base_server =
            language_server_for_path(&request.relative_path, &request.language_id)?;
        // A reviewed Pyright bundle takes priority for full Python semantics.
        // If it is absent, the only Python fallback is reviewed Ruff linting.
        let server_id = if request.language_id == "python"
            && installation(app, "pyright").is_ok() {
            "pyright"
        } else { base_server };
        let install = installation(app, server_id)?;
        let (root, saved_revision) = Self::validated_registered_text(registry, &request)?;
        // This is a genuine native OS confirmation, not a client boolean
        // or model-generated consent. It runs on Tauri's background thread.
        let choice=app.dialog()
            .message(format!("Allow the installed {} language server to inspect the current file and the staged copy of this project?\n\nProject: {}\nFile: {}\n\nThe isolated worker cannot edit source files.",
                server_id,request.project_id,request.relative_path))
            .title("Allow language analysis")
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Allow this session".into(),"Cancel".into(),
            )).blocking_show();
        if !choice {
            return Err("LSP_HUMAN_APPROVAL_DENIED".into());
        }
        // Recheck after the human dialog: no stale source or replacement
        // can inherit approval to a different on-disk project.
        let (root_now, rev_now) = Self::validated_registered_text(registry, &request)?;
        if root_now != root || rev_now != saved_revision {
            return Err("LSP_SCOPE_CHANGED_DURING_APPROVAL".into());
        }
        let input = publish_lsp_input(&root, &self.stage, &request.relative_path, server_id)
            .map_err(|e| e.code())?;
        if input.seal.manifest_sha256 != saved_revision {
            return Err("LSP_SNAPSHOT_CHANGED_DURING_ADMISSION".into());
        }
        let observed = registry
            .inspect_git_worktree(&request.project_id)
            .local_head
            .filter(|sha| sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
            .unwrap_or_else(|| "0".repeat(40));
        let plan = JobPlan {
            project_id: request.project_id.clone(),
            recipe_id: format!("lsp_{}", server_id.replace('-', "_")),
            source_head: observed.clone(),
            input_snapshot_sha256: input.seal.source_sha256.clone(),
            manifest_sha256: input.seal.manifest_sha256.clone(),
            toolchain_sha256: install.bundle_sha256,
            debug_target_binding_sha256: None,
            runtime_limit_ms: 5 * 60_000,
            memory_limit_bytes: 512 * 1024 * 1024,
            process_limit: if server_id == "clangd" { 16 } else { 4 },
            log_limit_bytes: 64 * 1024,
            effect_class: "language_analysis_isolated".into(),
        };
        let prepared = self
            .supervisor
            .prepare(plan.clone())
            .map_err(|e| e.code())?;
        self.supervisor
            .record_human_approval(&prepared.job_id, &prepared.plan_sha256, &observed, true)
            .map_err(|e| e.code())?;
        // Both OS paths consume the same exact project/source grant and
        // native pinned release digest; Linux additionally requires a
        // root-owned bundle, root-owned bwrap and kernel cgroup-v2 quotas.
        // No same-user fallback and no PATH-based language servers.
        #[cfg(windows)]
        let mut session = start_windows_lsp(
            Arc::clone(&self.supervisor),
            &prepared.job_id,
            &plan,
            &observed,
            &input,
            &install.bundle,
            &install.service,
        )
        .map_err(|e| e.code())?;
        #[cfg(target_os = "linux")]
        let mut session = start_linux_lsp(
            Arc::clone(&self.supervisor),
            &prepared.job_id,
            &plan,
            &observed,
            &input,
            &install.bundle,
        )
        .map_err(|e| e.code())?;
        session.initialize().map_err(|e| e.code())?;
        let started = Instant::now();
        while !session.is_ready() && started.elapsed() < Duration::from_secs(12) {
            session.poll().map_err(|e| e.code())?;
            thread::sleep(Duration::from_millis(40));
        }
        if !session.is_ready() {
            return Err("LSP_INITIALIZE_TIMEOUT".into());
        }
        let native_uri = session.native_document_uri().to_owned();
        let file_id = format!(
            "file_{:x}",
            Sha256::digest(request.relative_path.as_bytes())
        );
        let source = DiagnosticSource {
            project_id: request.project_id.clone(),
            file_id: file_id[..37].to_owned(),
            disk_sha256: saved_revision.clone(),
            buffer_revision: request.buffer_revision,
            language_id: request.language_id.clone(),
        };
        session
            .open(source.clone(), &native_uri, &request.text)
            .map_err(|e| e.code())?;
        let project_id = request.project_id;
        let relative_path = request.relative_path;
        let first = ActiveDocument {
            source, relative_path: relative_path.clone(),
            saved_revision, native_uri: native_uri.clone(),
            text: request.text, diagnostics: vec![],
        };
        let mut documents = BTreeMap::new();
        documents.insert(relative_path.clone(), first);
        let active = Active {
            project_id, approved_native_root: root,
            sealed_root: input.root.clone(),
            primary_path: relative_path.clone(), primary_uri: native_uri,
            provider: server_id, documents, session,
        };
        let result = serde_json::to_value(active.output(&relative_path)?)
            .map_err(|_| "LSP_RESPONSE_UNAVAILABLE")?;
        *holder = Some(active);
        Ok(result)
    }

    pub fn open_document(&self, registry: &IdeWorkspaceRegistry,
        request: LspRequest) -> Result<Value, String> {
        let mut lock = self.active.lock().map_err(|_| "LSP_HOST_UNAVAILABLE")?;
        let active = lock.as_mut().ok_or("LSP_SESSION_NOT_STARTED")?;
        if active.project_id != request.project_id ||
            !valid_provider_path(active.provider, &request.relative_path, &request.language_id) ||
            active.documents.len() >= MAX_OPEN_LSP_FILES ||
            active.documents.contains_key(&request.relative_path) {
            return Err("LSP_DOCUMENT_SCOPE_DENIED".into());
        }
        let (root, revision) = Self::validated_registered_text(registry, &request)?;
        if root != active.approved_native_root ||
            snapshot_file_sha256(&active.sealed_root.join(&request.relative_path),
                &active.sealed_root).as_deref() != Some(revision.as_str()) ||
            active.documents.values().map(|doc| doc.text.len()).sum::<usize>()
                .saturating_add(request.text.len()) > MAX_SESSION_LSP_BYTES {
            return Err("LSP_DOCUMENT_OUTSIDE_APPROVED_SNAPSHOT".into());
        }
        let prefix = staged_uri_prefix(&active.primary_uri, &active.primary_path)
            .ok_or("LSP_NATIVE_URI_UNAVAILABLE")?;
        let native_uri = format!("{}{}", prefix,
            encoded_relative(&request.relative_path).ok_or("LSP_INVALID_ACTIVE_SOURCE")?);
        if native_uri.len() > 2048 { return Err("LSP_NATIVE_URI_UNAVAILABLE".into()); }
        let source = DiagnosticSource {
            project_id: request.project_id,
            file_id: file_id(&request.relative_path),
            disk_sha256: revision.clone(),
            buffer_revision: request.buffer_revision,
            language_id: request.language_id,
        };
        active.session.open(source.clone(), &native_uri, &request.text)
            .map_err(|e| e.code())?;
        let path = request.relative_path;
        active.documents.insert(path.clone(), ActiveDocument {
            source, relative_path: path.clone(), saved_revision: revision,
            native_uri, text: request.text, diagnostics: vec![],
        });
        serde_json::to_value(active.output(&path)?)
            .map_err(|_| "LSP_RESPONSE_UNAVAILABLE".into())
    }
    pub fn close_document(&self, request: LspRequest) -> Result<(), String> {
        let mut lock = self.active.lock().map_err(|_| "LSP_HOST_UNAVAILABLE")?;
        let active = lock.as_mut().ok_or("LSP_SESSION_NOT_STARTED")?;
        let doc = active.documents.get(&request.relative_path)
            .ok_or("LSP_DOCUMENT_NOT_OPEN")?;
        if request.project_id != active.project_id ||
            request.expected_revision != doc.saved_revision ||
            request.language_id != doc.source.language_id {
            return Err("LSP_DOCUMENT_SCOPE_DENIED".into());
        }
        active.session.close_document(&doc.source).map_err(|e| e.code())?;
        active.documents.remove(&request.relative_path);
        Ok(())
    }
    fn use_active<F>(&self, registry: &IdeWorkspaceRegistry,
        request: LspRequest, mut action: F) -> Result<Value, String>
    where F: FnMut(&mut Active, &LspRequest) -> Result<LspReply, String> {
        let mut lock = self.active.lock().map_err(|_| "LSP_HOST_UNAVAILABLE")?;
        let active = lock.as_mut().ok_or("LSP_SESSION_NOT_STARTED")?;
        let doc = active.documents.get(&request.relative_path)
            .ok_or("LSP_DOCUMENT_NOT_OPEN")?;
        if request.project_id != active.project_id ||
            request.expected_revision != doc.saved_revision ||
            request.language_id != doc.source.language_id ||
            request.buffer_revision < doc.source.buffer_revision {
            return Err("LSP_SOURCE_SCOPE_MISMATCH".into());
        }
        let (root, _) = Self::validated_registered_text(registry, &request)?;
        if root != active.approved_native_root {
            return Err("LSP_PROJECT_SCOPE_CHANGED".into());
        }
        let old_source = doc.source.clone();
        let old_text = doc.text.clone();
        if request.buffer_revision > old_source.buffer_revision {
            if active.documents.values().map(|doc| doc.text.len()).sum::<usize>()
                .saturating_sub(old_text.len()).saturating_add(request.text.len())
                > MAX_SESSION_LSP_BYTES {
                return Err("LSP_SESSION_TEXT_LIMIT".into());
            }
            let mut next = old_source.clone();
            next.buffer_revision = request.buffer_revision;
            active.session.changed(&old_source, next.clone(), &request.text)
                .map_err(|e| e.code())?;
            let doc = active.documents.get_mut(&request.relative_path)
                .ok_or("LSP_DOCUMENT_NOT_OPEN")?;
            doc.source = next;
            doc.text = request.text.clone();
            doc.diagnostics.clear();
        } else if old_text != request.text {
            return Err("LSP_BUFFER_REVISION_MISMATCH".into());
        }
        let result = action(active, &request)?;
        serde_json::to_value(result).map_err(|_| "LSP_RESPONSE_UNAVAILABLE".into())
    }
    pub fn feature(&self, registry: &IdeWorkspaceRegistry,
        request: LspRequest) -> Result<Value, String> {
        self.use_active(registry, request, |active, req| {
            let kind = req.kind.as_deref().ok_or("LSP_METHOD_REQUIRED")?;
            let doc = active.documents.get(&req.relative_path)
                .ok_or("LSP_DOCUMENT_NOT_OPEN")?;
            if !matches!(kind, "completion" | "hover" | "definition") ||
                (doc.source.language_id == "python" && active.provider != "pyright") {
                return Err("LSP_METHOD_DENIED".into());
            }
            let position = req.position.clone().ok_or("LSP_POSITION_REQUIRED")?;
            active.session.query(kind, &doc.source, position).map_err(|e| e.code())?;
            active.receive(kind, &req.relative_path, registry)
        })
    }
    pub fn diagnostics(&self, registry: &IdeWorkspaceRegistry,
        request: LspRequest) -> Result<Value, String> {
        self.use_active(registry, request, |active, req|
            active.receive("diagnostics", &req.relative_path, registry))
    }
    pub fn change(&self, registry: &IdeWorkspaceRegistry,
        request: LspRequest) -> Result<Value, String> {
        self.use_active(registry, request, |active, req|
            active.output(&req.relative_path))
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut lock = self.active.lock().map_err(|_| "LSP_HOST_UNAVAILABLE")?;
        let mut uncertain = self.cleanup_uncertain.lock()
            .map_err(|_| "LSP_HOST_UNAVAILABLE")?;
        if *uncertain { return Err("LSP_NATIVE_CLEANUP_UNVERIFIED".into()); }
        if let Some(active) = lock.take() {
            if let Err(error) = active.session.close() {
                *uncertain = true;
                return Err(error.code());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_language_router_is_fixed_and_denies_untrusted_paths() {
        assert_eq!(
            language_server_for_path("src/main.rs", "rust").unwrap(),
            "rust-analyzer"
        );
        assert_eq!(
            language_server_for_path("lib/example.cpp", "cpp").unwrap(),
            "clangd"
        );
        assert_eq!(
            language_server_for_path("app/main.c", "c").unwrap(),
            "clangd"
        );
        assert_eq!(
            language_server_for_path("tools/a.py", "python").unwrap(),
            "ruff"
        );
        for (path, language) in [
            ("../../secret.rs", "rust"),
            ("src\\evil.rs", "rust"),
            ("main.rs", "python"),
            ("main.ts", "typescript"),
            ("main.rs/../../a.rs", "rust"),
            ("file:///secret.c", "c"),
        ] {
            assert!(language_server_for_path(path, language).is_err(), "{path}");
        }
    }
    #[test]
    fn absent_release_pins_deny_by_default() {
        assert!(pin(None).is_err());
        assert!(pin(Some("unsafe")).is_err());
        assert!(pin(Some(&"a".repeat(64))).is_ok());
    }
}
